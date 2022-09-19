use std::collections::{BinaryHeap, HashSet};
use std::cmp::{Ordering, Reverse};

use zipper::*;
use zipper_derive::*;

use crate::game::events::CommandError;
use crate::map::direction::Direction;
use crate::map::point::Point;
use crate::game::game::Game;
use crate::map::map::*;
use crate::player::Perspective;

use super::{normal_trait::*, chess};

pub enum MovementType {
    Hover,
    Foot,
    Wheel,
    Treads,
    Heli,
    Chess,
}

#[derive(PartialEq, Eq)]
struct WidthSearch<D: Direction> {
    path: Path<D>,
    path_cost: u8,
}
impl<D: Direction> PartialOrd for WidthSearch<D> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.path_cost.cmp(&other.path_cost))
    }
}
impl<D: Direction> Ord for WidthSearch<D> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.path_cost.cmp(&other.path_cost)
    }
}

// callback returns true if the search can be aborted
// if unit is None, units will be ignored
pub fn width_search<D: Direction, F: FnMut(Point, &Path<D>) -> bool>(movement_type: &MovementType, max_cost: u8, game: &Game<D>, start: Point, mut blocked_positions: HashSet<Point>, unit: Option<&dyn NormalUnitTrait<D>>, mut callback: F) {
    if blocked_positions.contains(&start) {
        println!("width_search fail");
    }
    let mut next_checks = BinaryHeap::new();
    let mut add_point = |p: Point, path_so_far: &Path<D>, cost_so_far: u8, next_checks: &mut BinaryHeap<Reverse<WidthSearch<D>>>| {
        if blocked_positions.contains(&p) {
            return false;
        }
        if callback(p, path_so_far) {
            return true;
        }
        blocked_positions.insert(p.clone());
        for (neighbor, step) in game.get_map().get_unit_movement_neighbors(p, movement_type) {
            if !blocked_positions.contains(&neighbor.point) {
                match (unit, game.get_map().get_unit(neighbor.point)) {
                    (Some(mover), Some(other)) => {
                        if !other.can_be_moved_through(mover, game) {
                            continue;
                        }
                    }
                    (_, _) => {}
                }
                if let Some(cost) = game.get_map().get_terrain(neighbor.point).unwrap().movement_cost(movement_type) {
                    if cost_so_far + cost <= max_cost {
                        let mut path = path_so_far.clone();
                        path.steps.push(step).unwrap();
                        next_checks.push(Reverse(WidthSearch{path, path_cost: cost_so_far + cost}));
                    }
                }
            }
        }
        false
    };
    add_point(start, &Path::new(start), 0, &mut next_checks);
    while let Some(Reverse(check)) = next_checks.pop() {
        let finished = add_point(check.path.end(game.get_map()).unwrap(), &check.path, check.path_cost, &mut next_checks);
        if finished {
            break;
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Zippable)]
#[zippable(bits = 3)]
pub enum PathStep<D: Direction> {
    Dir(D),
    Jump(D), // jumps 2 fields, caused by Fountains
    Diagonal(D), // moves diagonally, for chess units
    Knight(D, bool),
    Point(Point),
}
impl<D: Direction> PathStep<D> {
    pub fn progress(&self, map: &Map<D>, pos: Point) -> Result<Point, CommandError> {
        match self {
            Self::Dir(d) => {
                if let Some(o) = map.get_neighbor(pos, *d) {
                    Ok(o.point)
                } else {
                    Err(CommandError::InvalidPath)
                }
            }
            Self::Jump(d) => {
                if let Some(o) = map.get_neighbor(pos, *d).and_then(|o| map.get_neighbor(o.point, o.direction)) {
                    Ok(o.point)
                } else {
                    Err(CommandError::InvalidPath)
                }
            }
            Self::Diagonal(d) => {
                if let Some(o) = chess::get_diagonal_neighbor(map, pos, *d) {
                    Ok(o.point)
                } else {
                    Err(CommandError::InvalidPath)
                }
            }
            Self::Knight(d, turn_left) => {
                if let Some(o) = chess::get_knight_neighbor(map, pos, *d, *turn_left) {
                    Ok(o.point)
                } else {
                    Err(CommandError::InvalidPath)
                }
            }
            Self::Point(p) => Ok(*p),
        }
    }
    pub fn dir(&self) -> Option<D> {
        match self {
            Self::Dir(d) => Some(*d),
            Self::Jump(d) => Some(*d),
            Self::Diagonal(_) => None,
            Self::Knight(_, _) => None,
            Self::Point(_) => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Zippable)]
pub struct Path<D: Direction> {
    pub start: Point,
    pub steps: LVec::<PathStep::<D>, {crate::map::point_map::MAX_AREA}>,
}
impl<D: Direction> Path<D> {
    pub fn new(start: Point) -> Self {
        Self {
            start,
            steps: LVec::new(),
        }
    }

    pub fn end(&self, map: &Map<D>) -> Result<Point, CommandError> {
        let mut current = self.start;
        for step in &self.steps {
            current = step.progress(map, current)?;
        }
        Ok(current)
    }
    
    pub fn points(&self, map: &Map<D>) -> Result<Vec<Point>, CommandError> {
        let mut points = vec![self.start];
        let mut current = self.start;
        for step in self.steps.iter() {
            current = step.progress(map, current)?;
            points.push(current);
        }
        Ok(points)
    }

    pub fn fog_replacement(&self, game: &Game<D>, team: Perspective) -> Option<Self> {
        let mut result: Option<Path<D>> = None;
        let mut current = self.start;
        let mut previous_visible = false;
        let mut last_visible = None;
        if game.has_vision_at(team, current) {
            result = Some(Path::new(self.start));
            previous_visible = true;
            last_visible = Some(self.start);
        }
        for step in self.steps.iter() {
            let previous = current;
            current = step.progress(game.get_map(), current).expect(&format!("unable to find next point after {:?}", current));
            let visible = game.has_vision_at(team, current);
            if visible && !previous_visible {
                // either the unit appears out of fog or this is the first step
                if let Some(result) = &mut result {
                    // TODO: this step isn't necessary if the unit reappears in the same field where it last vanished
                    if last_visible != Some(previous) {
                        result.steps.push(PathStep::Point(previous)).unwrap();
                    }
                } else {
                    result = Some(Path::new(previous));
                }
            }
            if visible || previous_visible {
                // if the previous step was visible, this one should be too
                // CAUTION: should not be visible if teleporting into fog
                last_visible = Some(current);
                result.as_mut().unwrap().steps.push(step.clone()).unwrap();
            }
            previous_visible = visible;
        }
        result
    }
}
