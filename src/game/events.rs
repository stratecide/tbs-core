use crate::map::point::Point;
use crate::units::*;
use crate::map::direction::Direction;
use crate::game::game::Game;

pub enum Command {
    UnitCommand(UnitCommand),
}
impl Command {
    pub fn check_validity<D: Direction>(&self, game: &Game<D>) -> Result<(), CommandError> {
        match self {
            Self::UnitCommand(command) => command.check_validity(game)
        }
    }
    pub fn apply<D: Direction>(self, game: &mut Game<D>) -> Vec<Event> {
        match self {
            Self::UnitCommand(command) => command.apply(game)
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
}

pub enum Event {
    UnitPath(Point, Vec<Point>),
    UnitExhaust(Point),
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
        }
    }
}
