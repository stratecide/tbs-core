use std::collections::HashSet;
use std::str::FromStr;

use serde::Deserialize;

use crate::game::game::Game;
use crate::map::point::Point;
use crate::terrain::TerrainType;
use crate::units::movement::MovementType;
use crate::units::unit::Unit;
use crate::units::unit_types::UnitType;
use crate::map::direction::Direction;

use super::ConfigParseError;
use super::config::Config;



#[derive(Debug, Clone)]
pub(super) enum UnitTypeFilter {
    Unit(HashSet<UnitType>),
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
            _ => return Err(ConfigParseError::UnknownEnumMember(s.to_string()))
        })
    }
}

impl UnitTypeFilter {
    pub fn check(&self, _config: &Config, unit_type: UnitType) -> bool {
        match self {
            Self::Unit(u) => u.contains(&unit_type),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub(super) enum UnitFilter {
    Unit(HashSet<UnitType>),
    Movement(HashSet<MovementType>),
    Terrain(HashSet<TerrainType>),
}

impl FromStr for UnitFilter {
    type Err = ConfigParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut it = s.split(&['(', ' ', ')'])
        .map(str::trim);
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
                for unit in it {
                    set.insert(unit.parse()?);
                }
                Self::Movement(set)
            }
            "Terrain" | "T" => {
                let mut set = HashSet::new();
                for unit in it {
                    set.insert(unit.parse()?);
                }
                Self::Terrain(set)
            }
            invalid => return Err(ConfigParseError::UnknownEnumMember(invalid.to_string())),
        })
    }
}

impl UnitFilter {
    pub fn check<D: Direction>(&self, _config: &Config, game: &Game<D>, unit: &Unit<D>, unit_pos: Point, hero: Option<(&Unit<D>, Point)>, other_unit: Option<(&Unit<D>, Point)>) -> bool {
        match self {
            Self::Unit(u) => u.contains(&unit.typ()),
            Self::Movement(m) => m.contains(&unit.default_movement_type()),
            Self::Terrain(t) => t.contains(&game.get_map().get_terrain(unit_pos).unwrap().typ()),
        }
    }
}
