use serde::Deserialize;
use num_rational::Rational32;

use crate::map::direction::Direction;
use crate::map::map::Map;
use crate::map::point::Point;
use crate::terrain::ExtraMovementOptions;
use crate::units::attributes::*;
use crate::units::movement::PathStep;
use crate::units::unit_types::UnitType;

/*#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct MovementTypeConfig {
    allow_loops: bool,
    temporary: TemporaryBallast<D: Direction>,
    permanent: PermanentBallast<D: Direction>,
}*/

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum MovementPattern {
    Standard,
    StandardLoopLess,
    None,
    Straight, // Rook
    Diagonal, // Bishop
    Rays, // Queen
    Knight,
    Pawn,
}

impl MovementPattern {
    pub fn can_pass_friendly(&self) -> bool {
        match self {
            Self::Standard |
            Self::StandardLoopLess => true,
            _ => false,
        }
    }

    pub fn find_steps<D: Direction>(&self, map: &Map<D>, point: Point) -> Vec<PathStep<D>> {
        let mut result = Vec::new();
        let extra_movement_options = map.get_terrain(point).and_then(|t| Some(t.extra_step_options())).unwrap_or(ExtraMovementOptions::None);
        let mut add_dir = |d: D| {
            result.push(PathStep::Dir(d));
            match extra_movement_options {
                ExtraMovementOptions::Jump => {
                    result.push(PathStep::Jump(d));
                }
                _ => (),
            }
        };
        match self {
            Self::None => (),
            Self::Standard |
            Self::StandardLoopLess |
            Self::Straight => {
                for d in D::list() {
                    add_dir(d);
                }
            }
            Self::Diagonal => {
                for d in D::list() {
                    result.push(PathStep::Diagonal(d));
                }
            }
            Self::Rays => {
                for d in D::list() {
                    add_dir(d);
                    result.push(PathStep::Diagonal(d));
                }
            }
            Self::Knight => {
                for d in D::list() {
                    for turn_left in vec![true, false] {
                        result.push(PathStep::Knight(d, turn_left));
                    }
                }
            }
            Self::Pawn => {
                for d in D::list() {
                    result.push(PathStep::Dir(d));
                    result.push(PathStep::Diagonal(d));
                }
            }
        }
        result
    }
}
