use std::collections::HashSet;

use crate::game::game::Game;
use crate::map::direction::Direction;
use crate::map::point::Point;
use crate::map::point_map;
use crate::map::wrapping_map::OrientedPoint;
use crate::player::*;

use super::*;

use zipper::*;
use zipper::zipper_derive::*;


#[derive(Debug, Zippable)]
#[zippable(bits = 6)]
pub enum ChessCommand<D: Direction> {
    Rook(D, U16::<{point_map::MAX_AREA as u16}>),
}
impl<D: Direction> ChessCommand<D> {
    pub fn convert(self, start: Point, unit: &ChessUnit, handler: &mut EventHandler<D>) -> Result<(), CommandError> {
        let max_cost = unit.typ.get_movement();
        let team = handler.get_game().get_team(Some(&unit.owner)).unwrap();
        match (self, &unit.typ) {
            (Self::Rook(dir, distance), ChessUnits::Rook(_)) => {
                let mut path = None;
                straight_search(handler.get_game(), &Path::new(start), &dir, max_cost, Some(team), |p, path_so_far| {
                    if let Some(other) = handler.get_map().get_unit(p) {
                        if other.killable_by_chess(team, handler.get_game()) {
                            path = Some(path_so_far.clone());
                            true
                        } else if !handler.get_game().has_vision_at(Some(team), p) {
                            path = Some(path_so_far.clone());
                            path.as_mut().unwrap().steps.pop();
                            true
                        } else {
                            true
                        }
                    } else if path_so_far.steps.len() == *distance as usize {
                        path = Some(path_so_far.clone());
                        true
                    } else {
                        false
                    }
                });
                if let Some(path) = path {
                    let end = path.end(handler.get_map())?;
                    let mut recalculate_fog = false;
                    if let Some(other) = handler.get_map().get_unit(&end) {
                        recalculate_fog = true;
                        handler.add_event(Event::UnitDeath(end, other.clone()));
                    }
                    handler.add_event(Event::UnitPath(Some(None), path.clone(), true, UnitType::Chess::<D>(unit.clone())));
                    let vision_changes: HashSet<Point> = unit.get_vision(handler.get_game(), &end).into_iter().filter(|p| {
                        !handler.get_game().has_vision_at(Some(team), &p)
                    }).collect();
                    if vision_changes.len() > 0 {
                        let vision_changes: Vec<Point> = vision_changes.into_iter().collect();
                        handler.add_event(Event::PureFogChange(Some(team), vision_changes.try_into().unwrap()));
                    }
                    super::on_path_details(handler, &path, &UnitType::Chess::<D>(unit.clone()));
                    handler.add_event(Event::UnitExhaust(end));
                    if recalculate_fog {
                        handler.recalculate_fog(true);
                    }
                    Ok(())
                } else {
                    Err(CommandError::InvalidPath)
                }
            }
        }
    }
}

#[derive(Debug, PartialEq, Clone, Zippable)]
pub struct ChessUnit {
    pub typ: ChessUnits,
    pub owner: Owner,
    pub hp: Hp,
    pub exhausted: bool,
}
impl ChessUnit {
    /*fn consider_path_so_far<D: Direction>(&self, game: &Game<D>, path_so_far: &Path<D>) -> u8 {
        let mut max_cost = self.typ.get_movement();
        for p in path_so_far.points(game.get_map()).unwrap().into_iter().skip(1) {
            max_cost -= game.get_map().get_terrain(&p).unwrap().movement_cost(&MovementType::Chess).unwrap();
        }
        max_cost
    }*/
    pub fn rook_directions<D: Direction>(game: &Game<D>, path_so_far: &Path<D>) -> Vec<Box<D>> {
        let mut directions = D::list();
        if path_so_far.steps.len() > 0 {
            directions = directions.into_iter().filter(|d| {
                let mut accept = false;
                straight_search(game, path_so_far, &d, 255, None, |_, path| {
                    if path.steps.len() == path_so_far.steps.len() {
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
    pub fn movable_positions<D: Direction>(&self, game: &Game<D>, path_so_far: &Path<D>) -> HashSet<Point> {
        let mut result = HashSet::new();
        match self.typ {
            ChessUnits::Rook(_) => {
                let directions = Self::rook_directions(game, path_so_far);
                for d in directions {
                    straight_search(game, path_so_far, &d, self.typ.get_movement(), game.get_team(Some(&self.owner)), |p, _| {
                        result.insert(p.clone());
                        false
                    });
                }
            }
        }
        result
    }
    pub fn shortest_path_to<D: Direction>(&self, game: &Game<D>, path_so_far: &Path<D>, goal: &Point) -> Option<Path<D>> {
        match self.typ {
            ChessUnits::Rook(_) => {
                let directions = Self::rook_directions(game, path_so_far);
                let mut result: Option<Path<D>> = None;
                for d in directions {
                    straight_search(game, path_so_far, &d, self.typ.get_movement(), game.get_team(Some(&self.owner)), |_, path| {
                        if path.end(game.get_map()).ok() == Some(*goal) {
                            // TODO: should actually compare cost instead of length
                            if result.is_none() || result.as_ref().unwrap().steps.len() > path.steps.len() {
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

#[derive(Debug, PartialEq, Clone, Zippable)]
#[zippable(bits = 4)]
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
    pub fn value(&self) -> u16 {
        match self {
            Self::Rook(_) => 500,
        }
    }
}


// callback returns true if the search can be aborted
// if team is None, units will be ignored
fn straight_search<D: Direction, F: FnMut(&Point, &Path<D>) -> bool>(game: &Game<D>, path_so_far: &Path<D>, direction: &D, max_cost: u8, team: Option<Team>, mut callback: F) {
    let mut cost = 0;
    let mut blocked_positions = HashMap::new();
    blocked_positions.insert(path_so_far.start, *direction);
    let mut path = Path {start: path_so_far.start, steps: LVec::new()};
    let mut dp = OrientedPoint::new(path_so_far.start, false, *direction);
    loop {
        if path.steps.push(PathStep::Dir(*dp.direction())).is_err() {
            break;
        }
        if path.steps.len() <= path_so_far.steps.len() && path.steps[path.steps.len() - 1] != path_so_far.steps[path.steps.len() - 1] {
            break;
        }
        if let Some(next_dp) = game.get_map().get_neighbor(dp.point(), dp.direction()) {
            if blocked_positions.get(next_dp.point()).and_then(|d| Some(d == next_dp.direction() || d.opposite_direction() == *next_dp.direction())).unwrap_or(false) {
                break;
            }
            if let Some(c) = game.get_map().get_terrain(next_dp.point()).unwrap().movement_cost(&MovementType::Chess) {
                if cost + c > max_cost {
                    break;
                }
                if let Some(team) = team {
                    if let Some(unit) = game.get_map().get_unit(next_dp.point()) {
                        if unit.killable_by_chess(team, game) && path.steps.len() > path_so_far.steps.len() {
                            callback(next_dp.point(), &path);
                        }
                        break;
                    }
                }
                cost += c;
                dp = next_dp;
                if path.steps.len() > path_so_far.steps.len() && callback(dp.point(), &path) {
                    break;
                }
                blocked_positions.insert(*dp.point(), *dp.direction());
            }
        } else {
            break;
        }
    }

}

