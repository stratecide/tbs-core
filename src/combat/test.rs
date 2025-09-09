use uniform_smart_pointer::Urc;

use crate::config::config::Config;
use crate::config::environment::Environment;
use crate::game::commands::Command;
use crate::game::game::Game;
use crate::map::direction::Direction4;
use crate::map::map::Map;
use crate::map::point::{Point, Position};
use crate::map::point_map::PointMap;
use crate::map::wrapping_map::*;
use crate::terrain::TerrainType;
use crate::units::commands::{UnitCommand, UnitAction};
use crate::units::movement::Path;
use crate::units::unit_types::UnitType;

use super::*;

#[test]
fn hp_factor() {
    let map = PointMap::new(4, 4, false);
    let environment = Environment::new_map(Urc::new(Config::default()), map.size());
    let wmap: WrappingMap<Direction4> = WMBuilder::new(map).build();
    let mut map = Map::new2(wmap, &environment);
    for p in map.all_points() {
        map.set_terrain(p, TerrainType::Street.instance(&environment).build());
    }
    map.set_unit(Point::new(0, 0), Some(UnitType::bazooka().instance(&environment).set_owner_id(1).set_hp(100).build()));
    map.set_unit(Point::new(1, 0), Some(UnitType::bazooka().instance(&environment).set_owner_id(1).set_hp(100).build()));
    map.set_unit(Point::new(2, 0), Some(UnitType::bazooka().instance(&environment).set_owner_id(1).set_hp(100).build()));
    map.set_unit(Point::new(3, 0), Some(UnitType::bazooka().instance(&environment).set_owner_id(1).set_hp(100).build()));
    map.set_unit(Point::new(0, 1), Some(UnitType::bazooka().instance(&environment).set_owner_id(0).set_hp(100).build()));
    map.set_unit(Point::new(1, 1), Some(UnitType::bazooka().instance(&environment).set_owner_id(0).set_hp(75).build()));
    map.set_unit(Point::new(2, 1), Some(UnitType::bazooka().instance(&environment).set_owner_id(0).set_hp(50).build()));
    map.set_unit(Point::new(3, 1), Some(UnitType::bazooka().instance(&environment).set_owner_id(0).set_hp(25).build()));
    let map_settings = map.settings().unwrap();
    let settings = map_settings.build_default();
    let (mut game, _) = Game::new_server(map, &map_settings, settings, Urc::new(|| 0.));
    for x in 0..4 {
        game.handle_command(Command::UnitCommand(UnitCommand {
            unload_index: None,
            path: Path::new(Point::new(x, 1)),
            action: UnitAction::Attack(AttackInput::SplashPattern(OrientedPoint::simple(Point::new(x, 0), Direction4::D90))),
        }), Urc::new(|| 0.)).unwrap();
    }
    let base_damage = 100. - game.get_unit(Point::new(0, 0)).unwrap().get_hp() as f32;
    crate::debug!("base_damage is {base_damage}");
    assert!(base_damage > 0.);
    assert_eq!(100 - (base_damage * 0.75).ceil() as u8, game.get_unit(Point::new(1, 0)).unwrap().get_hp());
    assert_eq!(100 - (base_damage * 0.50).ceil() as u8, game.get_unit(Point::new(2, 0)).unwrap().get_hp());
    assert_eq!(100 - (base_damage * 0.25).ceil() as u8, game.get_unit(Point::new(3, 0)).unwrap().get_hp());
}

#[test]
fn terrain_defense() {
    let map = PointMap::new(4, 4, false);
    let environment = Environment::new_map(Urc::new(Config::default()), map.size());
    let wmap: WrappingMap<Direction4> = WMBuilder::new(map).build();
    let mut map = Map::new2(wmap, &environment);
    map.set_terrain(Point::new(0, 0), TerrainType::Street.instance(&environment).build());
    map.set_terrain(Point::new(1, 0), TerrainType::Grass.instance(&environment).build());
    map.set_terrain(Point::new(2, 0), TerrainType::Forest.instance(&environment).build());
    map.set_terrain(Point::new(3, 0), TerrainType::Mountain.instance(&environment).build());
    map.set_unit(Point::new(0, 0), Some(UnitType::bazooka().instance(&environment).set_owner_id(1).set_hp(100).build()));
    map.set_unit(Point::new(1, 0), Some(UnitType::bazooka().instance(&environment).set_owner_id(1).set_hp(100).build()));
    map.set_unit(Point::new(2, 0), Some(UnitType::bazooka().instance(&environment).set_owner_id(1).set_hp(100).build()));
    map.set_unit(Point::new(3, 0), Some(UnitType::bazooka().instance(&environment).set_owner_id(1).set_hp(100).build()));
    map.set_unit(Point::new(0, 1), Some(UnitType::bazooka().instance(&environment).set_owner_id(0).set_hp(100).build()));
    map.set_unit(Point::new(1, 1), Some(UnitType::bazooka().instance(&environment).set_owner_id(0).set_hp(100).build()));
    map.set_unit(Point::new(2, 1), Some(UnitType::bazooka().instance(&environment).set_owner_id(0).set_hp(100).build()));
    map.set_unit(Point::new(3, 1), Some(UnitType::bazooka().instance(&environment).set_owner_id(0).set_hp(100).build()));
    let map_settings = map.settings().unwrap();
    let settings = map_settings.build_default();
    let (mut game, _) = Game::new_server(map,&map_settings, settings, Urc::new(|| 0.));
    for x in 0..4 {
        game.handle_command(Command::UnitCommand(UnitCommand {
            unload_index: None,
            path: Path::new(Point::new(x, 1)),
            action: UnitAction::Attack(AttackInput::SplashPattern(OrientedPoint::simple(Point::new(x, 0), Direction4::D90))),
        }), Urc::new(|| 0.)).unwrap();
        crate::debug!("attacker hp: {}, defender hp: {}", game.get_unit(Point::new(x, 1)).unwrap().get_hp(), game.get_unit(Point::new(x, 0)).unwrap().get_hp());
    }
    let base_damage = 100. - game.get_unit(Point::new(0, 0)).unwrap().get_hp() as f32;
    assert!(base_damage > 0.);
    assert_eq!((base_damage / 1.1).ceil() as u8, 100 - game.get_unit(Point::new(1, 0)).unwrap().get_hp());
    assert_eq!((base_damage / 1.2).ceil() as u8, 100 - game.get_unit(Point::new(2, 0)).unwrap().get_hp());
    assert_eq!((base_damage / 1.3).ceil() as u8, 100 - game.get_unit(Point::new(3, 0)).unwrap().get_hp());
}

#[test]
fn displacement() {
    let map = PointMap::new(5, 4, false);
    let environment = Environment::new_map(Urc::new(Config::default()), map.size());
    let wmap: WrappingMap<Direction4> = WMBuilder::new(map).build();
    let mut map = Map::new2(wmap, &environment);
    map.set_unit(Point::new(1, 0), Some(UnitType::magnet().instance(&environment).set_owner_id(0).set_hp(100).build()));
    map.set_unit(Point::new(3, 0), Some(UnitType::sniper().instance(&environment).set_owner_id(0).set_hp(100).build()));
    map.set_unit(Point::new(1, 1), Some(UnitType::destroyer().instance(&environment).set_owner_id(0).set_hp(100).build()));
    map.set_unit(Point::new(2, 1), Some(UnitType::destroyer().instance(&environment).set_owner_id(1).set_hp(100).build()));
    //map.set_unit(Point::new(3, 1), Some(UnitType::destroyer().instance(&environment).set_owner_id(1).set_hp(100).build_with_defaults()));
    map.set_unit(Point::new(1, 2), Some(UnitType::war_ship().instance(&environment).set_owner_id(1).set_hp(100).build()));
    let map_settings = map.settings().unwrap();
    let settings = map_settings.build_default();
    let (mut game, _) = Game::new_server(map,&map_settings, settings, Urc::new(|| 0.));
    let unchanged = game.clone();

    game.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(Point::new(1, 0)),
        action: UnitAction::Attack(AttackInput::SplashPattern(OrientedPoint::simple(Point::new(3, 0), Direction4::D0))),
    }), Urc::new(|| 0.)).unwrap();
    assert_eq!(100, game.get_unit(Point::new(1, 0)).unwrap().get_hp());
    assert_eq!(100, game.get_unit(Point::new(2, 0)).unwrap().get_hp());
    assert_eq!(None, game.get_unit(Point::new(3, 0)));
    game.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(Point::new(1, 1)),
        action: UnitAction::Attack(AttackInput::SplashPattern(OrientedPoint::simple(Point::new(2, 1), Direction4::D0))),
    }), Urc::new(|| 0.)).unwrap();
    //assert!(game.get_unit(Point::new(0, 1)).unwrap().get_hp() < 100);
    for x in 2..=2 { // 1..=3 if 2 range and counter-attack happens before displacement
        assert_eq!(None, game.get_unit(Point::new(x, 1)), "x = {x}");
    }
    //assert!(game.get_unit(Point::new(4, 1)).unwrap().get_hp() < 100);

    // WarShip can't be displaced
    let mut game = unchanged.clone();
    game.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(Point::new(1, 1)),
        action: UnitAction::Attack(AttackInput::SplashPattern(OrientedPoint::simple(Point::new(1, 2), Direction4::D270))),
    }), Urc::new(|| 0.)).unwrap();
    assert!(game.get_unit(Point::new(1, 2)).unwrap().get_hp() < 100);
}

#[test]
fn dragon_head() {
    // create map
    let config = Urc::new(Config::default());
    let map = PointMap::new(4, 4, false);
    let wmap: WrappingMap<Direction4> = WMBuilder::new(map).build();
    let mut map = Map::new(wmap, &config);
    let map_env = map.environment().clone();
    map.set_unit(Point::new(0, 0), Some(UnitType::dragon_head().instance(&map_env).set_owner_id(0).set_hp(100).build()));
    map.set_unit(Point::new(1, 0), Some(UnitType::sniper().instance(&map_env).set_owner_id(1).set_hp(100).build()));
    map.set_unit(Point::new(2, 0), Some(UnitType::sniper().instance(&map_env).set_owner_id(1).set_hp(100).build()));
    map.set_unit(Point::new(3, 0), Some(UnitType::sniper().instance(&map_env).set_owner_id(1).set_hp(100).build()));
    // create game
    let map_settings = map.settings().unwrap();
    let settings = map_settings.build_default();
    let (mut game, _) = Game::new_server(map,&map_settings, settings, Urc::new(|| 0.));
    let unchanged = game.clone();
    game.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(Point::new(0, 0)),
        action: UnitAction::Attack(AttackInput::AttackPattern(Point::new(1, 0), Direction4::D0)),
    }), Urc::new(|| 0.)).unwrap();
    let hp1 = game.get_unit(Point::new(1, 0)).unwrap().get_hp();
    let hp2 = game.get_unit(Point::new(2, 0)).unwrap().get_hp();
    assert!(hp1 < 100);
    assert_eq!(hp1, hp2);
    assert_eq!(100, game.get_unit(Point::new(3, 0)).unwrap().get_hp());
    // target the other enemy
    game = unchanged;
    game.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(Point::new(0, 0)),
        action: UnitAction::Attack(AttackInput::AttackPattern(Point::new(2, 0), Direction4::D0)),
    }), Urc::new(|| 0.)).unwrap();
    assert_eq!(hp1, game.get_unit(Point::new(1, 0)).unwrap().get_hp());
    assert_eq!(hp2, game.get_unit(Point::new(2, 0)).unwrap().get_hp());
    assert_eq!(100, game.get_unit(Point::new(3, 0)).unwrap().get_hp());
}

#[test]
fn cannot_attack_friendly() {
    let map = PointMap::new(4, 4, false);
    let environment = Environment::new_map(Urc::new(Config::default()), map.size());
    let wmap: WrappingMap<Direction4> = WMBuilder::new(map).build();
    let mut map = Map::new2(wmap, &environment);
    for p in map.all_points() {
        map.set_terrain(p, TerrainType::Street.instance(&environment).build());
    }
    map.set_unit(Point::new(3, 0), Some(UnitType::bazooka().instance(&environment).set_owner_id(1).set_hp(100).build()));
    map.set_unit(Point::new(0, 1), Some(UnitType::bazooka().instance(&environment).set_owner_id(0).set_hp(100).build()));
    map.set_unit(Point::new(1, 1), Some(UnitType::bazooka().instance(&environment).set_owner_id(0).set_hp(100).build()));
    let map_settings = map.settings().unwrap();
    let settings = map_settings.build_default();
    let (mut game, _) = Game::new_server(map,&map_settings, settings, Urc::new(|| 0.));
    let path = Path::new(Point::new(0, 1));
    let board = Board::new(&game);
    let options = game.get_unit(Point::new(0, 1)).unwrap().options_after_path(&board, &path, None, &[]);
    assert_eq!(options, vec![UnitAction::Wait]);
    game.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: path.clone(),
        action: UnitAction::Attack(AttackInput::SplashPattern(OrientedPoint::simple(Point::new(1, 1), Direction4::D0))),
    }), Urc::new(|| 0.)).unwrap_err();
    game.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path,
        action: UnitAction::Attack(AttackInput::AttackPattern(Point::new(1, 1), Direction4::D0)),
    }), Urc::new(|| 0.)).unwrap_err();
}
