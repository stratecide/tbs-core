use std::collections::HashSet;

use crate::game::game::Game;
use crate::map::direction::Direction;
use crate::map::point::Point;
use crate::map::wrapping_map::OrientedPoint;
use crate::player::*;

use super::*;


pub enum ChessCommand<D: Direction> {
    Rook(D, u16),
}
impl<D: Direction> ChessCommand<D> {
    pub fn convert(self, start: Point, unit: &ChessUnit, handler: &mut EventHandler<D>) -> Result<(), CommandError> {
        let max_cost = unit.typ.get_movement();
        let team = handler.get_game().get_team(Some(&unit.owner)).unwrap();
        match (self, &unit.typ) {
            (Self::Rook(dir, distance), ChessUnits::Rook(_)) => {
                let mut path = None;
                straight_search(handler.get_game(), &start, &dir, max_cost, HashSet::new(), Some(team), |p, path_so_far| {
                    if let Some(other) = handler.get_map().get_unit(p) {
                        if other.killable_by_chess(team, handler.get_game()) {
                            path = Some(path_so_far.clone());
                            true
                        } else if !handler.get_game().has_vision_at(Some(team), p) {
                            path = Some(path_so_far.clone());
                            path.as_mut().unwrap().pop();
                            true
                        } else {
                            true
                        }
                    } else if path_so_far.len() == distance as usize {
                        path = Some(path_so_far.clone());
                        true
                    } else {
                        false
                    }
                });
                if let Some(mut path) = path {
                    path.insert(0, start.clone());
                    let p = path.last().unwrap().clone();
                    if let Some(other) = handler.get_map().get_unit(&p) {
                        handler.add_event(Event::UnitDeath(p.clone(), other.clone()));
                    }
                    handler.add_event(Event::UnitPath(None, path.iter().map(|p| Some(p.clone())).collect(), UnitType::Chess::<D>(unit.clone())));
                    let vision_changes: HashSet<Point> = unit.get_vision(handler.get_game(), &p).into_iter().filter(|p| {
                        !handler.get_game().has_vision_at(Some(team), &p)
                    }).collect();
                    if vision_changes.len() > 0 {
                        handler.add_event(Event::PureFogChange(Some(team), vision_changes));
                    }
                    super::on_path_details(handler, &path, &UnitType::Chess::<D>(unit.clone()));
                    handler.add_event(Event::UnitExhaust(p));
                    Ok(())
                } else {
                    Err(CommandError::InvalidPath)
                }
            }
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct ChessUnit {
    pub typ: ChessUnits,
    pub owner: Owner,
    pub hp: u8,
    pub exhausted: bool,
}
impl ChessUnit {
    fn consider_path_so_far<D: Direction>(&self, game: &Game<D>, _start: &Point, path_so_far: &Vec<Point>) -> u8 {
        let mut max_cost = self.typ.get_movement();
        for step in path_so_far {
            max_cost -= game.get_map().get_terrain(step).unwrap().movement_cost(&MovementType::Chess).unwrap();
        }
        max_cost
    }
    pub fn rook_directions<D: Direction>(game: &Game<D>, start: &Point, path_so_far: &Vec<Point>) -> Vec<Box<D>> {
        let mut directions = D::list();
        if path_so_far.len() > 0 {
            directions = directions.into_iter().filter(|d| {
                let mut accept = false;
                straight_search(game, start, &d, 255, HashSet::new(), None, |_, path| {
                    if path.len() == path_so_far.len() {
                        accept = path == path_so_far;
                        true
                    } else {
                        false
                    }
                });
                accept
            }).collect();
        }
        directions
    }
    pub fn movable_positions<D: Direction>(&self, game: &Game<D>, start: &Point, path_so_far: &Vec<Point>) -> HashSet<Point> {
        let max_cost = self.consider_path_so_far(game, start, path_so_far);
        let mut result = HashSet::new();
        match self.typ {
            ChessUnits::Rook(_) => {
                let mut blocked_positions:HashSet<Point> = path_so_far.iter().map(|p| p.clone()).collect();
                blocked_positions.insert(start.clone());
                let directions = Self::rook_directions(game, start, path_so_far);
                for d in directions {
                    straight_search(game, start, &d, max_cost, blocked_positions.clone(), game.get_team(Some(&self.owner)), |p, _| {
                        result.insert(p.clone());
                        false
                    });
                }
            }
        }
        result
    }
    pub fn shortest_path_to<D: Direction>(&self, game: &Game<D>, start: &Point, path_so_far: &Vec<Point>, goal: &Point) -> Option<Vec<Point>> {
        let max_cost = self.consider_path_so_far(game, start, path_so_far);
        let mut blocked_positions:HashSet<Point> = path_so_far.iter().map(|p| p.clone()).collect();
        blocked_positions.insert(start.clone());
        match self.typ {
            ChessUnits::Rook(_) => {
                let directions = Self::rook_directions(game, start, path_so_far);
                let mut result: Option<Vec<Point>> = None;
                for d in directions {
                    straight_search(game, start, &d, max_cost, blocked_positions.clone(), game.get_team(Some(&self.owner)), |_, path| {
                        if path.last() == Some(goal) {
                            // TODO: should actually compare cost instead of length
                            if result.is_none() || result.as_ref().unwrap().len() > path.len() {
                                result = Some(path.clone());
                            }
                        }
                        false
                    });
                }
                result
            }
        }
    }
    fn true_vision_range<D: Direction>(&self, _game: &Game<D>, _pos: &Point) -> usize {
        1
    }
    fn vision_range<D: Direction>(&self, _game: &Game<D>, _pos: &Point) -> usize {
        match self.typ {
            ChessUnits::Rook(_) => 8,
        }
    }
    pub fn get_vision<D: Direction>(&self, game: &Game<D>, pos: &Point) -> HashSet<Point> {
        let mut result = HashSet::new();
        match self.typ {
            ChessUnits::Rook(_) => {
                for d in D::list() {
                    let mut current = OrientedPoint::new(pos.clone(), false, *d);
                    for i in 0..self.vision_range(game, pos) {
                        if let Some(dp) = game.get_map().get_neighbor(current.point(), current.direction()) {
                            let terrain = game.get_map().get_terrain(current.point()).unwrap();
                            if i >= self.true_vision_range(game, pos) && terrain.requires_true_sight() {
                                break;
                            }
                            current = dp;
                            result.insert(current.point().clone());
                            if terrain.movement_cost(&MovementType::Chess).is_none() {
                                break;
                            }
                        } else {
                            break;
                        }
                    }
                }
            }
        }
        result
    }
}

pub fn check_chess_unit_can_act<D: Direction>(game: &Game<D>, at: &Point) -> Result<(), CommandError> {
    if !game.has_vision_at(Some(game.current_player().team), at) {
        return Err(CommandError::NoVision);
    }
    let unit = match game.get_map().get_unit(&at).ok_or(CommandError::MissingUnit)? {
        UnitType::Chess(unit) => unit,
        _ => return Err(CommandError::UnitTypeWrong),
    };
    if game.current_player().owner_id != unit.owner {
        return Err(CommandError::NotYourUnit);
    }
    if unit.exhausted {
        return Err(CommandError::UnitCannotMove);
    }
    Ok(())
}

#[derive(Debug, PartialEq, Clone)]
pub enum ChessUnits {
    Rook(bool),
}
impl ChessUnits {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Rook(_) => "Rook",
        }
    }
    pub fn get_movement(&self) -> u8 {
        match self {
            Self::Rook(_) => 8 * 6,
        }
    }
    pub fn get_armor(&self) -> (ArmorType, f32) {
        match self {
            Self::Rook(_) => (ArmorType::Light, 1.5),
        }
    }
}


// callback returns true if the search can be aborted
// if team is None, units will be ignored
fn straight_search<D: Direction, F: FnMut(&Point, &Vec<Point>) -> bool>(game: &Game<D>, start: &Point, direction: &D, max_cost: u8, mut blocked_positions: HashSet<Point>, team: Option<Team>, mut callback: F) {
    let mut dp = OrientedPoint::new(start.clone(), false, *direction);
    let mut ray = vec![];
    let mut cost = 0;
    blocked_positions.insert(start.clone());
    loop {
        if let Some(next_dp) = game.get_map().get_neighbor(dp.point(), dp.direction()) {
            if blocked_positions.contains(next_dp.point()) {
                break;
            }
            if let Some(c) = game.get_map().get_terrain(next_dp.point()).unwrap().movement_cost(&MovementType::Chess) {
                if cost + c > max_cost {
                    break;
                }
                if let Some(team) = team {
                    if let Some(unit) = game.get_map().get_unit(next_dp.point()) {
                        if unit.killable_by_chess(team, game) {
                            ray.push(next_dp.point().clone());
                            callback(next_dp.point(), &ray);
                        }
                        break;
                    }
                }
                cost += c;
                dp = next_dp;
                ray.push(dp.point().clone());
                if callback(dp.point(), &ray) {
                    break;
                }
                blocked_positions.insert(dp.point().clone());
            }
        } else {
            break;
        }
    }

}

