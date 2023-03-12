use std::collections::HashSet;

use zipper::*;
use zipper::zipper_derive::*;

use crate::{player::*, map::{direction::Direction, point::Point}, game::{game::Game, events::{EventHandler, Event}}, units::normal_units::DroneId};
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
            (Self::Grass, MovementType::Hover(HoverMode::Beach)) => Some(1.),
            (Self::Grass, MovementType::Wheel) => Some(1.5),
            (Self::Grass, land_units!()) => Some(1.),
            (Self::Grass, air_units!()) => Some(1.),
            
            (Self::Forest, sea_units!()) => None,
            (Self::Forest, MovementType::Hover(HoverMode::Beach)) => Some(1.5),
            (Self::Forest, MovementType::Foot) => Some(1.),
            (Self::Forest, MovementType::Wheel) => Some(2.),
            (Self::Forest, land_units!()) => Some(1.5),
            (Self::Forest, air_units!()) => Some(1.),

            (Self::Mountain, sea_units!()) => None,
            (Self::Mountain, MovementType::Foot) => Some(1.5),
            (Self::Mountain, MovementType::Heli) => Some(1.5),
            (Self::Mountain, MovementType::Plane) => Some(1.),
            (Self::Mountain,
                MovementType::Hover(_) |
                MovementType::Wheel |
                MovementType::Treads |
                MovementType::Chess) => None,

            (Self::Sea, land_units!()) => None,
            (Self::Sea, _) => Some(1.),

            (Self::Beach, MovementType::Chess) => None,
            (Self::Beach, MovementType::Ship) => None,
            (Self::Beach, _) => Some(1.),

            (Self::Reef, land_units!()) => None,
            (Self::Reef, _) => Some(1.),

            (Self::Street, sea_units!()) => None,
            (Self::Street, _) => Some(1.),

            (Self::Ruins, sea_units!()) => None,
            (Self::Ruins, _) => Some(1.),

            (Self::Bridge, MovementType::Chess) => None,
            (Self::Bridge, _) => Some(1.),
                
            (Self::Flame, _) => None,
            
            (Self::Realty(realty, _), movement_type) => return realty.movement_cost(movement_type),

            (Self::Tavern, MovementType::Chess) => None,
            (Self::Tavern, _) => Some(1.),

            (Self::Fountain, land_units!()) => None,
            (Self::Fountain, sea_units!()) => Some(1.),
            (Self::Fountain, MovementType::Hover(HoverMode::Beach)) => Some(1.),
            (Self::Fountain, MovementType::Heli) => Some(1.5),
            (Self::Fountain, MovementType::Plane) => Some(1.),

            (Self::Pipe(_), _) => None,

            (Self::ChessTile, sea_units!()) => None,
            (Self::ChessTile, _) => Some(1.),
        }.and_then(|v| Some((v * 6.) as u8))
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
            (Self::Forest, land_units!()) => 0.3,
            (Self::Realty(_, _), land_units!()) => 0.2,
            (Self::Ruins, land_units!()) => 0.2,
            (_, _) => 0.,
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


const MAX_BUILT_THIS_TURN: u8 = 9;
pub type BuiltThisTurn = U8<{MAX_BUILT_THIS_TURN}>;

#[derive(Debug, PartialEq, Clone, Zippable)]
#[zippable(bits = 6)]
pub enum Realty {
    Hq,
    City,
    Factory(BuiltThisTurn),
    Port(BuiltThisTurn),
    Airport(BuiltThisTurn),
}
impl Realty {
    pub fn income_factor(&self) -> i16 {
        match self {
            Self::City => 1,
            _ => 0,
        }
    }
    pub fn can_build(&self) -> bool {
        match self {
            Self::Factory(_) => true,
            Self::Airport(_) => true,
            Self::Port(_) => true,
            _ => false
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
    pub fn can_repair(&self, unit_type: &NormalUnits) -> bool {
        match self {
            Self::Factory(_) | Self::City => unit_type.repairable_factory(),
            Self::Port(_) => unit_type.repairable_port(),            
            Self::Airport(_) => unit_type.repairable_airport(),            
            _ => false,
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
    pub fn after_buying<D: Direction>(&self, pos: Point, handler: &mut EventHandler<D>) {
        match self {
            Self::Factory(built_this_turn) |
            Self::Airport(built_this_turn) |
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
    NormalUnits::list()
    .iter()
    .filter(|u| u.repairable_factory())
    .map(|u| {
        let value = u.value() + 300 * built_this_turn as u16;
        let unit = UnitType::Normal(NormalUnit::new_instance(u.clone(), owner));
        (unit, value)
    }).collect()
}

pub fn build_options_port<D: Direction>(_game: &Game<D>, owner: Owner, built_this_turn: u8) -> Vec<(UnitType<D>, u16)> {
    NormalUnits::list()
    .iter()
    .filter(|u| u.repairable_port())
    .map(|u| {
        let value = u.value() + 300 * built_this_turn as u16;
        let unit = UnitType::Normal(NormalUnit::new_instance(u.clone(), owner));
        (unit, value)
    }).collect()
}

pub fn build_options_airport<D: Direction>(_game: &Game<D>, owner: Owner, built_this_turn: u8) -> Vec<(UnitType<D>, u16)> {
    NormalUnits::list()
    .iter()
    .filter(|u| u.repairable_airport())
    .map(|u| {
        let value = u.value() + 300 * built_this_turn as u16;
        let unit = UnitType::Normal(NormalUnit::new_instance(u.clone(), owner));
        (unit, value)
    }).collect()
}
