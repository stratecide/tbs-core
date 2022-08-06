use std::collections::{BinaryHeap, HashSet};
use std::cmp::{Ordering, Reverse};

use crate::map::direction::Direction;
use crate::map::point::Point;
use crate::game::game::Game;
use crate::map::map::{NeighborMode};

use super::normal_trait::*;

pub enum MovementType {
    Hover,
    Foot,
    Wheel,
    Treads,
    Heli,
    Chess,
}

#[derive(PartialEq, Eq)]
struct WidthSearch {
    path: Vec<Point>,
    path_cost: u8,
}
impl PartialOrd for WidthSearch {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.path_cost.cmp(&other.path_cost))
    }
}
impl Ord for WidthSearch {
    fn cmp(&self, other: &Self) -> Ordering {
        self.path_cost.cmp(&other.path_cost)
    }
}

// callback returns true if the search can be aborted
// if team is None, units will be ignored
pub fn width_search<D: Direction, F: FnMut(&Point, &Vec<Point>) -> bool>(movement_type: &MovementType, max_cost: u8, game: &Game<D>, start: &Point, mut blocked_positions: HashSet<Point>, unit: Option<&dyn NormalUnitTrait<D>>, mut callback: F) {
    let mut next_checks = BinaryHeap::new();
    let mut add_point = |p: &Point, path_so_far: &Vec<Point>, cost_so_far: u8, next_checks: &mut BinaryHeap<Reverse<WidthSearch>>| {
        if blocked_positions.contains(p) {
            return false;
        }
        if callback(p, path_so_far) {
            return true;
        }
        blocked_positions.insert(p.clone());
        for neighbor in game.get_map().get_neighbors(p, NeighborMode::UnitMovement) {
            if !blocked_positions.contains(neighbor.point()) {
                match (unit, game.get_map().get_unit(neighbor.point())) {
                    (Some(mover), Some(other)) => {
                        if !other.can_be_moved_through(mover, game) {
                            continue;
                        }
                    }
                    (_, _) => {}
                }
                if let Some(cost) = game.get_map().get_terrain(neighbor.point()).unwrap().movement_cost(movement_type) {
                    if cost_so_far + cost <= max_cost {
                        let mut path = path_so_far.clone();
                        path.push(neighbor.point().clone());
                        next_checks.push(Reverse(WidthSearch{path, path_cost: cost_so_far + cost}));
                    }
                }
            }
        }
        false
    };
    add_point(start, &vec![], 0, &mut next_checks);
    while let Some(Reverse(check)) = next_checks.pop() {
        let finished = add_point(check.path.last().unwrap(), &check.path, check.path_cost, &mut next_checks);
        if finished {
            break;
        }
    }
}