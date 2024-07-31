use std::collections::HashSet;

use interfaces::ClientPerspective;

use crate::config::parse::*;
use crate::config::ConfigParseError;
use crate::game::event_handler::EventHandler;
use crate::game::game_view::GameView;
use crate::map::direction::Direction;
use crate::map::map_view::MapView;
use crate::terrain::TerrainType;
use crate::units::attributes::ActionStatus;

use super::custom_action::*;
use super::player::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CustomPower {
    None, // not all powers have a custom effect
    List(Vec<PlayerScript>),
    TapioSpiritSeed,
    TapioFairyFury(u8, u8),
    LageosRockets(u32, u8, u8),
    LageosStunRockets(u32, u8, u8),
}

impl FromConfig for CustomPower {
    fn from_conf(s: &str) -> Result<(Self, &str), ConfigParseError> {
        let (base, mut s) = string_base(s);
        Ok((match base {
            "L" | "List" => {
                let (effects, r) = parse_inner_vec(s, true)?;
                s = r;
                Self::List(effects)
            }
            "TapioSpiritSeed" => Self::TapioSpiritSeed,
            "TapioFairyFury" => {
                let (range, damage, r) = parse_tuple2(s)?;
                s = r;
                Self::TapioFairyFury(range, 1.max(damage))
            }
            "LageosRockets" => {
                let (count, range, damage, r) = parse_tuple3(s)?;
                s = r;
                Self::LageosRockets(1.max(count), range, 1.max(damage))
            }
            "LageosStunRockets" => {
                let (count, range, damage, r) = parse_tuple3(s)?;
                s = r;
                Self::LageosStunRockets(1.max(count), range, damage)
            }
            invalid => return Err(ConfigParseError::UnknownEnumMember(format!("CustomPower::{}", invalid))),
        }, s))
    }
}

impl CustomPower {
    pub fn next_condition<D: Direction>(
        &self,
        game: &impl GameView<D>,
        data_so_far: &[CustomActionData<D>],
    ) -> CustomActionTestResult<D> {
        match self {
            Self::TapioFairyFury(_, _) |
            Self::List(_) |
            Self::None => CustomActionTestResult::Success,
            Self::TapioSpiritSeed => {
                if data_so_far.len() == 0 {
                    let mut options = HashSet::new();
                    for p in game.all_points() {
                        if game.get_terrain(p).unwrap().typ() == TerrainType::Grass {
                            options.insert(p);
                        }
                    }
                    // if this becomes optional, remove panic from Self::execute
                    CustomActionTestResult::Next(CustomActionDataOptions::Point(options))
                } else {
                    CustomActionTestResult::Success
                }
            }
            Self::LageosRockets(count, _, _) |
            Self::LageosStunRockets(count, _, _) => {
                if data_so_far.len() < *count as usize {
                    CustomActionTestResult::NextOrSuccess(CustomActionDataOptions::Point(game.all_points().into_iter().collect()))
                } else {
                    CustomActionTestResult::Success
                }
            }
        }
    }

    pub fn is_data_valid<D: Direction>(
        &self,
        game: &impl GameView<D>,
        data: &[CustomActionData<D>],
    ) -> bool {
        for i in 0..data.len() {
            use CustomActionTestResult as R;
            match self.next_condition(game, &data[..i]) {
                R::Failure => return false,
                R::Success => return i == data.len(),
                R::Next(options) => {
                    if i >= data.len() || !options.contains(&data[i]) {
                        return false;
                    }
                }
                R::NextOrSuccess(options) => {
                    if i < data.len() && !options.contains(&data[i]) {
                        return false;
                    }
                }
            }
        }
        true
    }

    pub fn execute<D: Direction>(
        &self,
        handler: &mut EventHandler<D>,
        data: &[CustomActionData<D>],
    ) {
        let owner_id = handler.get_game().current_player().get_owner_id();
        match self {
            Self::None => (),
            Self::List(effects) => {
                for effect in effects {
                    effect.trigger(handler, owner_id);
                }
            },
            Self::TapioSpiritSeed => {
                let target = match data[0] {
                    CustomActionData::Point(p) => p,
                    _ => panic!("TapioSpiritSeed needs Point, got {data:?}"),
                };
                handler.terrain_replace(target, TerrainType::FairyForest.instance(handler.environment()).set_owner_id(owner_id).build_with_defaults());
            }
            Self::TapioFairyFury(range, damage) => {
                tapio_fairy_fury(handler, owner_id, *range, *damage);
            }
            Self::LageosRockets(_, range, damage) => {
                for data in data {
                    let target = match data {
                        CustomActionData::Point(p) => *p,
                        _ => panic!("TapioSpiritSeed needs Point, got {data:?}"),
                    };
                    let mut aura = HashSet::new();
                    aura.insert(target);
                    for layer in handler.get_map().range_in_layers(target, *range as usize) {
                        for p in layer {
                            aura.insert(p);
                        }
                    }
                    deal_damage(handler, aura.into_iter(), ClientPerspective::Neutral, *damage);
                }
            }
            Self::LageosStunRockets(_, range, damage) => {
                for data in data {
                    let target = match data {
                        CustomActionData::Point(p) => *p,
                        _ => panic!("TapioSpiritSeed needs Point, got {data:?}"),
                    };
                    let mut aura = HashSet::new();
                    aura.insert(target);
                    for layer in handler.get_map().range_in_layers(target, *range as usize) {
                        for p in layer {
                            aura.insert(p);
                        }
                    }
                    deal_damage(handler, aura.clone().into_iter(), ClientPerspective::Neutral, *damage);
                    for p in aura {
                        if handler.get_map().get_unit(p).is_some() {
                            handler.unit_status(p, ActionStatus::Exhausted);
                        }
                    }
                }
            }
        }
    }
}

pub(super) fn tapio_fairy_fury<D: Direction>(handler: &mut EventHandler<D>, owner_id: i8, range: u8, damage: u8) {
    let team = handler.environment().get_team(owner_id);
    let mut aura = HashSet::new();
    for p in handler.get_map().all_points() {
        let terrain = handler.get_map().get_terrain(p).unwrap();
        if terrain.typ() == TerrainType::FairyForest && terrain.get_owner_id() == owner_id {
            aura.insert(p);
            for layer in handler.get_map().range_in_layers(p, range as usize) {
                for p in layer {
                    aura.insert(p);
                }
            }
            handler.terrain_replace(p, TerrainType::Grass.instance(handler.environment()).build_with_defaults());
        }
    }
    deal_damage(handler, aura.into_iter(), team, damage);
}
