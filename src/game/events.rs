use crate::map::map::Map;
use crate::map::point::Point;
use crate::units::*;
use crate::map::direction::Direction;
use crate::game::game::Game;
use super::events;

pub enum Command<D: Direction> {
    UnitCommand(UnitCommand<D>),
}
impl<D: Direction> Command<D> {
    pub fn convert(self, handler: &mut events::EventHandler<D>) -> Result<(), CommandError> {
        match self {
            Self::UnitCommand(command) => command.convert(handler)
        }
    }
}

#[derive(Debug, Clone)]
pub enum CommandError {
    NoVision,
    MissingUnit,
    NotYourUnit,
    UnitCannotMove,
    InvalidPath,
    InvalidPoint(Point),
    InvalidTarget
}

#[derive(Debug, Clone)]
pub enum Event {
    UnitPath(Point, Vec<Point>),
    UnitExhaust(Point),
    UnitHpChange(Point, i8, i16),
    UnitDeath(Point, UnitType),
}
impl Event {
    pub fn apply<D: Direction>(&self, game: &mut Game<D>) {
        match self {
            Self::UnitPath(start, path) => {
                let unit = game.get_map_mut().set_unit(start.clone(), None);
                game.get_map_mut().set_unit(path.last().unwrap_or(start).clone(), unit);
            }
            Self::UnitExhaust(pos) => {
                let unit = game.get_map_mut().get_unit_mut(pos).expect(&format!("expected a unit at {:?} to exhaust!", pos));
                match unit {
                    UnitType::Normal(unit) => unit.exhausted = true,
                }
            }
            Self::UnitHpChange(pos, hp_change, _) => {
                let unit = game.get_map_mut().get_unit_mut(pos).expect(&format!("expected a unit at {:?} to change hp by {}!", pos, hp_change));
                match unit {
                    UnitType::Normal(unit) => unit.hp = (unit.hp as i8 + hp_change) as u8,
                }
            }
            Self::UnitDeath(pos, _) => {
                game.get_map_mut().set_unit(pos.clone(), None).expect(&format!("expected a unit at {:?} to die!", pos));
            }
        }
    }
    pub fn undo<D: Direction>(&self, game: &mut Game<D>) {
        match self {
            Self::UnitPath(start, path) => {
                let unit = game.get_map_mut().set_unit(path.last().unwrap_or(start).clone(), None);
                game.get_map_mut().set_unit(start.clone(), unit);
            }
            Self::UnitExhaust(pos) => {
                let unit = game.get_map_mut().get_unit_mut(pos).expect(&format!("expected a unit at {:?} to exhaust!", pos));
                match unit {
                    UnitType::Normal(unit) => unit.exhausted = false,
                }
            }
            Self::UnitHpChange(pos, hp_change, _) => {
                let unit = game.get_map_mut().get_unit_mut(pos).expect(&format!("expected a unit at {:?} to change hp by {}!", pos, -hp_change));
                match unit {
                    UnitType::Normal(unit) => unit.hp = (unit.hp as i8 - hp_change) as u8,
                }
            }
            Self::UnitDeath(pos, unit) => {
                game.get_map_mut().set_unit(pos.clone(), Some(unit.clone()));
            }
        }
    }
}


pub struct EventHandler<'a, D: Direction> {
    events: Vec<Event>,
    game: &'a mut Game<D>,
}
impl<'a, D: Direction> EventHandler<'a, D> {
    pub fn new(game: &'a mut Game<D>) -> Self {
        EventHandler {
            events: vec![],
            game,
        }
    }
    pub fn get_game(&self) -> &Game<D> {
        &self.game
    }
    pub fn get_map(&self) -> &Map<D> {
        self.game.get_map()
    }
    pub fn add_event(&mut self, event: Event) {
        event.apply(&mut self.game);
        self.events.push(event);
    }
    pub fn accept(self) -> Vec<Event> {
        self.events
    }
    pub fn cancel(mut self) {
        while let Some(event) = self.events.pop() {
            event.undo(&mut self.game);
        }
    }
}
