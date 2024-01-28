pub mod game;
pub mod settings;
pub mod events;
pub mod event_handler;
pub mod commands;
pub mod fog;

use std::sync::Arc;

use interfaces::game_interface::{ExportedGame, GameInterface};
use semver::Version;
use zipper::{Unzipper, ZipperError};

use crate::{config::config::Config, map::direction::*};

use self::game::Game;

pub enum GameType {
    Square(Game<Direction4>),
    Hex(Game<Direction6>),
}

pub fn import_server(config: &Arc<Config>, data: ExportedGame, name: String, version: Version) -> Result<GameType, ZipperError> {
    let mut unzipper = Unzipper::new(vec![data.public[0]], version.clone());
    if unzipper.read_bool()? {
        Ok(GameType::Hex(*Game::import_server(data, config, name, version)?))
    } else {
        Ok(GameType::Square(*Game::import_server(data, config, name, version)?))
    }
}

pub fn import_client(config: &Arc<Config>, public: Vec<u8>, team_view: Option<(u8, Vec<u8>)>, name: String, version: Version) -> Result<GameType, ZipperError> {
    let mut unzipper = Unzipper::new(vec![public[0]], version.clone());
    if unzipper.read_bool()? {
        Ok(GameType::Hex(*Game::import_client(public, team_view, config, name, version)?))
    } else {
        Ok(GameType::Square(*Game::import_client(public, team_view, config, name, version)?))
    }
}

#[cfg(test)]
mod tests {

    use std::sync::Arc;

    use interfaces::game_interface::*;
    use interfaces::map_interface::*;
    use semver::Version;
    use crate::config::config::Config;
    use crate::game::game::*;
    use crate::game::fog::*;
    use crate::map::direction::*;
    use crate::map::map::Map;
    use crate::map::point::Point;
    use crate::map::point::Position;
    use crate::map::point_map::PointMap;
    use crate::map::wrapping_map::WrappingMapBuilder;
    use crate::terrain::TerrainType;
    use crate::units::unit_types::UnitType;
    use crate::VERSION;

    #[test]
    fn export_import_chess() {
        let version = Version::parse(VERSION).unwrap();
        let config = Arc::new(Config::test_config());
        let map = PointMap::new(8, 8, false);
        let map = WrappingMapBuilder::<Direction4>::new(map, vec![]);
        let mut map = Map::new(map.build().unwrap(), &config);
        let environment = map.environment().clone();
        for p in map.all_points() {
            if p.y == 1 || p.y == 6 {
                map.set_terrain(p, TerrainType::ChessPawnTile.instance(&environment).build_with_defaults());
            } else {
                map.set_terrain(p, TerrainType::ChessTile.instance(&environment).build_with_defaults());
            }
        }
        map.set_unit(Point::new(0, 0), Some(UnitType::Rook.instance(&environment).set_owner_id(1).build_with_defaults()));
        map.set_unit(Point::new(7, 0), Some(UnitType::Rook.instance(&environment).set_owner_id(1).build_with_defaults()));
        map.set_unit(Point::new(0, 7), Some(UnitType::Rook.instance(&environment).set_owner_id(0).build_with_defaults()));
        map.set_unit(Point::new(7, 7), Some(UnitType::Rook.instance(&environment).set_owner_id(0).build_with_defaults()));
        
        map.set_unit(Point::new(1, 0), Some(UnitType::Knight.instance(&environment).set_owner_id(1).build_with_defaults()));
        map.set_unit(Point::new(6, 0), Some(UnitType::Knight.instance(&environment).set_owner_id(1).build_with_defaults()));
        map.set_unit(Point::new(1, 7), Some(UnitType::Knight.instance(&environment).set_owner_id(0).build_with_defaults()));
        map.set_unit(Point::new(6, 7), Some(UnitType::Knight.instance(&environment).set_owner_id(0).build_with_defaults()));
        
        map.set_unit(Point::new(2, 0), Some(UnitType::Bishop.instance(&environment).set_owner_id(1).build_with_defaults()));
        map.set_unit(Point::new(5, 0), Some(UnitType::Bishop.instance(&environment).set_owner_id(1).build_with_defaults()));
        map.set_unit(Point::new(2, 7), Some(UnitType::Bishop.instance(&environment).set_owner_id(0).build_with_defaults()));
        map.set_unit(Point::new(5, 7), Some(UnitType::Bishop.instance(&environment).set_owner_id(0).build_with_defaults()));

        map.set_unit(Point::new(3, 0), Some(UnitType::Queen.instance(&environment).set_owner_id(1).build_with_defaults()));
        map.set_unit(Point::new(4, 0), Some(UnitType::King.instance(&environment).set_owner_id(1).build_with_defaults()));
        map.set_unit(Point::new(3, 7), Some(UnitType::Queen.instance(&environment).set_owner_id(0).build_with_defaults()));
        map.set_unit(Point::new(4, 7), Some(UnitType::King.instance(&environment).set_owner_id(0).build_with_defaults()));
        
        for x in 0..8 {
            map.set_unit(Point::new(x, 1), Some(UnitType::Pawn.instance(&environment).set_direction(Direction4::D270).set_owner_id(1).build_with_defaults()));
            map.set_unit(Point::new(x, 6), Some(UnitType::Pawn.instance(&environment).set_direction(Direction4::D90).set_owner_id(0).build_with_defaults()));
        }

        let settings = map.settings().unwrap();

        for fog_setting in [FogSetting::None, FogSetting::Sharp(0)] {
            println!("fog setting: {fog_setting}");
            let mut settings = settings.clone();
            settings.fog_mode = FogMode::Constant(fog_setting);
            let perspective = Perspective::Team(0);
            let (server, events) = map.clone().game_server(&settings, || 0.);
            let client = map.clone().game_client(&settings, events.get(&perspective).unwrap().0);
            let data = server.export();
            println!("data: {data:?}");
            let imported_server = Game::import_server(data.clone(), &config, "".to_string(), version.clone()).unwrap();
            assert_eq!(server.get_fog(), imported_server.get_fog());
            assert_eq!(server.environment(), imported_server.environment());
            assert_eq!(Box::new(server), imported_server);
            assert_eq!(Ok(Box::new(client)), Game::import_client(data.public.clone(), data.get_team(0), &config, "".to_string(), version.clone()));
        }
    }
}
