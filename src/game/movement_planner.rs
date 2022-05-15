use super::game::Game;
use crate::map::direction::*;
use crate::map::point::*;
use crate::units::*;

pub struct MovementPlanner<D: Direction> {
    game: Game<D>,
    origin: Point,
    unit: UnitType,
}
impl<D: Direction> MovementPlanner<D> {
    pub fn new(game: Game<D>, origin: Point, unit: &UnitType) -> Self {
        Self {
            game,
            origin,
            unit: unit.clone(),
        }
    }
    pub fn get_game(&self) -> &Game<D> {
        &self.game
    }
    pub fn reachable_points(&self) -> Vec<Point> {
        let mut result = vec![self.origin];
        for d in D::list() {
            if let Some(point) = self.game.get_map().get_neighbor(&self.origin, &d) {
                result.push(point.point().clone());
            }
        }
        result
    }
    pub fn cancel(self) -> Game<D> {
        self.game
    }
}
