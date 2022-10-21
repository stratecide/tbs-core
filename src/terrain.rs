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
        MovementType::Treads |
        MovementType::Chess
    };
}

macro_rules! sea_units {
    () => {
        MovementType::Hover(HoverMode::Sea)
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
    Beach,
    Bridge,
    ChessTile,
    Flame,
    Forest,
    Fountain,
    Grass,
    Mountain,
    Pipe(D::P),
    Realty(Realty, Option::<Owner>),
    Reef,
    Ruins,
    Sea,
    Street,
    Tavern,
}
impl<D: Direction> Terrain<D> {
    pub fn movement_cost(&self, movement_type: MovementType) -> Option<u8> {
        match (self, movement_type) {
            (Self::Grass, sea_units!()) => None,
            (Self::Grass, MovementType::Hover(HoverMode::Beach)) => Some(6),
            (Self::Grass, MovementType::Wheel) => Some(9),
            (Self::Grass, land_units!()) => Some(6),
            (Self::Grass, air_units!()) => Some(6),
            
            (Self::Forest, sea_units!()) => None,
            (Self::Forest, MovementType::Hover(HoverMode::Beach)) => Some(9),
            (Self::Forest, MovementType::Foot) => Some(6),
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

            (Self::Ruins, sea_units!()) => None,
            (Self::Ruins, _) => Some(6),

            (Self::Bridge, MovementType::Chess) => None,
            (Self::Bridge, _) => Some(6),
                
            (Self::Flame, _) => None,
            
            (Self::Realty(realty, _), movement_type) => realty.movement_cost(movement_type),

            (Self::Tavern, MovementType::Chess) => None,
            (Self::Tavern, _) => Some(6),

            (Self::Fountain, land_units!()) => None,
            (Self::Fountain, sea_units!()) => Some(6),
            (Self::Fountain, MovementType::Hover(HoverMode::Beach)) => Some(6),
            (Self::Fountain, MovementType::Heli) => Some(9),

            (Self::Pipe(_), _) => None,

            (Self::ChessTile, sea_units!()) => None,
            (Self::ChessTile, _) => Some(6),
        }
    }
    pub fn like_beach_for_hovercraft(&self) -> bool {
        match self {
            Self::Beach => true,
            Self::Realty(realty, _) => realty.like_beach_for_hovercraft(),
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
                        Self::Mountain => HoverMode::Land,
                        Self::Pipe(_) => mode,
                        Self::Realty(_, _) => HoverMode::Land,
                        Self::Reef => HoverMode::Sea,
                        Self::Ruins => HoverMode::Land,
                        Self::Sea => HoverMode::Sea,
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
    pub fn defense(&self, unit: &UnitType<D>) -> f32 {
        let movement_type = match unit {
            UnitType::Normal(unit) => {
                unit.get_movement(self).0
            }
            UnitType::Mercenary(unit) => {
                unit.get_movement(self).0
            }
            UnitType::Chess(_) => {
                MovementType::Chess
            }
            UnitType::Structure(_) => return 1.0,
        };
        match (self, movement_type) {
            (Self::Grass, land_units!()) => 1.1,
            (Self::Forest, land_units!()) => 1.3,
            (Self::Realty(_, _), land_units!()) => 1.2,
            (Self::Ruins, land_units!()) => 1.2,
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
    Port(U8::<9>),
    Airport(U8::<9>),
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
            Self::Port(built_this_turn) => build_options_port(game, owner, **built_this_turn),
            Self::Airport(built_this_turn) => build_options_airport(game, owner, **built_this_turn),
            _ => vec![],
        }
    }
    pub fn movement_cost(&self, movement_type: MovementType) -> Option<u8> {
        match (self, movement_type) {
            (Self::Port(_), MovementType::Chess) => None,
            (Self::Port(_), _) => Some(6),

            (
                Self::Hq |
                Self::City |
                Self::Factory(_) |
                Self::Airport(_),
                sea_units!()
            ) => None,
            _ => Some(6)
        }
    }
    pub fn like_beach_for_hovercraft(&self) -> bool {
        match self {
            Self::Port(_) => true,
            _ => false,
        }
    }
}

pub fn build_options_factory<D: Direction>(_game: &Game<D>, owner: Owner, built_this_turn: u8) -> Vec<(UnitType<D>, u16)> {
    let units = vec![
        NormalUnits::Hovercraft(false),
        NormalUnits::DragonHead,
        NormalUnits::Magnet,
        NormalUnits::Artillery,
    ];
    units.into_iter().map(|u| {
        let value = u.value() + 300 * built_this_turn as u16;
        let unit = UnitType::Normal(NormalUnit::new_instance(u, owner));
        (unit, value)
    }).collect()
}

pub fn build_options_port<D: Direction>(_game: &Game<D>, owner: Owner, built_this_turn: u8) -> Vec<(UnitType<D>, u16)> {
    let units = vec![
        NormalUnits::Hovercraft(true),
    ];
    units.into_iter().map(|u| {
        let value = u.value() + 300 * built_this_turn as u16;
        let unit = UnitType::Normal(NormalUnit::new_instance(u, owner));
        (unit, value)
    }).collect()
}

pub fn build_options_airport<D: Direction>(_game: &Game<D>, owner: Owner, built_this_turn: u8) -> Vec<(UnitType<D>, u16)> {
    let units = vec![
        NormalUnits::TransportHeli(LVec::new()),
    ];
    units.into_iter().map(|u| {
        let value = u.value() + 300 * built_this_turn as u16;
        let unit = UnitType::Normal(NormalUnit::new_instance(u, owner));
        (unit, value)
    }).collect()
}
