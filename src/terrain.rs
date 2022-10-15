use std::collections::HashSet;

use zipper::*;
use zipper::zipper_derive::*;

use crate::{player::*, map::{direction::Direction, point::Point}, game::game::Game};
use crate::units::normal_units::{NormalUnits, NormalUnit};
use crate::units::movement::*;
use crate::units::UnitType;
use crate::units::normal_trait::NormalUnitTrait;


macro_rules! land_units {
    () => {
        MovementType::Foot |
        MovementType::Wheel |
        MovementType::Hover(HoverMode::Land) |
        MovementType::Hover(HoverMode::Beach) |
        MovementType::Treads |
        MovementType::Chess
    };
}

macro_rules! sea_units {
    () => {
        MovementType::Hover(HoverMode::Sea) |
        MovementType::Hover(HoverMode::Beach)
    };
}

macro_rules! air_units {
    () => {
        MovementType::Heli
    };
}


#[derive(Debug, PartialEq, Clone, Zippable)]
#[zippable(bits = 8)]
pub enum Terrain<D: Direction> {
    Grass,
    Forest,
    Mountain,
    Sea,
    Beach,
    Reef,
    Street,
    Bridge,
    Flame,
    Realty(Realty, Option::<Owner>),
    Fountain,
    Pipe(D::P),
    ChessTile,
}
impl<D: Direction> Terrain<D> {
    pub fn movement_cost(&self, movement_type: MovementType) -> Option<u8> {
        match (self, movement_type) {
            (Self::Grass, sea_units!()) => None,
            (Self::Grass, MovementType::Wheel) => Some(9),
            (Self::Grass, land_units!()) => Some(6),
            (Self::Grass, air_units!()) => Some(6),
            
            (Self::Forest, sea_units!()) => None,
            (Self::Forest, MovementType::Foot) => Some(9),
            (Self::Forest, land_units!()) => Some(9),
            (Self::Forest, air_units!()) => Some(6),

            (Self::Mountain, sea_units!()) => None,
            (Self::Mountain, MovementType::Foot) => Some(9),
            (Self::Mountain, MovementType::Heli) => Some(9),
            (Self::Mountain,
                MovementType::Hover(_) |
                MovementType::Wheel |
                MovementType::Treads |
                MovementType::Chess) => None,

            (Self::Sea, land_units!()) => None,
            (Self::Sea, _) => Some(6),

            (Self::Beach, MovementType::Chess) => None,
            (Self::Beach, _) => Some(6),

            (Self::Reef, land_units!()) => None,
            (Self::Reef, _) => Some(6),

            (Self::Street, sea_units!()) => None,
            (Self::Street, _) => Some(6),

            (Self::Bridge, MovementType::Chess) => None,
            (Self::Bridge, _) => Some(6),
                
            (Self::Flame, _) => None,
            
            (Self::Realty(realty, _), movement_type) => realty.movement_cost(movement_type),

            (Self::Fountain, land_units!()) => None,
            (Self::Fountain, sea_units!()) => Some(6),
            (Self::Fountain, MovementType::Heli) => Some(9),

            (Self::Pipe(_), _) => None,

            (Self::ChessTile, sea_units!()) => None,
            (Self::ChessTile, _) => Some(6),
        }
    }
    pub fn like_beach_for_hovercraft(&self) -> bool {
        match self {
            Self::Beach => true,
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
                        Self::Mountain => HoverMode::Land,
                        Self::Pipe(_) => mode,
                        Self::Realty(_, _) => HoverMode::Land,
                        Self::Reef => HoverMode::Sea,
                        Self::Sea => HoverMode::Sea,
                        Self::Street => HoverMode::Land,
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
    pub fn defense(&self, unit: &UnitType<D>) -> f32 {
        let u: &dyn NormalUnitTrait<D> = match unit {
            UnitType::Normal(unit) => {
                unit
            }
            UnitType::Mercenary(unit) => {
                unit
            }
            UnitType::Chess(_) => {
                return match self {
                    Self::Grass => 1.1,
                    _ => 1.,
                };
            }
            UnitType::Structure(_) => return 1.0,
        };
        match (self, u.get_movement(self).0) {
            (Self::Grass, MovementType::Foot) => 1.1,
            (_, _) => 1.,
        }
    }
    pub fn requires_true_sight(&self) -> bool {
        false
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
            Terrain::Realty(_, owner) => {
                if let Some(player) = owner.and_then(|owner| game.get_owning_player(&owner)) {
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
            Terrain::Realty(realty, _) => Terrain::Realty(realty.clone(), None),
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


#[derive(Debug, PartialEq, Clone, Zippable)]
#[zippable(bits = 6)]
pub enum Realty {
    Hq,
    City,
    Factory(U8::<9>),
}
impl Realty {
    pub fn income_factor(&self) -> i16 {
        match self {
            Self::City => 1,
            _ => 0,
        }
    }
    pub fn buildable_units<D: Direction>(&self, game: &Game<D>, owner: Owner) -> Vec<(UnitType<D>, u16)> {
        match self {
            Self::Factory(built_this_turn) => build_options_factory(game, owner, **built_this_turn),
            _ => vec![],
        }
    }
    pub fn movement_cost(&self, movement_type: MovementType) -> Option<u8> {
        match (self, movement_type) {
            (
                Self::Hq |
                Self::City |
                Self::Factory(_)
                ,
                MovementType::Hover(HoverMode::Sea)
            ) => None,
            _ => Some(6),
        }
    }
}

pub fn build_options_factory<D: Direction>(_game: &Game<D>, owner: Owner, built_this_turn: u8) -> Vec<(UnitType<D>, u16)> {
    let units = vec![
        NormalUnits::Hovercraft(false),
        NormalUnits::DragonHead,
        NormalUnits::Artillery,
    ];
    units.into_iter().map(|u| {
        let value = u.value() + 300 * built_this_turn as u16;
        let unit = UnitType::Normal(NormalUnit::new_instance(u, owner));
        (unit, value)
    }).collect()
}
