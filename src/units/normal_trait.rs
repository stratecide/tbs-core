
use crate::game::events::*;
use crate::player::*;
use crate::map::direction::Direction;
use crate::map::point::Point;
use crate::game::game::Game;
use crate::map::map::{Map};
use crate::terrain::Terrain;

use super::*;

pub trait NormalUnitTrait<D: Direction> {
    fn as_trait(&self) -> &dyn NormalUnitTrait<D>;
    fn as_unit(&self) -> UnitType<D>;
    fn as_transportable(&self) -> TransportableTypes;
    fn get_hp(&self) -> u8;
    fn get_weapons(&self) -> Vec<(WeaponType, f32)>;
    fn get_owner(&self) -> &Owner;
    fn get_team(&self, game: &Game<D>) -> Option<Team>;
    fn can_act(&self, player: &Player) -> bool;
    fn get_movement(&self) -> (MovementType, u8);
    fn has_stealth(&self) -> bool;
    fn shortest_path_to(&self, game: &Game<D>, path_so_far: &Path<D>, goal: Point) -> Option<Path<D>> {
        let (blocked_positions, movement_type, max_cost) = self.consider_path_so_far(game, path_so_far);
        let start = path_so_far.end(game.get_map()).unwrap();
        let mut result = None;
        width_search(&movement_type, max_cost, game, start, blocked_positions, Some(self.as_trait()), |p, path| {
            if p == goal {
                result = Some(path.clone());
                true
            } else {
                false
            }
        });
        result
    }
    fn options_after_path(&self, game: &Game<D>, path: &Path<D>) -> Vec<UnitAction<D>>;
    fn can_stop_on(&self, p: Point, game: &Game<D>) -> bool {
        // doesn't check terrain
        if let Some(_) = game.get_map().get_unit(p) {
            false
        } else {
            true
        }
    }
    fn can_attack_after_moving(&self) -> bool;
    fn shortest_path_to_attack(&self, game: &Game<D>, path_so_far: &Path<D>, goal: Point) -> Option<Path<D>> {
        if !self.can_attack_after_moving() {
            // no need to look for paths if the unit can't attack after moving
            if path_so_far.steps.len() == 0 && self.attackable_positions(game, path_so_far.start, false).contains(&goal) {
                return Some(path_so_far.clone());
            } else {
                return None;
            }
        }
        let (blocked_positions, movement_type, max_cost) = self.consider_path_so_far(game, path_so_far);
        let current_pos = path_so_far.end(game.get_map()).unwrap();
        let mut result = None;
        width_search(&movement_type, max_cost, game, current_pos, blocked_positions, Some(self.as_trait()), |p, path| {
            if (p == path_so_far.start || self.can_stop_on(p, game)) && self.attackable_positions(game, p, path.steps.len() + path_so_far.steps.len() > 0).contains(&goal) {
                result = Some(path.clone());
                true
            } else {
                false
            }
        });
        result
    }
    fn can_move_to(&self, p: Point, game: &Game<D>) -> bool {
        // doesn't check terrain
        if let Some(unit) = game.get_map().get_unit(p) {
            if !unit.can_be_moved_through(self.as_trait(), game) {
                return false
            }
        }
        true
    }
    fn consider_path_so_far(&self, game: &Game<D>, path_so_far: &Path<D>) -> (HashSet<Point>, MovementType, u8) {
        let (movement_type, mut max_cost) = self.get_movement();
        let mut blocked_positions = HashSet::new();
        blocked_positions.insert(path_so_far.start);
        for step in path_so_far.points(game.get_map()).unwrap().into_iter().skip(1) {
            blocked_positions.insert(step);
            max_cost -= game.get_map().get_terrain(step).unwrap().movement_cost(&movement_type).unwrap();
        }
        blocked_positions.remove(&path_so_far.end(game.get_map()).unwrap());
        (blocked_positions, movement_type, max_cost)
    }
    fn movable_positions(&self, game: &Game<D>, path_so_far: &Path<D>) -> HashSet<Point> {
        let (blocked_positions, movement_type, max_cost) = self.consider_path_so_far(game, path_so_far);
        let start = path_so_far.end(game.get_map()).unwrap();
        let mut result = HashSet::new();
        width_search(&movement_type, max_cost, game, start, blocked_positions, Some(self.as_trait()), |p, _| {
            result.insert(p.clone());
            false
        });
        result
    }
    fn check_path(&self, game: &Game<D>, path: &Path<D>) -> Result<(), CommandError> {
        let (movement_type, mut remaining_movement) = self.get_movement();
        let mut current = path.start;
        for p in &path.steps {
            // the points in the path have to neighbor each other
            current = match p {
                PathStep::Dir(d) => {
                    if let Some(p) = game.get_map().get_neighbor(current, *d) {
                        p.point
                    } else {
                        // should not happen because it's already checked before calling this method, but better to be safe
                        return Err(CommandError::InvalidPath);
                    }
                }
                PathStep::Jump(d) => {
                    if game.get_map().get_terrain(current) != Some(&Terrain::Fountain) {
                        return Err(CommandError::InvalidPath);
                    }
                    if let Some(p) = game.get_map().get_neighbor(current, *d).and_then(|o| game.get_map().get_neighbor(o.point, o.direction)) {
                        p.point
                    } else {
                        // should not happen because it's already checked before calling this method, but better to be safe
                        return Err(CommandError::InvalidPath);
                    }
                }
                PathStep::Diagonal(_) | PathStep::Knight(_, _) => {
                    return Err(CommandError::InvalidPath);
                }
                PathStep::Point(_p) => {
                    // currently no use case
                    return Err(CommandError::InvalidPath);
                }
            };
            // check if that unit can move far enough
            if let Some(terrain) = game.get_map().get_terrain(current) {
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
            // no visible unit should block movement
            if let Some(unit) = game.get_map().get_unit(current) {
                if game.has_vision_at(Some(game.current_player().team), current) && !unit.can_be_moved_through(self.as_trait(), game) {
                    return Err(CommandError::InvalidPath);
                }
            }
        }
        Ok(())
    }
    fn get_attack_type(&self) -> AttackType;
    fn can_attack_unit(&self, game: &Game<D>, unit: &UnitType<D>) -> bool;
    fn threatens(&self, game: &Game<D>, unit: &UnitType<D>) -> bool;
    fn attackable_positions(&self, game: &Game<D>, position: Point, moved: bool) -> HashSet<Point>;
    // the result-vector should never contain the same point multiple times
    fn attack_splash(&self, map: &Map<D>, from: Point, to: &AttackInfo<D>) -> Result<Vec<Point>, CommandError>;
    fn make_attack_info(&self, game: &Game<D>, from: Point, to: Point) -> Option<AttackInfo<D>>;
    fn can_capture(&self) -> bool;
    fn can_pull(&self) -> bool;
}

pub fn check_normal_unit_can_act<D: Direction>(game: &Game<D>, at: Point, unload_index: Option<UnloadIndex>) -> Result<(), CommandError> {
    if !game.has_vision_at(Some(game.current_player().team), at) {
        return Err(CommandError::NoVision);
    }
    let unit = game.get_map().get_unit(at).ok_or(CommandError::MissingUnit)?;
    let unit: &dyn NormalUnitTrait<D> = if let Some(index) = unload_index {
        unit.get_boarded().get(*index as usize).ok_or(CommandError::MissingBoardedUnit)?.as_trait()
    } else {
        unit.as_normal_trait().ok_or(CommandError::UnitTypeWrong)?
    };
    if &game.current_player().owner_id != unit.get_owner() {
        return Err(CommandError::NotYourUnit);
    }
    if !unit.can_act(game.current_player()) {
        return Err(CommandError::UnitCannotMove);
    }
    Ok(())
}
