use std::sync::Arc;

use interfaces::{Perspective, GameEventsMap};

use crate::config::config::Config;
use crate::game::commands::Command;
use crate::game::fog::*;
use crate::game::game::Game;
use crate::game::game_view::GameView;
use crate::handle::Handle;
use crate::map::direction::*;
use crate::map::map::Map;
use crate::map::point::*;
use crate::map::point_map::PointMap;
use crate::map::wrapping_map::*;
use crate::script::custom_action::CustomActionData;
use crate::terrain::TerrainType;
use crate::units::attributes::ActionStatus;
use crate::units::combat::AttackVector;
use crate::units::commands::*;
use crate::units::movement::Path;
use crate::units::unit::Unit;
use crate::units::unit_types::UnitType;

#[test]
fn unit_builder_transported() {
    let config = Arc::new(Config::test_config());
    let map = WMBuilder::<Direction4>::new(PointMap::new(5, 5, false));
    let map = Map::new(map.build(), &config);
    let map_env = map.environment().clone();
    let unit: Unit<Direction4> = UnitType::transport_heli().instance(&map_env).set_owner_id(0).set_transported(vec![
        UnitType::marine().instance(&map_env).set_hp(34).build_with_defaults(),
    ]).build_with_defaults();
    assert_eq!(unit.get_transported().len(), 1);
}

#[test]
fn fog_replacement() {
    let config = Arc::new(Config::test_config());
    let map = PointMap::new(5, 5, false);
    let map = WMBuilder::<Direction4>::new(map);
    let mut map = Map::new(map.build(), &config);
    let map_env = map.environment().clone();
    map.set_terrain(Point::new(1, 1), TerrainType::City.instance(&map_env).build_with_defaults());
    let unit = UnitType::sniper().instance(&map_env).set_owner_id(0).set_status(ActionStatus::Capturing).build_with_defaults();
    let map_view = Handle::new(map);
    assert_eq!(
        unit.fog_replacement(&map_view, Point::new(1, 1), FogIntensity::Light),
        Some(map_env.config.unknown_unit().instance(&map_env).build_with_defaults())
    );
}

#[test]
fn build_drone() {
    let config = Arc::new(Config::test_config());
    let map = PointMap::new(5, 7, false);
    let map = WMBuilder::<Direction6>::new(map);
    let mut map = Map::new(map.build(), &config);
    let map_env = map.environment().clone();
    for x in 0..5 {
        for y in 0..7 {
            map.set_terrain(Point::new(x, y), TerrainType::Sea.instance(&map_env).build_with_defaults());
        }
    }
    map.set_unit(Point::new(3, 4), Some(UnitType::drone_boat().instance(&map_env).set_owner_id(0).build_with_defaults()));
    map.set_unit(Point::new(1, 6), Some(UnitType::war_ship().instance(&map_env).set_owner_id(1).build_with_defaults()));
    let mut settings = map.settings().unwrap();
    settings.fog_mode = FogMode::Constant(FogSetting::None);
    settings.players[0].set_funds(1000);
    let (mut server, events) = Game::new_server(map.clone(), settings.build_default(), Arc::new(|| 0.));
    let mut client = Game::new_client(map, settings.build_default(), events.get(&Perspective::Team(0)).unwrap());
    let to_build = UnitType::light_drone().instance(&server.environment())
        .set_owner_id(0)
        .build_with_defaults();
    let events = server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(Point::new(3, 4)),
        action: UnitAction::custom(0, vec![CustomActionData::Unit(to_build)]),
    }), Arc::new(|| 0.)).unwrap();
    client.with_mut(|client| {
        for ev in events.get(&Perspective::Team(0)).unwrap() {
            ev.apply(client);
        }
    });
    assert_eq!(
        server.get_unit(Point::new(3, 4)).unwrap().get_transported().len(),
        1
    );
    assert_eq!(
        client.get_unit(Point::new(3, 4)).unwrap().get_transported().len(),
        1
    );
}

#[test]
fn repair_unit() {
    let config = Arc::new(Config::test_config());
    let map = PointMap::new(5, 7, false);
    let map = WMBuilder::<Direction6>::new(map);
    let mut map = Map::new(map.build(), &config);
    let map_env = map.environment().clone();
    for x in 0..5 {
        for y in 0..7 {
            map.set_terrain(Point::new(x, y), TerrainType::Grass.instance(&map_env).build_with_defaults());
        }
    }
    map.set_terrain(Point::new(3, 4), TerrainType::Factory.instance(&map_env).set_owner_id(0).build_with_defaults());
    map.set_unit(Point::new(3, 4), Some(UnitType::small_tank().instance(&map_env).set_owner_id(0).set_hp(1).build_with_defaults()));
    map.set_unit(Point::new(1, 6), Some(UnitType::war_ship().instance(&map_env).set_owner_id(1).build_with_defaults()));
    let mut settings = map.settings().unwrap();
    settings.fog_mode = FogMode::Constant(FogSetting::None);
    settings.players[0].set_funds(1000);
    let (mut server, _) = Game::new_server(map.clone(), settings.build_default(), Arc::new(|| 0.));
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(Point::new(3, 4)),
        action: UnitAction::custom(1, Vec::new()),
    }), Arc::new(|| 0.)).unwrap();
    assert!(*server.get_owning_player(0).unwrap().funds < 1000);
    assert!(server.get_unit(Point::new(3, 4)).unwrap().get_hp() > 1);
    assert_eq!(server.get_unit(Point::new(3, 4)).unwrap().get_status(), ActionStatus::Repairing);
    server.handle_command(Command::EndTurn, Arc::new(|| 0.)).unwrap();
    server.handle_command(Command::EndTurn, Arc::new(|| 0.)).unwrap();
    assert_eq!(server.get_unit(Point::new(3, 4)).unwrap().get_status(), ActionStatus::Ready);
}


#[test]
fn end_game() {
    let config = Arc::new(Config::test_config());
    let map = PointMap::new(4, 4, false);
    let wmap: WrappingMap<Direction4> = WMBuilder::new(map).build();
    let mut map = Map::new(wmap, &config);
    let map_env = map.environment().clone();
    map.set_unit(Point::new(0, 0), Some(UnitType::small_tank().instance(&map_env).set_owner_id(0).build_with_defaults()));
    map.set_unit(Point::new(0, 1), Some(UnitType::small_tank().instance(&map_env).set_owner_id(1).set_hp(1).build_with_defaults()));
    let settings = map.settings().unwrap();
    let (mut game, _) = Game::new_server(map, settings.build_default(), Arc::new(|| 0.));
    game.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(Point::new(0, 0)),
        action: UnitAction::Attack(AttackVector::Direction(Direction4::D270)),
    }), Arc::new(|| 0.)).unwrap();
    game.with(|game| {
        assert!(game.has_ended());
        for (i, player) in game.players.iter().enumerate() {
            assert_eq!(player.dead, i != 0);
        }
    });
}

#[test]
fn defeat_player_of_3() {
    let config = Arc::new(Config::test_config());
    let map = PointMap::new(4, 4, false);
    let wmap: WrappingMap<Direction4> = WMBuilder::new(map).build();
    let mut map = Map::new(wmap, &config);
    let map_env = map.environment().clone();
    map.set_unit(Point::new(0, 0), Some(UnitType::small_tank().instance(&map_env).set_owner_id(0).set_hp(1).build_with_defaults()));
    map.set_unit(Point::new(0, 1), Some(UnitType::small_tank().instance(&map_env).set_owner_id(1).build_with_defaults()));
    map.set_unit(Point::new(0, 2), Some(UnitType::small_tank().instance(&map_env).set_owner_id(2).build_with_defaults()));
    let settings = map.settings().unwrap();
    let (mut game, _) = Game::new_server(map, settings.build_default(), Arc::new(|| 0.));
    game.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(Point::new(0, 0)),
        action: UnitAction::Attack(AttackVector::Direction(Direction4::D270)),
    }), Arc::new(|| 0.)).unwrap();
    game.with(|game| {
        assert!(!game.has_ended());
        for (i, player) in game.players.iter().enumerate() {
            assert_eq!(player.dead, i == 0);
        }
    });
    assert_eq!(game.current_owner(), 1);
    game.handle_command(Command::EndTurn, Arc::new(|| 0.)).unwrap();
    assert_eq!(game.current_owner(), 2);
    game.handle_command(Command::EndTurn, Arc::new(|| 0.)).unwrap();
    assert_eq!(game.current_owner(), 1);
}

#[test]
fn on_death_lose_game() {
    let config = Arc::new(Config::test_config());
    let map = PointMap::new(4, 4, false);
    let wmap: WrappingMap<Direction4> = WMBuilder::new(map).build();
    let mut map = Map::new(wmap, &config);
    let map_env = map.environment().clone();
    map.set_unit(Point::new(0, 0), Some(UnitType::small_tank().instance(&map_env).set_owner_id(0).build_with_defaults()));
    map.set_unit(Point::new(0, 1), Some(UnitType::life_crystal().instance(&map_env).set_owner_id(1).set_hp(1).build_with_defaults()));
    map.set_unit(Point::new(0, 2), Some(UnitType::small_tank().instance(&map_env).set_owner_id(1).build_with_defaults()));
    map.set_unit(Point::new(0, 3), Some(UnitType::small_tank().instance(&map_env).set_owner_id(1).build_with_defaults()));
    let settings = map.settings().unwrap();
    let (mut game, _) = Game::new_server(map, settings.build_default(), Arc::new(|| 0.));
    game.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(Point::new(0, 0)),
        action: UnitAction::Attack(AttackVector::Direction(Direction4::D270)),
    }), Arc::new(|| 0.)).unwrap();
    game.with(|game| {
        assert!(game.has_ended());
        for (i, player) in game.players.iter().enumerate() {
            assert_eq!(player.dead, i != 0);
        }
    });
}

#[test]
fn puffer_fish() {
    let config = Arc::new(Config::test_config());
    let map = PointMap::new(4, 4, false);
    let wmap: WrappingMap<Direction4> = WMBuilder::new(map).build();
    let mut map = Map::new(wmap, &config);
    let map_env = map.environment().clone();
    let sea = TerrainType::ShallowSea.instance(&map_env).build_with_defaults();
    // experiment
    map.set_terrain(Point::new(1, 1), sea.clone());
    map.set_terrain(Point::new(2, 1), sea.clone());
    map.set_unit(Point::new(0, 1), Some(UnitType::small_tank().instance(&map_env).set_owner_id(0).build_with_defaults()));
    map.set_unit(Point::new(0, 2), Some(UnitType::artillery().instance(&map_env).set_owner_id(0).build_with_defaults()));
    map.set_unit(Point::new(1, 1), Some(UnitType::puffer_fish().instance(&map_env).build_with_defaults()));
    map.set_unit(Point::new(2, 0), Some(UnitType::small_tank().instance(&map_env).set_owner_id(1).build_with_defaults()));
    let settings = map.settings().unwrap();
    let (mut game, _) = Game::new_server(map, settings.build_default(), Arc::new(|| 0.));
    game.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(Point::new(0, 1)),
        action: UnitAction::Attack(AttackVector::Direction(Direction4::D0)),
    }), Arc::new(|| 0.)).unwrap();
    assert_eq!(game.get_unit(Point::new(2, 1)).unwrap().typ(), UnitType::puffer_fish());
    assert_eq!(game.get_unit(Point::new(0, 1)).unwrap().get_hp(), 100);
    assert_eq!(game.get_unit(Point::new(2, 1)).unwrap().get_hp(), 100);
    let hp = game.get_unit(Point::new(2, 0)).unwrap().get_hp();
    assert!(hp < 100);
    game.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(Point::new(0, 2)),
        action: UnitAction::Attack(AttackVector::Point(Point::new(2, 1))),
    }), Arc::new(|| 0.)).unwrap();
    assert_eq!(game.get_unit(Point::new(2, 1)).unwrap().get_hp(), 100);
    assert!(game.get_unit(Point::new(2, 0)).unwrap().get_hp() < hp);
}


#[test]
fn capture_pyramid() {
    let config = Arc::new(Config::test_config());
    let map = PointMap::new(4, 4, false);
    let wmap: WrappingMap<Direction4> = WMBuilder::new(map).build();
    let mut map = Map::new(wmap, &config);
    let map_env = map.environment().clone();
    map.set_unit(Point::new(0, 0), Some(UnitType::small_tank().instance(&map_env).set_owner_id(0).build_with_defaults()));
    map.set_unit(Point::new(0, 1), Some(UnitType::pyramid().instance(&map_env).set_owner_id(1).set_hp(1).build_with_defaults()));
    map.set_unit(Point::new(0, 2), Some(UnitType::small_tank().instance(&map_env).set_owner_id(0).build_with_defaults()));
    map.set_unit(Point::new(0, 3), Some(UnitType::small_tank().instance(&map_env).set_owner_id(1).build_with_defaults()));
    let settings = map.settings().unwrap();
    let (mut game, _) = Game::new_server(map, settings.build_default(), Arc::new(|| 0.));
    game.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(Point::new(0, 0)),
        action: UnitAction::Attack(AttackVector::Direction(Direction4::D270)),
    }), Arc::new(|| 0.)).unwrap();
    assert_eq!(game.get_unit(Point::new(0, 1)).unwrap().get_owner_id(), -1);
    game.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(Point::new(0, 2)),
        action: UnitAction::Attack(AttackVector::Direction(Direction4::D90)),
    }), Arc::new(|| 0.)).unwrap();
    assert_eq!(game.get_unit(Point::new(0, 1)).unwrap().get_owner_id(), 0);
}

#[test]
fn s_factory() {
    let config = Arc::new(Config::test_config());
    let map = PointMap::new(4, 4, false);
    let wmap: WrappingMap<Direction6> = WMBuilder::new(map).build();
    let mut map = Map::new(wmap, &config);
    let map_env = map.environment().clone();
    map.set_unit(Point::new(1, 1), Some(UnitType::factory().instance(&map_env).set_owner_id(0).set_hp(1).build_with_defaults()));
    map.set_unit(Point::new(1, 3), Some(UnitType::pyramid().instance(&map_env).set_owner_id(0).set_hp(1).build_with_defaults()));
    map.set_unit(Point::new(0, 3), Some(UnitType::small_tank().instance(&map_env).set_owner_id(1).build_with_defaults()));
    let settings = map.settings().unwrap();
    let (mut game, _) = Game::new_server(map, settings.build_default(), Arc::new(|| 0.));
    let environment = game.environment().clone();
    game.with(|game| {
        assert_eq!(*game.current_player().funds, game.current_player().get_income() * 2);
    });
    assert_ne!(game.get_unit(Point::new(1, 1)).unwrap().get_status(), ActionStatus::Exhausted);
    let to_build = UnitType::marine().instance(&game.environment())
        .set_owner_id(0)
        .build_with_defaults();
    game.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(Point::new(1, 1)),
        action: UnitAction::custom(0, vec![CustomActionData::Unit(to_build), CustomActionData::Direction(Direction6::D180)]),
    }), Arc::new(|| 0.)).unwrap();
    assert_eq!(game.get_unit(Point::new(0, 1)).unwrap(), UnitType::marine().instance(&environment).set_owner_id(0). set_status(ActionStatus::Exhausted).build_with_defaults());
    assert_eq!(game.get_unit(Point::new(1, 1)).unwrap().get_status(), ActionStatus::Exhausted);
}
