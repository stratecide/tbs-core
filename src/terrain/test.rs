use std::sync::Arc;

use crate::config::config::Config;
use crate::config::environment::Environment;
use crate::game::commands::Command;
use crate::game::game::Game;
use crate::game::game_view::GameView;
use crate::map::direction::*;
use crate::map::map::Map;
use crate::map::point::*;
use crate::map::point_map::PointMap;
use crate::terrain::TerrainType;
use crate::map::wrapping_map::*;
use crate::units::combat::AttackVector;
use crate::units::commands::{UnitCommand, UnitAction};
use crate::units::movement::{Path, PathStep};
use crate::units::unit_types::UnitType;


#[test]
fn capture_city() {
    let map = PointMap::new(4, 4, false);
    let environment = Environment::new_map(Arc::new(Config::test_config()), map.size());
    let wmap: WrappingMap<Direction4> = WMBuilder::new(map).build();
    let mut map = Map::new2(wmap, &environment);
    map.set_terrain(Point::new(1, 1), TerrainType::City.instance(&environment).set_owner_id(-1).build_with_defaults());
    map.set_unit(Point::new(0, 0), Some(UnitType::Sniper.instance(&environment).set_owner_id(0).build_with_defaults()));
    map.set_unit(Point::new(3, 3), Some(UnitType::Sniper.instance(&environment).set_owner_id(1).build_with_defaults()));
    let settings = map.settings().unwrap().build_default();
    let (mut game, _) = Game::new_server(map, settings, Arc::new(|| 0.));
    game.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::with_steps(Point::new(0, 0), vec![
            PathStep::Dir(Direction4::D0),
            PathStep::Dir(Direction4::D270),
        ]),
        action: UnitAction::Capture,
    }), Arc::new(|| 0.)).unwrap();
    game.handle_command(Command::EndTurn, Arc::new(|| 0.)).unwrap();
    game.handle_command(Command::EndTurn, Arc::new(|| 0.)).unwrap();
    assert_eq!(0, game.get_terrain(Point::new(1, 1)).unwrap().get_owner_id());
}

#[test]
fn build_unit() {
    let map = PointMap::new(4, 4, false);
    let environment = Environment::new_map(Arc::new(Config::test_config()), map.size());
    let wmap: WrappingMap<Direction4> = WMBuilder::new(map).build();
    let mut map = Map::new2(wmap, &environment);
    map.set_terrain(Point::new(0, 0), TerrainType::Factory.instance(&environment).set_owner_id(0).build_with_defaults());
    map.set_terrain(Point::new(1, 1), TerrainType::City.instance(&environment).set_owner_id(0).build_with_defaults());
    map.set_unit(Point::new(3, 3), Some(UnitType::Sniper.instance(&environment).set_owner_id(1).build_with_defaults()));
    let mut settings = map.settings().unwrap();
    settings.players[0].set_income(1000);
    let (mut game, _) = Game::new_server(map, settings.build_default(), Arc::new(|| 0.));
    assert_eq!(1000, game.with(|game| *game.current_player().funds));
    game.handle_command(Command::BuyUnit(Point::new(0, 0), UnitType::Marine, Direction4::D0), Arc::new(|| 0.)).unwrap();
    assert!(game.with(|game| *game.current_player().funds) < 1000);
    assert_eq!(0, game.get_unit(Point::new(0, 0)).unwrap().get_owner_id());
    assert!(game.get_unit(Point::new(0, 0)).unwrap().is_exhausted());
    assert_eq!(1, game.get_terrain(Point::new(0, 0)).unwrap().get_built_this_turn());
}

#[test]
fn kraken() {
    let map = PointMap::new(5, 5, false);
    let map_env = Environment::new_map(Arc::new(Config::test_config()), map.size());
    let wmap: WrappingMap<Direction4> = WMBuilder::new(map).build();
    let mut map = Map::new2(wmap, &map_env);
    for x in 0..map.width() {
        for y in 0..map.height() {
            map.set_terrain(Point::new(x, y), TerrainType::TentacleDepths.instance(&map_env).build_with_defaults());
        }
    }
    map.set_terrain(Point::new(2, 2), TerrainType::Kraken.instance(&map_env).set_anger(7).build_with_defaults());
    map.set_unit(Point::new(2, 1), Some(UnitType::WarShip.instance(&map_env).set_owner_id(1).build_with_defaults()));
    map.set_unit(Point::new(1, 2), Some(UnitType::WarShip.instance(&map_env).set_owner_id(0).build_with_defaults()));
    map.set_unit(Point::new(3, 2), Some(UnitType::WarShip.instance(&map_env).set_owner_id(0).build_with_defaults()));
    let settings = map.settings().unwrap();
    let (mut game, _) = Game::new_server(map, settings.build_default(), Arc::new(|| 0.));
    let environment = game.environment();
    assert_eq!(game.get_unit(Point::new(0, 0)), Some(UnitType::Tentacle.instance(&environment).build_with_defaults()));
    assert_eq!(game.get_terrain(Point::new(2, 2)).unwrap().get_anger(), 7);
    game.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(Point::new(3, 2)),
        action: UnitAction::Attack(AttackVector::Direction(Direction4::D0)),
    }), Arc::new(|| 0.)).unwrap();
    assert_eq!(game.get_unit(Point::new(4, 2)), None);
    assert_eq!(game.get_terrain(Point::new(2, 2)).unwrap().get_anger(), 8);
    assert_eq!(game.get_unit(Point::new(3, 2)).unwrap().get_hp(), 100);
    game.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(Point::new(1, 2)),
        action: UnitAction::Attack(AttackVector::Direction(Direction4::D180)),
    }), Arc::new(|| 0.)).unwrap();
    assert_eq!(game.get_terrain(Point::new(2, 2)).unwrap().get_anger(), 0);
    assert!(game.get_unit(Point::new(3, 2)).unwrap().get_hp() < 100);
}
