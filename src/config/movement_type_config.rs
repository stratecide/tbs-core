use crate::map::direction::Direction;
use crate::map::map_view::MapView;
use crate::map::point::Point;
use crate::map::wrapping_map::Distortion;
use crate::terrain::terrain::Terrain;
use crate::terrain::ExtraMovementOptions;
use crate::units::movement::{PathStep, PathStepTakes, PbEntry, TBallast};

/*#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct MovementTypeConfig {
    allow_loops: bool,
    temporary: TemporaryBallast<D: Direction>,
    permanent: PermanentBallast<D: Direction>,
}*/

crate::listable_enum! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
}

impl MovementPattern {
    pub fn can_pass_friendly(&self) -> bool {
        match self {
            Self::Standard |
            Self::StandardLoopLess => true,
            _ => false,
        }
    }

    pub fn add_temporary_ballast<D: Direction>(&self, terrain: &Terrain, permanent_ballast: &[PbEntry<D>], temporary_ballast: &mut Vec<TBallast<D>>) {
        match self {
            MovementPattern::Standard |
            MovementPattern::StandardLoopLess => temporary_ballast.push(TBallast::ForbiddenDirection(None)),
            MovementPattern::Straight => temporary_ballast.push(TBallast::Direction(None)),
            MovementPattern::Diagonal => temporary_ballast.push(TBallast::DiagonalDirection(None)),
            MovementPattern::Pawn => {
                let mut dir = None;
                if terrain.is_chess() {
                    for t in permanent_ballast {
                        if let PbEntry::PawnDirection(d) = t {
                            dir = Some(*d);
                            break;
                        }
                    }
                    if dir.is_none() {
                        panic!("Pawn Permanent missing PawnDirection: {permanent_ballast:?}");
                    }
                };
                temporary_ballast.push(TBallast::Direction(dir));
            }
            MovementPattern::None => (),
            MovementPattern::Knight => {
            }
            MovementPattern::Rays => {
                temporary_ballast.push(TBallast::Direction(None));
                temporary_ballast.push(TBallast::DiagonalDirection(None));
            }
        }
    }

    pub fn find_steps<D: Direction>(&self, _map: &impl MapView<D>, _point: Point, step_id: usize, _permanent_ballast: &[PbEntry<D>], temporary_ballast: &[TBallast<D>], extra_movement_options: ExtraMovementOptions) -> Vec<PathStep<D>> {
        let mut result = Vec::new();
        let add_dir = |result: &mut Vec<_>, d: D| {
            result.push(PathStep::Dir(d));
            match extra_movement_options {
                ExtraMovementOptions::Jump => result.push(PathStep::Jump(d)),
                ExtraMovementOptions::None => (),
            }
        };
        match self {
            Self::None => (),
            Self::Standard |
            Self::StandardLoopLess => {
                for d in D::list() {
                    add_dir(&mut result, d);
                }
            }
            Self::Straight => {
                for d in D::list() {
                    add_dir(&mut result, d);
                }
            }
            Self::Diagonal => {
                for d in D::list() {
                    result.push(PathStep::Diagonal(d));
                }
            }
            Self::Rays => {
                for d in D::list() {
                    add_dir(&mut result, d);
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
                let mut result = Vec::new();
                match temporary_ballast.get(0) {
                    Some(TBallast::Direction(Some(d))) => {
                        result.push(PathStep::Dir(*d));
                        if step_id == 0 {
                            result.push(PathStep::Diagonal(*d));
                            result.push(PathStep::Diagonal(d.rotate(true)));
                        }
                    }
                    _ => {
                        for d in D::list() {
                            result.push(PathStep::Dir(d));
                            if step_id == 0 {
                                result.push(PathStep::Diagonal(d));
                            }
                        }
                    }
                }
                return result;
            }
        };
        result.into_iter()
        .filter(|step| {
            temporary_ballast.iter().all(|temp| {
                match (temp, step) {
                    (TBallast::ForbiddenDirection(Some(d1)), PathStep::Dir(d2)) => d1 != d2,
                    (TBallast::Direction(Some(d1)), PathStep::Dir(d2)) => d1 == d2,
                    (TBallast::Direction(Some(_)), _) => false,
                    (TBallast::DiagonalDirection(Some(d1)), PathStep::Diagonal(d2)) => d1 == d2,
                    (TBallast::DiagonalDirection(Some(_)), _) => false,
                    //(TBallast::StepCount(step_count), _) => *step_count > 0,
                    _ => true
                }
            })
        }).collect()
    }

    pub fn update_temporary_ballast<D: Direction>(&self, step: &PathStep<D>, step_distortion: Distortion<D>, temporary_ballast: &mut [TBallast<D>]) {
        for p in temporary_ballast {
            match p {
                TBallast::Direction(dir) => {
                    *dir = step.dir().map(|d| step_distortion.update_direction(d));
                }
                TBallast::DiagonalDirection(dir) => {
                    *dir = step.diagonal_dir().map(|d| step_distortion.update_diagonal_direction(d));
                }
                TBallast::ForbiddenDirection(dir) => {
                    *dir = step.dir().map(|d| step_distortion.update_direction(d.opposite_direction()));
                }
                TBallast::MovementPoints(_mp) => {
                    panic!("should already have been handled in movement.rs");
                }
                /*TBallast::StepCount(step_count) => {
                    TBallast::StepCount(*step_count - 1)
                }*/
                TBallast::Takes(takes) => {
                    match self {
                        Self::None |
                        Self::Standard |
                        Self::StandardLoopLess => *takes = PathStepTakes::Deny,
                        Self::Straight |
                        Self::Diagonal |
                        Self::Knight |
                        Self::Rays => *takes = PathStepTakes::Allow,
                        Self::Pawn => {
                            match step {
                                PathStep::Diagonal(_) => *takes = PathStepTakes::Force,
                                _ => *takes = PathStepTakes::Deny,
                            }
                        }
                    }
                }
            }
        }
    }
}
