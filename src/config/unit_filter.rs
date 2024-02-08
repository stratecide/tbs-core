use std::collections::HashSet;
use std::str::FromStr;

use crate::map::map::Map;
use crate::map::point::Point;
use crate::terrain::TerrainType;
use crate::units::hero::Hero;
use crate::units::movement::{MovementType, TBallast};
use crate::units::unit::Unit;
use crate::units::unit_types::UnitType;
use crate::map::direction::Direction;

use super::movement_type_config::MovementPattern;
use super::ConfigParseError;
use super::config::Config;



#[derive(Debug, Clone)]
pub(super) enum UnitTypeFilter {
    Unit(HashSet<UnitType>),
    MovementPattern(HashSet<MovementPattern>),
}

impl FromStr for UnitTypeFilter {
    type Err = ConfigParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let it = s.split(')').next().unwrap().trim();
        let mut it = it.split(&['(', ' ']);
        Ok(match it.next().unwrap() {
            "U" | "Unit" => {
                let mut set = HashSet::new();
                for s in it {
                    set.insert(s.parse()?);
                }
                if set.len() == 0 {
                    return Err(ConfigParseError::EmptyList);
                }
                Self::Unit(set)
            }
            "MP" | "MovementPattern" => {
                let mut set = HashSet::new();
                for s in it {
                    set.insert(s.parse()?);
                }
                if set.len() == 0 {
                    return Err(ConfigParseError::EmptyList);
                }
                Self::MovementPattern(set)
            }
            _ => return Err(ConfigParseError::UnknownEnumMember(s.to_string()))
        })
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
 * UnitFilter is the first thing to replace with Rhai
 */
#[derive(Debug, Clone)]
pub(super) enum UnitFilter {
    Unit(HashSet<UnitType>),
    Movement(HashSet<MovementType>),
    Terrain(HashSet<TerrainType>),
    MovementPattern(HashSet<MovementPattern>),
}

impl FromStr for UnitFilter {
    type Err = ConfigParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut it = s.split(&['(', ' ', ')'])
        .map(str::trim)
        .filter(|s| s.len() > 0);
        Ok(match it.next().unwrap() {
            "Unit" | "U" => {
                let mut set = HashSet::new();
                for unit in it {
                    set.insert(unit.parse()?);
                }
                Self::Unit(set)
            }
            "Movement" | "M" => {
                let mut set = HashSet::new();
                for movement_type in it {
                    set.insert(movement_type.parse()?);
                }
                Self::Movement(set)
            }
            "Terrain" | "T" => {
                let mut set = HashSet::new();
                for terrain in it {
                    set.insert(terrain.parse()?);
                }
                Self::Terrain(set)
            }
            "MP" | "MovementPattern" => {
                let mut set = HashSet::new();
                for s in it {
                    set.insert(s.parse()?);
                }
                if set.len() == 0 {
                    return Err(ConfigParseError::EmptyList);
                }
                Self::MovementPattern(set)
            }
            invalid => return Err(ConfigParseError::UnknownEnumMember(invalid.to_string())),
        })
    }
}

impl UnitFilter {
    pub fn check<D: Direction>(
        &self,
        map: &Map<D>,
        unit: &Unit<D>,
        unit_pos: (Point, Option<usize>),
        // when moving out of a transporter, or start_turn for transported units
        transporter: Option<(&Unit<D>, Point)>,
        // the attacked unit, the unit this one was destroyed by, ...
        other_unit: Option<(&Unit<D>, Point)>,
        // the heroes affecting this unit. shouldn't be taken from game since they could have died before this function is called
        heroes: &[&(Unit<D>, Hero, Point, Option<usize>)],
        // empty if the unit hasn't moved
        temporary_ballast: &[TBallast<D>],
    ) -> bool {
        match self {
            Self::Unit(u) => u.contains(&unit.typ()),
            Self::Movement(m) => m.contains(&unit.default_movement_type()),
            Self::Terrain(t) => t.contains(&map.get_terrain(unit_pos.0).unwrap().typ()),
            Self::MovementPattern(m) => m.contains(&unit.movement_pattern()),
        }
    }
}
