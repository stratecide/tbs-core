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
use crate::map::test::chess_board;
use crate::map::wrapping_map::*;
use crate::script::custom_action::test::{CA_UNIT_BUILD_UNIT, CA_UNIT_REPAIR};
use crate::script::custom_action::CustomActionInput;
use crate::tags::{Int32, TagValue};
use crate::tags::tests::*;
use crate::terrain::TerrainType;
use crate::tokens::token::Token;
use crate::tokens::token_types::TokenType;
use crate::units::combat::AttackVector;
use crate::units::commands::*;
use crate::units::movement::{Path, PathStep};
use crate::units::unit::*;
use crate::units::unit_types::UnitType;

use super::movement::MovementType;

// helper functions
impl<D: Direction> Unit<D> {
    pub fn get_hp(&self) -> u8 {
        match self.get_tag(TAG_HP) {
            Some(TagValue::Int(value)) => value.0 as u8,
            _ => 100
        }
    }
}

impl<D: Direction> UnitBuilder<D> {
    pub fn set_hp(self, hp: u8) -> Self {
        self.set_tag(TAG_HP, TagValue::Int(Int32(hp as i32)))
    }
    pub fn set_hero_origin(self, p: Point) -> Self {
        self.set_tag(TAG_HERO_ORIGIN, TagValue::Point(p))
    }
}

impl MovementType {
    pub const FOOT: MovementType = MovementType(0);
    pub const AMPHIBIOUS: MovementType = MovementType(12);
    pub const HOVER: MovementType = MovementType(4);
}

// actual tests

#[test]
fn unit_builder_transported() {
    let config = Arc::new(Config::test_config());
    let map = WMBuilder::<Direction4>::new(PointMap::new(5, 5, false));
    let map = Map::new(map.build(), &config);
    let map_env = map.environment().clone();
    let unit: Unit<Direction4> = UnitType::transport_heli().instance(&map_env).set_owner_id(0).set_transported(vec![
        UnitType::marine().instance(&map_env).set_hp(34).build(),
    ]).build();
    assert_eq!(unit.get_transported().len(), 1);
}

#[test]
fn fog_replacement() {
    let config = Arc::new(Config::test_config());
    let map = PointMap::new(5, 5, false);
    let map = WMBuilder::<Direction4>::new(map);
    let mut map = Map::new(map.build(), &config);
    let map_env = map.environment().clone();
    map.set_terrain(Point::new(1, 1), TerrainType::City.instance(&map_env).build());
    let unit = UnitType::sniper().instance(&map_env).set_owner_id(0).set_flag(FLAG_CAPTURING).build();
    let map_view = Handle::new(map);
    assert_eq!(
        unit.fog_replacement(&map_view, Point::new(1, 1), FogIntensity::Light),
        Some(map_env.config.unknown_unit().instance(&map_env).build())
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
            map.set_terrain(Point::new(x, y), TerrainType::Sea.instance(&map_env).build());
        }
    }
    map.set_unit(Point::new(3, 4), Some(UnitType::drone_boat().instance(&map_env).set_owner_id(0).set_hp(100).build()));
    map.set_unit(Point::new(1, 6), Some(UnitType::war_ship().instance(&map_env).set_owner_id(1).build()));
    let mut settings = map.settings().unwrap();
    settings.fog_mode = FogMode::Constant(FogSetting::None);
    settings.players[0].set_funds(1000);
    let (mut server, events) = Game::new_server(map.clone(), settings.build_default(), Arc::new(|| 0.));
    let mut client = Game::new_client(map, settings.build_default(), events.get(&Perspective::Team(0)).unwrap());
    assert!(server.get_unit(Point::new(3, 4)).unwrap().get_tag(TAG_DRONE_STATION_ID).is_none());
    let events = server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(Point::new(3, 4)),
        action: UnitAction::custom(CA_UNIT_BUILD_UNIT, vec![CustomActionInput::ShopItem(0.into())]),
    }), Arc::new(|| 0.)).unwrap();
    client.with_mut(|client| {
        for ev in events.get(&Perspective::Team(0)).unwrap() {
            ev.apply(client);
        }
    });
    assert!(server.get_unit(Point::new(3, 4)).unwrap().get_tag(TAG_DRONE_STATION_ID).is_some());
    assert!(client.get_unit(Point::new(3, 4)).unwrap().get_tag(TAG_DRONE_STATION_ID).is_some());
    assert_eq!(
        server.get_unit(Point::new(3, 4)).unwrap().get_transported().len(),
        1
    );
    assert_eq!(
        client.get_unit(Point::new(3, 4)).unwrap().get_transported()[0],
        UnitType::light_drone().instance(&server.environment())
        .set_owner_id(0)
        .set_hp(100)
        .set_tag(TAG_DRONE_ID, client.get_unit(Point::new(3, 4)).unwrap().get_tag(TAG_DRONE_STATION_ID).unwrap())
        .set_flag(FLAG_EXHAUSTED)
        .build()
    );
    assert_eq!(
        server.get_unit(Point::new(3, 4)).unwrap().get_transported()[0].get_tag(TAG_DRONE_ID),
        server.get_unit(Point::new(3, 4)).unwrap().get_tag(TAG_DRONE_STATION_ID)
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
            map.set_terrain(Point::new(x, y), TerrainType::Grass.instance(&map_env).build());
        }
    }
    map.set_terrain(Point::new(3, 4), TerrainType::Factory.instance(&map_env).set_owner_id(0).build());
    map.set_unit(Point::new(3, 4), Some(UnitType::small_tank().instance(&map_env).set_owner_id(0).set_hp(1).build()));
    map.set_unit(Point::new(1, 6), Some(UnitType::war_ship().instance(&map_env).set_owner_id(1).build()));
    let mut settings = map.settings().unwrap();
    settings.fog_mode = FogMode::Constant(FogSetting::None);
    settings.players[0].set_funds(1000);
    let (mut server, _) = Game::new_server(map.clone(), settings.build_default(), Arc::new(|| 0.));
    assert_eq!(server.get_unit(Point::new(3, 4)).unwrap().get_hp(), 1);
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(Point::new(3, 4)),
        action: UnitAction::custom(CA_UNIT_REPAIR, Vec::new()),
    }), Arc::new(|| 0.)).unwrap();
    assert!(*server.get_owning_player(0).unwrap().funds < 1000);
    assert!(server.get_unit(Point::new(3, 4)).unwrap().get_hp() > 1);
    assert!(server.get_unit(Point::new(3, 4)).unwrap().has_flag(FLAG_REPAIRING));
    assert!(server.get_unit(Point::new(3, 4)).unwrap().has_flag(FLAG_EXHAUSTED));
    server.handle_command(Command::EndTurn, Arc::new(|| 0.)).unwrap();
    server.handle_command(Command::EndTurn, Arc::new(|| 0.)).unwrap();
    assert!(!server.get_unit(Point::new(3, 4)).unwrap().has_flag(FLAG_REPAIRING));
    assert!(!server.get_unit(Point::new(3, 4)).unwrap().has_flag(FLAG_EXHAUSTED));
}


#[test]
fn end_game() {
    let config = Arc::new(Config::test_config());
    let map = PointMap::new(4, 4, false);
    let wmap: WrappingMap<Direction4> = WMBuilder::new(map).build();
    let mut map = Map::new(wmap, &config);
    let map_env = map.environment().clone();
    map.set_unit(Point::new(0, 0), Some(UnitType::small_tank().instance(&map_env).set_owner_id(0).set_hp(100).build()));
    map.set_unit(Point::new(0, 1), Some(UnitType::small_tank().instance(&map_env).set_owner_id(1).set_hp(1).build()));
    let settings = map.settings().unwrap();
    let (mut game, _) = Game::new_server(map, settings.build_default(), Arc::new(|| 0.));
    game.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(Point::new(0, 0)),
        action: UnitAction::Attack(AttackVector::Direction(Direction4::D270)),
    }), Arc::new(|| 0.)).unwrap();
    game.with(|game| {
        assert_eq!(game.get_map().get_unit(Point::new(0, 1)), None);
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
    map.set_unit(Point::new(0, 0), Some(UnitType::small_tank().instance(&map_env).set_owner_id(0).set_hp(1).build()));
    map.set_unit(Point::new(0, 1), Some(UnitType::small_tank().instance(&map_env).set_owner_id(1).build()));
    map.set_unit(Point::new(0, 2), Some(UnitType::small_tank().instance(&map_env).set_owner_id(2).build()));
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
    map.set_unit(Point::new(0, 0), Some(UnitType::small_tank().instance(&map_env).set_owner_id(0).build()));
    map.set_unit(Point::new(0, 1), Some(UnitType::life_crystal().instance(&map_env).set_owner_id(1).set_hp(1).build()));
    map.set_unit(Point::new(0, 2), Some(UnitType::small_tank().instance(&map_env).set_owner_id(1).build()));
    map.set_unit(Point::new(0, 3), Some(UnitType::small_tank().instance(&map_env).set_owner_id(1).build()));
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
    let sea = TerrainType::ShallowSea.instance(&map_env).build();
    // experiment
    map.set_terrain(Point::new(1, 1), sea.clone());
    map.set_terrain(Point::new(2, 1), sea.clone());
    map.set_unit(Point::new(0, 1), Some(UnitType::small_tank().instance(&map_env).set_owner_id(0).set_hp(100).build()));
    map.set_unit(Point::new(0, 2), Some(UnitType::artillery().instance(&map_env).set_owner_id(0).set_hp(100).build()));
    map.set_unit(Point::new(1, 1), Some(UnitType::puffer_fish().instance(&map_env).build()));
    map.set_unit(Point::new(2, 0), Some(UnitType::small_tank().instance(&map_env).set_owner_id(1).set_hp(100).build()));
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
    map.set_unit(Point::new(0, 0), Some(UnitType::small_tank().instance(&map_env).set_owner_id(0).build()));
    map.set_unit(Point::new(0, 1), Some(UnitType::pyramid().instance(&map_env).set_owner_id(1).set_hp(1).build()));
    map.set_unit(Point::new(0, 2), Some(UnitType::small_tank().instance(&map_env).set_owner_id(0).build()));
    map.set_unit(Point::new(0, 3), Some(UnitType::small_tank().instance(&map_env).set_owner_id(1).build()));
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
    map.set_unit(Point::new(1, 1), Some(UnitType::factory().instance(&map_env).set_owner_id(0).set_hp(1).build()));
    map.set_unit(Point::new(1, 3), Some(UnitType::pyramid().instance(&map_env).set_owner_id(0).set_hp(1).build()));
    map.set_unit(Point::new(0, 3), Some(UnitType::small_tank().instance(&map_env).set_owner_id(1).build()));
    let settings = map.settings().unwrap();
    let (mut game, _) = Game::new_server(map, settings.build_default(), Arc::new(|| 0.));
    game.with(|game| {
        assert_eq!(*game.current_player().funds, game.current_player().get_income() * 2);
    });
    assert!(!game.get_unit(Point::new(1, 1)).unwrap().has_flag(FLAG_EXHAUSTED));
    let to_build = UnitType::marine().instance(&game.environment())
        .set_owner_id(0)
        .set_hp(100);
    game.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(Point::new(1, 1)),
        action: UnitAction::custom(CA_UNIT_BUILD_UNIT, vec![CustomActionInput::ShopItem(0.into()), CustomActionInput::Direction(Direction6::D180)]),
    }), Arc::new(|| 0.)).unwrap();
    assert_eq!(game.get_unit(Point::new(0, 1)).unwrap(), to_build.set_flag(FLAG_EXHAUSTED).build());
    assert!(game.get_unit(Point::new(1, 1)).unwrap().has_flag(FLAG_EXHAUSTED));
}

#[test]
fn marine_movement_types() {
    let config = Arc::new(Config::test_config());
    let map = PointMap::new(4, 4, false);
    let wmap: WrappingMap<Direction4> = WMBuilder::new(map).build();
    let mut map = Map::new(wmap, &config);
    let map_env = map.environment().clone();
    let mut bubble = Token::new(map_env.clone(), TokenType::BUBBLE_FACTORY);
    bubble.set_owner_id(0);
    map.set_tokens(Point::new(0, 0), vec![bubble]);
    let mut bubble = Token::new(map_env.clone(), TokenType::BUBBLE_PORT);
    bubble.set_owner_id(0);
    map.set_tokens(Point::new(1, 0), vec![bubble]);
    map.set_unit(Point::new(3, 3), Some(UnitType::sniper().instance(&map_env).set_owner_id(1).build()));
    let mut settings = map.settings().unwrap();
    settings.players[0].set_funds(1000);
    let (mut game, _) = Game::new_server(map, settings.build_default(), Arc::new(|| 0.));
    let environment = game.environment();
    game.handle_command(Command::TokenAction(Point::new(0, 0), vec![
        CustomActionInput::ShopItem(0.into()),
    ].try_into().unwrap()), Arc::new(|| 0.)).unwrap();
    assert_eq!(game.get_unit(Point::new(0, 0)), Some(UnitType::marine().instance(&environment).set_owner_id(0).set_hp(100).set_movement_type(MovementType::FOOT).build()));
    game.handle_command(Command::TokenAction(Point::new(1, 0), vec![
        CustomActionInput::ShopItem(0.into()),
    ].try_into().unwrap()), Arc::new(|| 0.)).unwrap();
    assert_eq!(game.get_unit(Point::new(1, 0)), Some(UnitType::marine().instance(&environment).set_owner_id(0).set_hp(100).set_movement_type(MovementType::AMPHIBIOUS).build()));
}

#[test]
fn enter_transporter() {
    let config = Arc::new(Config::test_config());
    let map = PointMap::new(4, 4, false);
    let wmap: WrappingMap<Direction4> = WMBuilder::new(map).build();
    let mut map = Map::new(wmap, &config);
    let map_env = map.environment().clone();
    // map setup
    map.set_unit(Point::new(0, 0), Some(UnitType::sniper().instance(&map_env).set_owner_id(0).set_hp(100).build()));
    map.set_unit(Point::new(2, 0), Some(UnitType::transport_heli().instance(&map_env).set_owner_id(0).set_hp(100).build()));
    map.set_unit(Point::new(3, 3), Some(UnitType::sniper().instance(&map_env).set_owner_id(1).set_hp(100).build()));
    // create game
    let settings = map.settings().unwrap();
    let (mut game, _) = Game::new_server(map, settings.build_default(), Arc::new(|| 0.));
    // test
    let transporter = game.get_unit(Point::new(2, 0)).unwrap();
    assert_eq!(transporter.get_transported().len(), 0);
    let path = Path::with_steps(Point::new(0, 0), vec![PathStep::Dir(Direction4::D0), PathStep::Dir(Direction4::D0)]);
    assert!(game.get_unit(Point::new(0, 0)).unwrap().options_after_path(&*game, &path, None, &[]).contains(&UnitAction::Enter));
    game.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path,
        action: UnitAction::Enter,
    }), Arc::new(|| 0.)).unwrap();
    let transporter = game.get_unit(Point::new(2, 0)).unwrap();
    assert_eq!(transporter.typ(), UnitType::transport_heli());
    assert_eq!(transporter.get_transported().len(), 1);
    assert_eq!(transporter.get_transported()[0].typ(), UnitType::sniper());
    assert_eq!(game.get_unit(Point::new(0, 0)), None);
}

#[test]
fn chess_movement_exhausts_all() {
    let map = chess_board();
    let settings = map.settings().unwrap();
    let mut server = Game::new_server(map.clone(), settings.build_default(), Arc::new(|| 0.)).0;
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::with_steps(Point::new(0, 6), vec![PathStep::Dir(Direction4::D90), PathStep::Dir(Direction4::D90)]),
        action: UnitAction::Wait,
    }), Arc::new(|| 0.)).unwrap();
    assert!(server.get_unit(Point::new(0, 4)).unwrap().has_flag(FLAG_EXHAUSTED));
    for x in 0..8 {
        println!("x = {x}");
        assert!(!server.get_unit(Point::new(x, 0)).unwrap().has_flag(FLAG_EXHAUSTED));
        assert!(!server.get_unit(Point::new(x, 1)).unwrap().has_flag(FLAG_EXHAUSTED));
        if x > 0 {
            assert!(server.get_unit(Point::new(x, 6)).unwrap().has_flag(FLAG_EXHAUSTED));
        }
        assert!(server.get_unit(Point::new(x, 7)).unwrap().has_flag(FLAG_EXHAUSTED));
    }
    assert_eq!(server.get_unit(Point::new(0, 6)), None);
}

#[test]
fn chess_castling() {
    let config = Arc::new(Config::test_config());
    let map = PointMap::new(5, 5, false);
    let map = WMBuilder::<Direction4>::new(map);
    let mut map = Map::new(map.build(), &config);
    let map_env = map.environment().clone();

    for p in map.all_points() {
        map.set_terrain(p, TerrainType::ChessTile.instance(&map_env).build());
    }
    map.set_unit(Point::new(0, 0), Some(UnitType::king().instance(&map_env).set_owner_id(0).set_flag(FLAG_UNMOVED).build()));
    map.set_unit(Point::new(4, 0), Some(UnitType::rook().instance(&map_env).set_owner_id(0).set_flag(FLAG_UNMOVED).build()));
    map.set_unit(Point::new(4, 4), Some(UnitType::rook().instance(&map_env).set_owner_id(0).set_flag(FLAG_UNMOVED).build()));
    map.set_unit(Point::new(2, 2), Some(UnitType::small_tank().instance(&map_env).set_owner_id(1).set_hp(1).build()));

    let settings = map.settings().unwrap();
    let (mut server, _) = Game::new_server(map.clone(), settings.build_default(), Arc::new(|| 0.));
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::with_steps(Point::new(4, 4), vec![PathStep::Dir(Direction4::D180), PathStep::Dir(Direction4::D180), PathStep::Dir(Direction4::D180), PathStep::Dir(Direction4::D180)]),
        action: UnitAction::Wait,
    }), Arc::new(|| 0.)).unwrap();
    assert!(server.get_unit(Point::new(0, 0)).unwrap().has_flag(FLAG_UNMOVED));
    assert!(server.get_unit(Point::new(4, 0)).unwrap().has_flag(FLAG_UNMOVED));
    assert!(!server.get_unit(Point::new(0, 4)).unwrap().has_flag(FLAG_UNMOVED));
    server.handle_command(Command::EndTurn, Arc::new(|| 0.)).unwrap();
    server.handle_command(Command::EndTurn, Arc::new(|| 0.)).unwrap();
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::with_steps(Point::new(0, 4), vec![PathStep::Dir(Direction4::D90), PathStep::Dir(Direction4::D90), PathStep::Dir(Direction4::D90)]),
        action: UnitAction::Attack(AttackVector::Direction(Direction4::D90)),
    }), Arc::new(|| 0.)).unwrap_err();
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::with_steps(Point::new(4, 0), vec![PathStep::Dir(Direction4::D180), PathStep::Dir(Direction4::D180), PathStep::Dir(Direction4::D180)]),
        action: UnitAction::Attack(AttackVector::Direction(Direction4::D180)),
    }), Arc::new(|| 0.)).unwrap();
    assert!(!server.get_unit(Point::new(1, 0)).unwrap().has_flag(FLAG_UNMOVED));
    assert!(!server.get_unit(Point::new(2, 0)).unwrap().has_flag(FLAG_UNMOVED));
    assert!(!server.get_unit(Point::new(0, 4)).unwrap().has_flag(FLAG_UNMOVED));
}

#[test]
fn chess_en_passant() {
    let config = Arc::new(Config::test_config());
    let map = PointMap::new(5, 5, false);
    let map = WMBuilder::<Direction4>::new(map);
    let mut map = Map::new(map.build(), &config);
    let map_env = map.environment().clone();

    for p in map.all_points() {
        map.set_terrain(p, TerrainType::ChessPawnTile.instance(&map_env).build());
    }
    map.set_unit(Point::new(0, 0), Some(UnitType::pawn().instance(&map_env).set_owner_id(0).set_tag(TAG_PAWN_DIRECTION, TagValue::Direction(Direction4::D270)).set_hp(100).build()));
    map.set_unit(Point::new(1, 2), Some(UnitType::pawn().instance(&map_env).set_owner_id(1).set_tag(TAG_PAWN_DIRECTION, TagValue::Direction(Direction4::D90)).set_hp(100).build()));
    map.set_unit(Point::new(4, 3), Some(UnitType::rook().instance(&map_env).set_owner_id(0).set_hp(100).build()));
    map.set_unit(Point::new(4, 4), Some(UnitType::rook().instance(&map_env).set_owner_id(1).set_hp(100).build()));

    let mut settings = map.settings().unwrap();
    settings.fog_mode = FogMode::Constant(FogSetting::None);
    let (mut server, _) = Game::new_server(map.clone(), settings.build_default(), Arc::new(|| 0.));
    let unchanged = server.clone();
    // take pawn normally
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::with_steps(Point::new(0, 0), vec![PathStep::Dir(Direction4::D270)]),
        action: UnitAction::Wait,
    }), Arc::new(|| 0.)).unwrap();
    assert!(server.get_unit(Point::new(0, 1)).unwrap().get_tag(TAG_EN_PASSANT).is_none());
    server.handle_command(Command::EndTurn, Arc::new(|| 0.)).unwrap();
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::with_steps(Point::new(1, 2), vec![PathStep::Diagonal(Direction4::D90)]),
        action: UnitAction::Take,
    }), Arc::new(|| 0.)).unwrap();
    // unable to take pawn that wasn't moved (out of range)
    let mut server = unchanged.clone();
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::with_steps(Point::new(4, 3), vec![PathStep::Dir(Direction4::D180)]),
        action: UnitAction::Wait,
    }), Arc::new(|| 0.)).unwrap();
    server.handle_command(Command::EndTurn, Arc::new(|| 0.)).unwrap();
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::with_steps(Point::new(1, 2), vec![PathStep::Diagonal(Direction4::D90)]),
        action: UnitAction::Take,
    }), Arc::new(|| 0.)).unwrap_err();
    // en passant
    let mut server = unchanged.clone();
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::with_steps(Point::new(0, 0), vec![PathStep::Dir(Direction4::D270), PathStep::Dir(Direction4::D270)]),
        action: UnitAction::Wait,
    }), Arc::new(|| 0.)).unwrap();
    server.handle_command(Command::EndTurn, Arc::new(|| 0.)).unwrap();
    assert_eq!(server.get_unit(Point::new(0, 2)).unwrap().get_tag(TAG_EN_PASSANT), Some(TagValue::Point(Point::new(0, 1))));
    assert_eq!(server.get_unit(Point::new(0, 2)).unwrap().get_en_passant(), Some(Point::new(0, 1)));
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::with_steps(Point::new(1, 2), vec![PathStep::Diagonal(Direction4::D90)]),
        action: UnitAction::Wait,
    }), Arc::new(|| 0.)).unwrap();
    // pawn moved twice, no en passant possible
    let mut server = unchanged.clone();
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::with_steps(Point::new(0, 0), vec![PathStep::Dir(Direction4::D270)]),
        action: UnitAction::Wait,
    }), Arc::new(|| 0.)).unwrap();
    server.handle_command(Command::EndTurn, Arc::new(|| 0.)).unwrap();
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::with_steps(Point::new(4, 4), vec![PathStep::Dir(Direction4::D180)]),
        action: UnitAction::Wait,
    }), Arc::new(|| 0.)).unwrap();
    server.handle_command(Command::EndTurn, Arc::new(|| 0.)).unwrap();
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::with_steps(Point::new(0, 1), vec![PathStep::Dir(Direction4::D270)]),
        action: UnitAction::Wait,
    }), Arc::new(|| 0.)).unwrap();
    server.handle_command(Command::EndTurn, Arc::new(|| 0.)).unwrap();
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::with_steps(Point::new(1, 2), vec![PathStep::Diagonal(Direction4::D90)]),
        action: UnitAction::Take,
    }), Arc::new(|| 0.)).unwrap_err();
    // en passant not possible when tried one turn later
    let mut server = unchanged.clone();
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::with_steps(Point::new(0, 0), vec![PathStep::Dir(Direction4::D270), PathStep::Dir(Direction4::D270)]),
        action: UnitAction::Wait,
    }), Arc::new(|| 0.)).unwrap();
    server.handle_command(Command::EndTurn, Arc::new(|| 0.)).unwrap();
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::with_steps(Point::new(4, 4), vec![PathStep::Dir(Direction4::D180)]),
        action: UnitAction::Wait,
    }), Arc::new(|| 0.)).unwrap();
    assert_eq!(server.get_unit(Point::new(0, 2)).unwrap().get_tag(TAG_EN_PASSANT), Some(TagValue::Point(Point::new(0, 1))));
    server.handle_command(Command::EndTurn, Arc::new(|| 0.)).unwrap();
    assert!(server.get_unit(Point::new(0, 2)).unwrap().get_tag(TAG_EN_PASSANT).is_none());
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::with_steps(Point::new(4, 3), vec![PathStep::Dir(Direction4::D180)]),
        action: UnitAction::Wait,
    }), Arc::new(|| 0.)).unwrap();
    server.handle_command(Command::EndTurn, Arc::new(|| 0.)).unwrap();
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::with_steps(Point::new(1, 2), vec![PathStep::Diagonal(Direction4::D90)]),
        action: UnitAction::Take,
    }), Arc::new(|| 0.)).unwrap_err();
}
