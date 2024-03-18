use std::collections::HashSet;

use crate::game::game_view::GameView;
use crate::map::point::Point;
use crate::terrain::TerrainType;
use crate::units::hero::{Hero, HeroType};
use crate::units::movement::{MovementType, TBallast};
use crate::units::unit::Unit;
use crate::units::unit_types::UnitType;
use crate::map::direction::Direction;

use super::movement_type_config::MovementPattern;
use super::parse::{parse_inner_vec, string_base, FromConfig};
use super::ConfigParseError;
use super::config::Config;



#[derive(Debug, Clone)]
pub(super) enum UnitTypeFilter {
    Unit(HashSet<UnitType>),
    MovementPattern(HashSet<MovementPattern>),
}

impl FromConfig for UnitTypeFilter {
    fn from_conf(s: &str) -> Result<(Self, &str), ConfigParseError> {
        let (base, mut remainder) = string_base(s);
        Ok((match base {
            "U" | "Unit" => {
                let (list, r) = parse_inner_vec::<UnitType>(remainder, true)?;
                remainder = r;
                Self::Unit(list.into_iter().collect())
            }
            "MP" | "MovementPattern" => {
                let (list, r) = parse_inner_vec::<MovementPattern>(remainder, true)?;
                remainder = r;
                Self::MovementPattern(list.into_iter().collect())
            }
            _ => return Err(ConfigParseError::UnknownEnumMember(s.to_string()))
        }, remainder))
    }
}

impl UnitTypeFilter {
    pub fn check(&self, config: &Config, unit_type: UnitType) -> bool {
        match self {
            Self::Unit(u) => u.contains(&unit_type),
            Self::MovementPattern(m) => m.contains(&config.movement_pattern(unit_type)),
        }
    }
}


/**
 * UnitFilter and custom actions are the first things to replace with Rhai
 */
#[derive(Debug, Clone)]
pub(crate) enum UnitFilter {
    Unit(HashSet<UnitType>),
    Movement(HashSet<MovementType>),
    Terrain(HashSet<TerrainType>),
    MovementPattern(HashSet<MovementPattern>),
    Hero(HashSet<(HeroType, Option<u8>)>),
    HeroGlobal(HashSet<(HeroType, Option<u8>)>),
    IsHero(HashSet<(HeroType, Option<u8>)>),
    Not(Vec<Self>),
}

impl FromConfig for UnitFilter {
    fn from_conf(s: &str) -> Result<(Self, &str), ConfigParseError> {
        let (base, mut remainder) = string_base(s);
        Ok((match base {
            "Unit" | "U" => {
                let (list, r) = parse_inner_vec::<UnitType>(remainder, true)?;
                remainder = r;
                Self::Unit(list.into_iter().collect())
            }
            "Movement" | "M" => {
                let (list, r) = parse_inner_vec::<MovementType>(remainder, true)?;
                remainder = r;
                Self::Movement(list.into_iter().collect())
            }
            "Terrain" | "T" => {
                let (list, r) = parse_inner_vec::<TerrainType>(remainder, true)?;
                remainder = r;
                Self::Terrain(list.into_iter().collect())
            }
            "MP" | "MovementPattern" => {
                let (list, r) = parse_inner_vec::<MovementPattern>(remainder, true)?;
                remainder = r;
                Self::MovementPattern(list.into_iter().collect())
            }
            "H" | "Hero" => {
                let (list, r) = parse_inner_vec::<(HeroType, Option<u8>)>(remainder, true)?;
                remainder = r;
                Self::Hero(list.into_iter().collect())
            }
            "HG" | "HeroGlobal" => {
                let (list, r) = parse_inner_vec::<(HeroType, Option<u8>)>(remainder, true)?;
                remainder = r;
                Self::HeroGlobal(list.into_iter().collect())
            }
            "IH" | "IsHero" => {
                let (list, r) = parse_inner_vec::<(HeroType, Option<u8>)>(remainder, true)?;
                remainder = r;
                Self::IsHero(list.into_iter().collect())
            }
            "Not" => {
                let (list, r) = parse_inner_vec::<Self>(remainder, true)?;
                remainder = r;
                Self::Not(list)
            }
            invalid => return Err(ConfigParseError::UnknownEnumMember(invalid.to_string())),
        }, remainder))
    }
}

impl UnitFilter {
    pub fn check<D: Direction>(
        &self,
        map: &impl GameView<D>,
        unit: &Unit<D>,
        unit_pos: (Point, Option<usize>),
        // when moving out of a transporter, or start_turn for transported units
        transporter: Option<(&Unit<D>, Point)>,
        // the attacked unit, the unit this one was destroyed by, ...
        other_unit: Option<(&Unit<D>, Point)>,
        // the heroes affecting this unit. shouldn't be taken from game since they could have died before this function is called
        heroes: &[(Unit<D>, Hero, Point, Option<usize>)],
        // empty if the unit hasn't moved
        temporary_ballast: &[TBallast<D>],
    ) -> bool {
        match self {
            Self::Unit(u) => u.contains(&unit.typ()),
            Self::Movement(m) => m.contains(&unit.default_movement_type()),
            Self::Terrain(t) => t.contains(&map.get_terrain(unit_pos.0).unwrap().typ()),
            Self::MovementPattern(m) => m.contains(&unit.movement_pattern()),
            Self::Hero(h) => {
                for (_, hero, _, _) in heroes {
                    let power = hero.get_active_power() as u8;
                    if h.iter().any(|h| h.0 == hero.typ() && h.1.unwrap_or(power) == power) {
                        return true;
                    }
                }
                false
            }
            Self::HeroGlobal(h) => {
                for p in map.all_points() {
                    if let Some(u) = map.get_unit(p) {
                        if u.get_owner_id() == unit.get_owner_id() && u.is_hero() {
                            let hero = u.get_hero();
                            let power = hero.get_active_power() as u8;
                            let hero = hero.typ();
                            if h.iter().any(|h| h.0 == hero && h.1.unwrap_or(power) == power) {
                                return true;
                            }
                        }
                    }
                }
                false
            }
            Self::IsHero(h) => {
                let hero = unit.get_hero();
                let power = hero.get_active_power() as u8;
                let hero = hero.typ();
                h.iter().any(|h| h.0 == hero && h.1.unwrap_or(power) == power)
            }
            Self::Not(negated) => {
                // returns true if at least one check returns false
                // if you need all checks to return false, put them into separate Self::Not wrappers instead
                negated.iter()
                .any(|negated| !negated.check(map, unit, unit_pos, transporter, other_unit, heroes, temporary_ballast))
            }
        }
    }
}
