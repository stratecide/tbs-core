use std::collections::HashSet;

use serde::Deserialize;

use crate::game::game::Game;
use crate::map::point::Point;
use crate::terrain::TerrainType;
use crate::units::movement::MovementType;
use crate::units::unit::Unit;
use crate::units::unit_types::UnitType;
use crate::map::direction::Direction;

use super::Config;



#[derive(Debug, Clone, Default, Deserialize)]
pub(super) enum UnitTypeFilter {
    #[default]
    All,
    Unit(HashSet<UnitType>),
}

impl UnitTypeFilter {
    pub fn check(&self, config: &Config, unit_type: UnitType) -> bool {
        match self {
            Self::All => true,
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

impl UnitFilter {
    pub fn check<D: Direction>(&self, config: &Config, unit: &Unit<D>, game: &Game<D>, pos: Point) -> bool {
        match self {
            Self::Unit(u) => u.contains(&unit.typ()),
            Self::Movement(m) => m.contains(&unit.default_movement_type()),
            Self::Terrain(t) => t.contains(&game.get_map().get_terrain(pos).unwrap().typ()),
        }
    }
}
