use std::collections::HashSet;

use zipper::*;
use zipper::zipper_derive::*;

use crate::{player::*, map::{direction::Direction, point::Point}, game::{game::Game, events::{EventHandler, Event}}, units::{normal_units::DroneId, structures::{Structures, Structure}}};
use crate::units::normal_units::{NormalUnits, NormalUnit};
use crate::units::movement::*;
use crate::units::UnitType;


macro_rules! land_units {
    () => {
        MovementType::Foot |
        MovementType::Wheel |
        MovementType::Hover(HoverMode::Land) |
        MovementType::Treads |
        MovementType::Chess
    };
}

macro_rules! sea_units {
    () => {
        MovementType::Hover(HoverMode::Sea) |
        MovementType::Boat |
        MovementType::Ship
    };
}

macro_rules! air_units {
    () => {
        MovementType::Heli |
        MovementType::Plane
    };
}


#[derive(Debug, PartialEq, Clone, Zippable)]
#[zippable(bits = 8)]
pub enum Terrain<D: Direction> {
    Beach,
    Bridge,
    ChessTile,
    Flame,
    Forest,
    Fountain,
    Grass,
    Hill,
    Icebergs,
    Lillypads,
    Mountain,
    Pipe(D::P),
    Realty(Realty, Option::<Owner>, CaptureProgress),
    Reef,
    Ruins,
    Sea,
    ShallowSea,
    Street,
    Tavern,
}
impl<D: Direction> Terrain<D> {
    pub fn movement_cost(&self, movement_type: MovementType) -> Option<MovementPoints> {
        match (self, movement_type) {
            (Self::Beach, MovementType::Chess) => None,
            (Self::Beach, MovementType::Ship) => None,
            (Self::Beach, _) => Some(MovementPoints::from(1.)),

            (Self::Bridge, MovementType::Chess) => None,
            (Self::Bridge, _) => Some(MovementPoints::from(1.)),

            (Self::Grass, sea_units!()) => None,
            (Self::Grass, MovementType::Hover(_)) => Some(MovementPoints::from(1.5)),
            (Self::Grass, MovementType::Wheel) => Some(MovementPoints::from(1.5)),
            (Self::Grass, land_units!()) => Some(MovementPoints::from(1.)),
            (Self::Grass, air_units!()) => Some(MovementPoints::from(1.)),
            
            (Self::Forest, sea_units!()) => None,
            (Self::Forest, MovementType::Hover(_)) => Some(MovementPoints::from(2.)),
            (Self::Forest, MovementType::Wheel) => Some(MovementPoints::from(2.)),
            (Self::Forest, MovementType::Foot) => Some(MovementPoints::from(1.)),
            (Self::Forest, land_units!()) => Some(MovementPoints::from(1.5)),
            (Self::Forest, air_units!()) => Some(MovementPoints::from(1.)),

            (Self::Hill, sea_units!()) => None,
            (Self::Hill, MovementType::Hover(_)) => Some(MovementPoints::from(2.)),
            (Self::Hill, MovementType::Wheel) => Some(MovementPoints::from(2.)),
            (Self::Hill, MovementType::Foot) => Some(MovementPoints::from(1.)),
            (Self::Hill, land_units!()) => Some(MovementPoints::from(1.5)),
            (Self::Hill, air_units!()) => Some(MovementPoints::from(1.)),

            (Self::Icebergs, MovementType::Boat) => Some(MovementPoints::from(1.5)),
            (Self::Icebergs, MovementType::Ship) => Some(MovementPoints::from(2.)),
            (Self::Icebergs, MovementType::Hover(_)) => Some(MovementPoints::from(1.5)),
            (Self::Icebergs, land_units!()) => None,
            (Self::Icebergs, air_units!()) => Some(MovementPoints::from(1.)),

            (Self::Lillypads, MovementType::Foot) => Some(MovementPoints::from(2.)),
            (Self::Lillypads, MovementType::Boat) => Some(MovementPoints::from(1.5)),
            (Self::Lillypads, MovementType::Ship) => Some(MovementPoints::from(1.)),
            (Self::Lillypads, land_units!()) => None,
            (Self::Lillypads, air_units!()) => Some(MovementPoints::from(1.)),
            (Self::Lillypads, MovementType::Hover(_)) => Some(MovementPoints::from(2.)),

            (Self::Mountain, sea_units!()) => None,
            (Self::Mountain, MovementType::Foot) => Some(MovementPoints::from(1.5)),
            (Self::Mountain, MovementType::Heli) => Some(MovementPoints::from(1.5)),
            (Self::Mountain, MovementType::Plane) => Some(MovementPoints::from(1.)),
            (Self::Mountain,
                MovementType::Hover(_) |
                MovementType::Wheel |
                MovementType::Treads |
                MovementType::Chess) => None,

            (Self::Sea, land_units!()) => None,
            (Self::Sea, MovementType::Hover(_)) => Some(MovementPoints::from(1.5)),
            (Self::Sea, _) => Some(MovementPoints::from(1.)),

            (Self::ShallowSea, land_units!()) => None,
            (Self::ShallowSea, _) => Some(MovementPoints::from(1.)),

            (Self::Reef, land_units!()) => None,
            (Self::Reef, MovementType::Ship) => None,
            (Self::Reef, MovementType::Boat) => Some(MovementPoints::from(1.5)),
            (Self::Reef, _) => Some(MovementPoints::from(1.)),

            (Self::Street, sea_units!()) => None,
            (Self::Street, _) => Some(MovementPoints::from(1.)),

            (Self::Ruins, sea_units!()) => None,
            (Self::Ruins, _) => Some(MovementPoints::from(1.)),

            (Self::Flame, _) => None,
            
            (Self::Realty(realty, _, _), movement_type) => return realty.movement_cost(movement_type),

            (Self::Tavern, MovementType::Chess) => None,
            (Self::Tavern, _) => Some(MovementPoints::from(1.)),

            (Self::Fountain, land_units!()) => None,
            (Self::Fountain, sea_units!()) => Some(MovementPoints::from(1.)),
            (Self::Fountain, MovementType::Hover(_)) => Some(MovementPoints::from(1.)),
            (Self::Fountain, MovementType::Heli) => Some(MovementPoints::from(1.5)),
            (Self::Fountain, MovementType::Plane) => Some(MovementPoints::from(1.)),

            (Self::Pipe(_), _) => None,

            (Self::ChessTile, sea_units!()) => None,
            (Self::ChessTile, _) => Some(MovementPoints::from(1.)),
        }
    }
    pub fn is_water(&self) -> bool {
        self.movement_cost(MovementType::Boat).is_some()
    }
    pub fn like_beach_for_hovercraft(&self) -> bool {
        match self {
            Self::Beach => true,
            Self::Realty(realty, _, _) => realty.like_beach_for_hovercraft(),
            Self::Tavern => true,
            _ => false,
        }
    }
    pub fn update_movement_type(&self, movement_type: MovementType, prev_terrain: &Self) -> Option<MovementType> {
        // only sea-faring or flying units can cross between beach and bridge tiles
        if Self::Sea.movement_cost(movement_type).is_none() {
            match (prev_terrain, self) {
                (Self::Beach, Self::Bridge) |
                (Self::Bridge, Self::Beach) => {
                    return None;
                }
                _ => {}
            }
        }
        Some(match movement_type {
            MovementType::Hover(mode) => {
                if self.like_beach_for_hovercraft() {
                    MovementType::Hover(HoverMode::Beach)
                } else if mode == HoverMode::Beach {
                    MovementType::Hover(match self {
                        Self::Beach => HoverMode::Beach,
                        Self::Bridge => HoverMode::Sea,
                        Self::ChessTile => HoverMode::Land,
                        Self::Flame => HoverMode::Land,
                        Self::Forest => HoverMode::Land,
                        Self::Fountain => HoverMode::Sea,
                        Self::Grass => HoverMode::Land,
                        Self::Hill => HoverMode::Land,
                        Self::Icebergs => HoverMode::Sea,
                        Self::Lillypads => HoverMode::Sea,
                        Self::Mountain => HoverMode::Land,
                        Self::Pipe(_) => mode,
                        Self::Realty(_, _, _) => HoverMode::Land,
                        Self::Reef => HoverMode::Sea,
                        Self::Ruins => HoverMode::Land,
                        Self::Sea => HoverMode::Sea,
                        Self::ShallowSea => HoverMode::Sea,
                        Self::Street => HoverMode::Land,
                        Self::Tavern => HoverMode::Beach,
                    })
                } else {
                    MovementType::Hover(mode)
                }
            }
            _ => {
                movement_type
            }
        })
    }

    pub fn update_movement(&self, movement_meta: &MovementSearchMeta<D>, prev_terrain: &Self) -> Option<MovementSearchMeta<D>> {
        let remaining_movement = if let Some(cost) = self.movement_cost(movement_meta.movement_type) {
            if cost <= movement_meta.remaining_movement {
                movement_meta.remaining_movement - cost
            } else {
                return None;
            }
        } else {
            return None;
        };
        let movement_type = if let Some(m) = self.update_movement_type(movement_meta.movement_type, prev_terrain) {
            m
        } else {
            return None;
        };
        Some(MovementSearchMeta {
            remaining_movement,
            movement_type,
            illegal_next_dir: None,
            path: movement_meta.path.clone(),
            stealth: movement_meta.stealth,
        })
    }

    pub fn defense_bonus(&self, unit: &UnitType<D>) -> f32 {
        let movement_type = match unit {
            UnitType::Normal(unit) => {
                unit.get_movement(self).0
            }
            UnitType::Chess(_) => {
                MovementType::Chess
            }
            UnitType::Structure(_) => return 0.0,
        };
        match (self, movement_type) {
            (Self::Grass, land_units!()) => 0.1,
            (Self::Forest, land_units!()) => 0.2,
            (Self::Fountain, sea_units!()) => -0.2,
            (Self::Hill, land_units!()) => 0.1,
            (Self::Icebergs, sea_units!()) => 0.2,
            (Self::Lillypads, sea_units!()) => -0.1,
            (Self::Realty(_, _, _), land_units!()) => 0.3,
            (Self::Reef, sea_units!()) => 0.1,
            (Self::Ruins, land_units!()) => 0.3,
            (Self::Tavern, land_units!()) => 0.2,
            (_, _) => 0.,
        }
    }

    pub fn range_bonus(&self) -> u8 {
        match self {
            Self::Hill => 1,
            Self::Mountain => 1,
            Self::Fountain => 1,
            _ => 0,
        }
    }

    pub fn requires_true_sight(&self) -> bool {
        match self {
            Self::Forest => true,
            Self::Icebergs => true,
            _ => false
        }
    }

    pub fn get_vision(&self, game: &Game<D>, pos: Point, team: Perspective) -> HashSet<Point> {
        let mut result = HashSet::new();
        match self {
            Terrain::Flame => {
                result.insert(pos);
                for layer in game.get_map().range_in_layers(pos, 2) {
                    for (p, _, _) in layer {
                        result.insert(p);
                    }
                }
            }
            Terrain::Realty(_, owner, _) => {
                if let Some(player) = owner.and_then(|owner| game.get_owning_player(owner)) {
                    if Some(player.team) == team {
                        result.insert(pos.clone());
                    }
                }
            }
            _ => {}
        }
        result
    }

    pub fn fog_replacement(&self) -> Terrain<D> {
        match self {
            Terrain::Realty(realty, _, _) => Terrain::Realty(realty.clone(), None, CaptureProgress::None),
            _ => self.clone(),
        }
    }
    /*fn end_turn(&self) {
        match self {
            Terrain::Realty(realty, owner) => realty.end_turn(owner),
            _ => {}, // do nothin by default
        }
    }*/
}


const MAX_BUILT_THIS_TURN: u8 = 9;
pub type BuiltThisTurn = U8<{MAX_BUILT_THIS_TURN}>;
#[derive(Debug, PartialEq, Clone, Copy, Zippable)]
#[zippable(bits = 1)]
pub enum CaptureProgress {
    None,
    Capturing(Owner, U8::<9>),
}

#[derive(Debug, PartialEq, Clone, Zippable)]
#[zippable(bits = 6)]
pub enum Realty {
    Hq,
    City,
    OilPlatform,
    Factory(BuiltThisTurn),
    Port(BuiltThisTurn),
    Airport(BuiltThisTurn),
    ConstructionSite(BuiltThisTurn),
}
impl Realty {
    pub fn income_factor(&self) -> i16 {
        match self {
            Self::City => 1,
            Self::OilPlatform => 2,
            _ => 0,
        }
    }
    pub fn can_build(&self) -> bool {
        match self {
            Self::Factory(_) => true,
            Self::Airport(_) => true,
            Self::Port(_) => true,
            Self::ConstructionSite(_) => true,
            _ => false
        }
    }
    pub fn buildable_units<D: Direction>(&self, game: &Game<D>, owner: Owner) -> Vec<(UnitType<D>, u16)> {
        match self {
            Self::Factory(built_this_turn) => build_options_factory(game, owner, **built_this_turn),
            Self::Port(built_this_turn) => build_options_port(game, owner, **built_this_turn),
            Self::Airport(built_this_turn) => build_options_airport(game, owner, **built_this_turn),
            Self::ConstructionSite(built_this_turn) => build_options_construction_site(game, owner, **built_this_turn),
            _ => vec![],
        }
    }
    pub fn can_repair(&self, unit_type: &NormalUnits) -> bool {
        match self {
            Self::Factory(_) | Self::City => unit_type.repairable_factory(),
            Self::Port(_) | Self::OilPlatform => unit_type.repairable_port(),
            Self::Airport(_) => unit_type.repairable_airport(),            
            _ => false,
        }
    }
    pub fn movement_cost(&self, movement_type: MovementType) -> Option<MovementPoints> {
        match (self, movement_type) {
            (Self::Hq, MovementType::Chess) => None,
            (Self::Port(_), MovementType::Chess) => None,
            (Self::OilPlatform, MovementType::Chess) => None,
            (Self::Hq, _) => Some(MovementPoints::from(1.)),
            (Self::Port(_), _) => Some(MovementPoints::from(1.)),
            (Self::OilPlatform, _) => Some(MovementPoints::from(1.)),
            (
                Self::City |
                Self::Factory(_) |
                Self::Airport(_),
                sea_units!()
            ) => None,
            _ => Some(MovementPoints::from(1.))
        }
    }
    pub fn like_beach_for_hovercraft(&self) -> bool {
        match self {
            Self::Hq => true,
            Self::Port(_) => true,
            Self::OilPlatform => true,
            _ => false,
        }
    }
    pub fn after_buying<D: Direction>(&self, pos: Point, handler: &mut EventHandler<D>) {
        match self {
            Self::Factory(built_this_turn) |
            Self::Airport(built_this_turn) |
            Self::ConstructionSite(built_this_turn) |
            Self::Port(built_this_turn) => {
                if **built_this_turn < MAX_BUILT_THIS_TURN {
                    handler.add_event(Event::UpdateBuiltThisTurn(pos, *built_this_turn, BuiltThisTurn::new(**built_this_turn + 1)));
                }
            }
            _ => {}
        }
    }
}

pub fn build_options_factory<D: Direction>(_game: &Game<D>, owner: Owner, built_this_turn: u8) -> Vec<(UnitType<D>, u16)> {
    let mut result: Vec<(UnitType<D>, u16)> = NormalUnits::list()
    .iter()
    .filter(|u| u.repairable_factory())
    .map(|u| {
        let value = u.value() + 300 * built_this_turn as u16;
        let unit = UnitType::Normal(NormalUnit::new_instance(u.clone(), owner));
        (unit, value)
    }).collect();
    result.sort_by(|v1, v2| v1.1.cmp(&v2.1));
    result
}

pub fn build_options_port<D: Direction>(_game: &Game<D>, owner: Owner, built_this_turn: u8) -> Vec<(UnitType<D>, u16)> {
    let mut result: Vec<(UnitType<D>, u16)> = NormalUnits::list()
    .iter()
    .filter(|u| u.repairable_port())
    .map(|u| {
        let value = u.value() + 300 * built_this_turn as u16;
        let unit = UnitType::Normal(NormalUnit::new_instance(u.clone(), owner));
        (unit, value)
    }).collect();
    result.sort_by(|v1, v2| v1.1.cmp(&v2.1));
    result
}

pub fn build_options_airport<D: Direction>(_game: &Game<D>, owner: Owner, built_this_turn: u8) -> Vec<(UnitType<D>, u16)> {
    let mut result: Vec<(UnitType<D>, u16)> = NormalUnits::list()
    .iter()
    .filter(|u| u.repairable_airport())
    .map(|u| {
        let value = u.value() + 300 * built_this_turn as u16;
        let unit = UnitType::Normal(NormalUnit::new_instance(u.clone(), owner));
        (unit, value)
    }).collect();
    result.sort_by(|v1, v2| v1.1.cmp(&v2.1));
    result
}

pub fn build_options_construction_site<D: Direction>(_game: &Game<D>, owner: Owner, built_this_turn: u8) -> Vec<(UnitType<D>, u16)> {
    let mut list = vec![
        Structures::ShockTower(Some(owner)),
        Structures::DroneTower(Some((owner, LVec::new(), DroneId::new(0))))
    ];
    for d in D::list() {
        list.push(Structures::MegaCannon(Some(owner), d));
        list.push(Structures::LaserCannon(Some(owner), d));
    }
    let mut result: Vec<(UnitType<D>, u16)> = list.into_iter()
    .map(|u| {
        let value = u.value() + 300 * built_this_turn as u16;
        let unit = UnitType::Structure(Structure::new_instance(u.clone()));
        (unit, value)
    }).collect();
    result.sort_by(|v1, v2| v1.1.cmp(&v2.1));
    result
}

