use std::collections::{BinaryHeap, HashSet};
use std::cmp::{Ordering, Reverse};
use std::fmt;

use crate::game::events::*;
use crate::player::{Owner, Player};
use crate::map::direction::Direction;
use crate::map::point::Point;
use crate::game::game::Game;

#[derive(Debug, PartialEq, Clone)]
pub enum UnitType {
    Normal(NormalUnit),
}
impl UnitType {
    pub fn get_owner(&self) -> Option<&Owner> {
        match self {
            Self::Normal(unit) => Some(&unit.owner),
        }
    }
    pub fn can_act(&self, player: &Player) -> bool {
        match self {
            Self::Normal(unit) => !unit.exhausted && unit.owner == player.owner_id,
        }
    }
    pub fn movable_positions<D: Direction>(&self, game: &Game<D>, start: &Point, path_so_far: &Vec<Point>) -> HashSet<Point> {
        match self {
            Self::Normal(unit) => unit.movable_positions(game, start, path_so_far)
        }
    }
    pub fn shortest_path_to<D: Direction>(&self, game: &Game<D>, start: &Point, path_so_far: &Vec<Point>, goal: &Point) -> Option<Vec<Point>> {
        match self {
            Self::Normal(unit) => unit.shortest_path_to(game, start, path_so_far, goal)
        }
    }
    pub fn options_after_path<D: Direction>(&self, game: &Game<D>, start: &Point, path: &Vec<Point>) -> Vec<UnitAction> {
        match self {
            Self::Normal(unit) => unit.options_after_path(game, start, path)
        }
    }
    pub fn can_move_to<D: Direction>(&self, p: &Point, game: &Game<D>) -> bool {
        match self {
            Self::Normal(unit) => unit.can_move_to(p, game)
        }
    }
    pub fn check_path<D: Direction>(&self, game: &Game<D>, start: &Point, path: &Vec<Point>) -> Result<(), CommandError> {
        match self {
            Self::Normal(unit) => unit.check_path(game, start, path)
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum NormalUnits {
    Hovercraft,
    TransportHeli(Vec<NormalUnit>),
}

#[derive(Debug, PartialEq, Clone)]
pub struct NormalUnit {
    pub typ: NormalUnits,
    pub owner: Owner,
    pub hp: u8,
    pub exhausted: bool,
}
impl NormalUnit {
    pub fn new_instance(from: NormalUnits, color_id: u8) -> NormalUnit {
        NormalUnit {
            typ: from,
            owner: color_id,
            hp: 100,
            exhausted: false,
        }
    }
    pub fn get_movement(&self) -> (MovementType, u8) {
        let factor = 6;
        match self.typ {
            NormalUnits::Hovercraft => (MovementType::Hover, 3 * factor),
            NormalUnits::TransportHeli(_) => (MovementType::Heli, 6 * factor),
        }
    }
    fn consider_path_so_far<D: Direction>(&self, game: &Game<D>, start: &Point, path_so_far: &Vec<Point>) -> (HashSet<Point>, MovementType, u8) {
        let (movement_type, mut max_cost) = self.get_movement();
        let mut blocked_positions = HashSet::new();
        if path_so_far.len() > 0 {
            blocked_positions.insert(start.clone());
            for step in path_so_far {
                blocked_positions.insert(step.clone());
                max_cost -= game.get_map().get_terrain(step).unwrap().movement_cost(&movement_type).unwrap();
            }
            blocked_positions.remove(path_so_far.last().unwrap());
        };
        (blocked_positions, movement_type, max_cost)
    }
    pub fn movable_positions<D: Direction>(&self, game: &Game<D>, start: &Point, path_so_far: &Vec<Point>) -> HashSet<Point> {
        let (blocked_positions, movement_type, max_cost) = self.consider_path_so_far(game, start, path_so_far);
        let start = path_so_far.last().unwrap_or(start);
        let mut result = HashSet::new();
        width_search(&movement_type, max_cost, game, start, blocked_positions, |p, _| {
            result.insert(p.clone());
            false
        });
        result
    }
    pub fn shortest_path_to<D: Direction>(&self, game: &Game<D>, start: &Point, path_so_far: &Vec<Point>, goal: &Point) -> Option<Vec<Point>> {
        let (blocked_positions, movement_type, max_cost) = self.consider_path_so_far(game, start, path_so_far);
        let start = path_so_far.last().unwrap_or(start);
        let mut result = None;
        width_search(&movement_type, max_cost, game, start, blocked_positions, |p, path| {
            if p == goal {
                result = Some(path.clone());
                true
            } else {
                false
            }
        });
        result
    }
    pub fn options_after_path<D: Direction>(&self, game: &Game<D>, _start: &Point, path: &Vec<Point>) -> Vec<UnitAction> {
        let mut result = vec![];
        if path.len() == 0 || game.get_map().get_unit(path.last().unwrap()).is_none() {
            result.push(UnitAction::Wait);
        }
        result
    }
    pub fn can_move_to<D: Direction>(&self, p: &Point, game: &Game<D>) -> bool {
        // doesn't check terrain
        if let Some(unit) = game.get_map().get_unit(p) {
            if !self.can_move_past(unit) {
                return false
            }
        }
        true
    }
    fn can_move_past(&self, other: &UnitType) -> bool {
        // TODO: should be false for enemy units unless self has stealth
        true
    }
    pub fn check_path<D: Direction>(&self, game: &Game<D>, start: &Point, path: &Vec<Point>) -> Result<(), CommandError> {
        let mut blocked = HashSet::new();
        blocked.insert(start.clone());
        let (movement_type, mut remaining_movement) = self.get_movement();
        let mut current = start;
        for p in path {
            // no point can be travelled to twice
            if blocked.contains(p) {
                return Err(CommandError::InvalidPath);
            }
            // check if that unit can move far enough
            if let Some(terrain) = game.get_map().get_terrain(p) {
                if let Some(cost) = terrain.movement_cost(&movement_type) {
                    if cost > remaining_movement {
                        return Err(CommandError::InvalidPath);
                    }
                    remaining_movement -= cost;
                } else {
                    return Err(CommandError::InvalidPath);
                }
            } else {
                // no terrain means the point is invalid
                return Err(CommandError::InvalidPath);
            }
            // the points in the path have to neighbor each other
            if game.get_map().get_direction(current, p).is_none() {
                return Err(CommandError::InvalidPath);
            }
            // no visible unit should block movement
            if let Some(unit) = game.get_map().get_unit(p) {
                if game.has_vision_at(game.current_player(), p) && !self.can_move_past(unit) {
                    return Err(CommandError::InvalidPath);
                }
            }
            current = p;
            blocked.insert(p.clone());
        }
        Ok(())
    }
}

pub enum MovementType {
    Hover,
    Foot,
    Wheel,
    Treads,
    Heli,
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

fn width_search<D: Direction, F: FnMut(&Point, &Vec<Point>) -> bool>(movement_type: &MovementType, max_cost: u8, game: &Game<D>, start: &Point, mut blocked_positions: HashSet<Point>, mut callback: F) {
    let mut next_checks = BinaryHeap::new();
    let mut add_point = |p: &Point, path_so_far: &Vec<Point>, cost_so_far: u8, next_checks: &mut BinaryHeap<Reverse<WidthSearch>>| {
        if !blocked_positions.contains(p) && callback(p, path_so_far) {
            return true;
        }
        blocked_positions.insert(p.clone());
        for neighbor in game.get_map().get_neighbors(p) {
            if !blocked_positions.contains(neighbor.point()) {
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

#[derive(Debug, Clone)]
pub enum UnitAction {
    Wait,
}
impl fmt::Display for UnitAction {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Wait => write!(f, "Wait"),
        }
    }
}

pub enum UnitCommand {
    MoveWait(Point, Vec<Point>),
}
impl UnitCommand {
    /**
     * only checks whether the command appears valid to the player that sent it
     */
    pub fn check_validity<D: Direction>(&self, game: &Game<D>) -> Result<(), CommandError> {
        match self {
            Self::MoveWait(start, path) => {
                if !game.has_vision_at(game.current_player(), start) {
                    return Err(CommandError::NoVision);
                }
                if let Some(unit) = game.get_map().get_unit(start) {
                    if Some(&game.current_player().owner_id) != unit.get_owner() {
                        return Err(CommandError::NotYourUnit);
                    }
                    if !unit.can_act(game.current_player()) {
                        return Err(CommandError::UnitCannotMove);
                    }
                    unit.check_path(game, start, path)?;
                    if let Some(p) = path.last() {
                        if let Some(_) = game.get_map().get_unit(p) {
                            if game.has_vision_at(game.current_player(), p) {
                                return Err(CommandError::InvalidPath);
                            }
                        }
                    }
                    Ok(())
                } else {
                    Err(CommandError::MissingUnit)
                }
            }
        }
    }
    pub fn apply<D: Direction>(self, game: &Game<D>) -> Vec<Event> {
        let mut result = vec![];
        match self {
            Self::MoveWait(start, path) => {
                let unit = game.get_map().get_unit(&start).unwrap();
                let mut path_taken = vec![];
                for p in path {
                    if !unit.can_move_to(&p, game) {
                        break;
                    }
                    path_taken.push(p);
                }
                let end = path_taken.last().unwrap_or(&start).clone();
                if path_taken.len() > 0 {
                    result.push(Event::UnitPath(start, path_taken));
                }
                result.push(Event::UnitExhaust(end));
            }
        }
        result
    }
}
