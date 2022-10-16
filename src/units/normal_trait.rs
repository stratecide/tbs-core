
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
    fn as_trait_mut(&mut self) -> &mut dyn NormalUnitTrait<D>;
    fn as_unit(&self) -> UnitType<D>;
    fn as_transportable(&self) -> TransportableTypes;
    fn get_type(&self) -> &NormalUnits;
    fn get_type_mut(&mut self) -> &mut NormalUnits;
    fn get_hp(&self) -> u8;
    fn get_weapons(&self) -> Vec<(WeaponType, f32)>;
    fn get_owner(&self) -> &Owner;
    fn get_team(&self, game: &Game<D>) -> Option<Team>;
    fn can_act(&self, player: &Player) -> bool;
    fn get_movement(&self, terrain: &Terrain<D>) -> (MovementType, u8);
    fn has_stealth(&self) -> bool;
    fn shortest_path_to(&self, game: &Game<D>, path_so_far: &Path<D>, goal: Point) -> Option<Path<D>> {
        let mut result = None;
        movement_search(game, self.as_trait(), path_so_far, None, |path, p, _can_stop_here| {
            if goal == p {
                result = Some(path.clone());
                PathSearchFeedback::Found
            } else {
                PathSearchFeedback::Continue
            }
        });
        result
    }
    fn options_after_path(&self, game: &Game<D>, path: &Path<D>) -> Vec<UnitAction<D>>;
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
        let mut result = None;
        movement_search(game, self.as_trait(), path_so_far, None, |path, p, can_stop_here| {
            if can_stop_here && self.attackable_positions(game, p, path.steps.len() > 0).contains(&goal) {
                result = Some(path.clone());
                PathSearchFeedback::Found
            } else {
                PathSearchFeedback::Continue
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
    fn movable_positions(&self, game: &Game<D>, path_so_far: &Path<D>) -> HashSet<Point> {
        let mut result = HashSet::new();
        movement_search(game, self.as_trait(), path_so_far, None, |_path, p, _can_stop_here| {
            result.insert(p);
            PathSearchFeedback::Continue
        });
        result
    }
    fn check_path(&self, game: &Game<D>, path_to_check: &Path<D>) -> Result<(), CommandError> {
        let team = self.get_team(game);
        let fog = game.get_fog().get(&team);
        let mut path_is_valid = false;
        movement_search(game, self.as_trait(), path_to_check, fog, |path, _p, can_stop_here| {
            if path == path_to_check {
                path_is_valid = can_stop_here;
            }
            // if path_to_check will be found at all, it would be the first one this callback gets called with
            PathSearchFeedback::Found
        });
        // TODO: make this method's return value a bool
        if path_is_valid {
            Ok(())
        } else {
            Err(CommandError::InvalidPath)
        }
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
