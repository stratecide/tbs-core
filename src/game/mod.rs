pub mod game;
pub mod settings;
pub mod events;
pub mod event_handler;
pub mod rhai_event_handler;
pub mod commands;
pub mod fog;
pub mod event_fx;
#[cfg(test)]
mod test;

use interfaces::ExportedGame;
use semver::Version;
use uniform_smart_pointer::Urc;
use zipper::{Unzipper, ZipperError};

use crate::{config::config::Config, map::direction::*};

use self::game::Game;

pub enum GameType {
    Square(Game<Direction4>),
    Hex(Game<Direction6>),
}

pub fn import_server(config: &Urc<Config>, data: ExportedGame, version: Version) -> Result<GameType, ZipperError> {
    let mut unzipper = Unzipper::new(vec![data.public[0]], version.clone());
    if unzipper.read_bool()? {
        Ok(GameType::Hex(Game::import_server(data, config, version)?))
    } else {
        Ok(GameType::Square(Game::import_server(data, config, version)?))
    }
}

pub fn import_client(config: &Urc<Config>, public: Vec<u8>, team_view: Option<(u8, Vec<u8>)>, version: Version) -> Result<GameType, ZipperError> {
    let mut unzipper = Unzipper::new(vec![public[0]], version.clone());
    if unzipper.read_bool()? {
        Ok(GameType::Hex(Game::import_client(public, team_view, config, version)?))
    } else {
        Ok(GameType::Square(Game::import_client(public, team_view, config, version)?))
    }
}
