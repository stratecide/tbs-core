pub mod game;
pub mod settings;
pub mod events;
pub mod event_handler;
pub mod rhai_event_handler;
pub mod commands;
pub mod fog;
pub mod game_view;
pub mod rhai_board;
pub mod modified_view;
pub mod event_fx;

use std::sync::Arc;

use interfaces::ExportedGame;
use semver::Version;
use zipper::{Unzipper, ZipperError};

use crate::{config::config::Config, map::direction::*};

use self::game::Game;

pub enum GameType {
    Square(Game<Direction4>),
    Hex(Game<Direction6>),
}

pub fn import_server(config: &Arc<Config>, data: ExportedGame, version: Version) -> Result<GameType, ZipperError> {
    let mut unzipper = Unzipper::new(vec![data.public[0]], version.clone());
    if unzipper.read_bool()? {
        Ok(GameType::Hex(*Game::import_server(data, config, version)?))
    } else {
        Ok(GameType::Square(*Game::import_server(data, config, version)?))
    }
}

pub fn import_client(config: &Arc<Config>, public: Vec<u8>, team_view: Option<(u8, Vec<u8>)>, version: Version) -> Result<GameType, ZipperError> {
    let mut unzipper = Unzipper::new(vec![public[0]], version.clone());
    if unzipper.read_bool()? {
        Ok(GameType::Hex(*Game::import_client(public, team_view, config, version)?))
    } else {
        Ok(GameType::Square(*Game::import_client(public, team_view, config, version)?))
    }
}

#[cfg(test)]
mod tests {

    use std::sync::Arc;

    use interfaces::game_interface::*;
    use interfaces::Perspective;
    use semver::Version;
    use crate::game::game::*;
    use crate::game::fog::*;
    use crate::VERSION;

    #[test]
    fn export_import_chess() {
        let version = Version::parse(VERSION).unwrap();
        let map = crate::map::test::chess_board();
        let config = map.environment().config.clone();
        let settings = map.settings().unwrap();

        for fog_setting in [FogSetting::None, FogSetting::Sharp(0)] {
            println!("fog setting: {fog_setting}");
            let mut settings = settings.clone();
            settings.fog_mode = FogMode::Constant(fog_setting);
            let perspective = Perspective::Team(0);
            let (server, events) = Game::new_server(map.clone(), settings.build_default(), Arc::new(|| 0.));
            let client = Game::new_client(map.clone(), settings.build_default(), events.get(&perspective).unwrap());
            let data = server.export();
            println!("data: {data:?}");
            let imported_server = Game::import_server(data.clone(), &config, version.clone()).unwrap();
            server.with(|server| {
                assert_eq!(server.get_fog(), imported_server.get_fog());
                assert_eq!(server.environment(), imported_server.environment());
                assert_eq!(*server, *imported_server);
            });
            client.with(|client| {
                assert_eq!(*client, *Game::import_client(data.public.clone(), data.get_team(0), &config, version.clone()).unwrap());
            });
        }
    }
}
