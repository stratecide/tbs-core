use std::collections::HashSet;

use zipper::*;
use zipper::zipper_derive::*;

use crate::{player::*, map::{direction::Direction, point::Point}, game::game::Game};
use crate::units::normal_units::{NormalUnits, NormalUnit};
use crate::units::movement::MovementType;
use crate::units::UnitType;
use crate::units::normal_trait::NormalUnitTrait;

#[derive(Debug, PartialEq, Clone, Zippable)]
#[zippable(bits = 8)]
pub enum Terrain<D: Direction> {
    Grass,
    Street,
    Realty(Realty, Option::<Owner>),
    Fountain,
    Pipe(D::P),
    ChessTile,
}
impl<D: Direction> Terrain<D> {
    pub fn movement_cost(&self, movement_type: &MovementType) -> Option<u8> {
        match self {
            Self::Grass => match movement_type {
                MovementType::Foot | MovementType::Treads => Some(6),
                MovementType::Heli => Some(6),
                MovementType::Hover => Some(6),
                MovementType::Wheel => Some(9),
                MovementType::Chess => Some(6),
            }
            Self::Street => Some(6),
            Self::Realty(_, _) => Some(6),
            Self::Fountain => match movement_type {
                MovementType::Heli => Some(6),
                MovementType::Hover => Some(6),
                _ => None,
            }
            Self::Pipe(_) => None,
            Self::ChessTile => match movement_type {
                MovementType::Chess => Some(0),
                _ => Some(6)
            }
        }
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
        match (self, u.get_movement().0) {
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
}

pub fn build_options_factory<D: Direction>(_game: &Game<D>, owner: Owner, built_this_turn: u8) -> Vec<(UnitType<D>, u16)> {
    let units = vec![
        NormalUnits::Hovercraft,
        NormalUnits::DragonHead,
        NormalUnits::Artillery,
    ];
    units.into_iter().map(|u| {
        let value = u.value() + 300 * built_this_turn as u16;
        let unit = UnitType::Normal(NormalUnit::new_instance(u, owner));
        (unit, value)
    }).collect()
}
