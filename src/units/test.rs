use interfaces::{Perspective, GameEventsMap};
use uniform_smart_pointer::Urc;

use crate::combat::AttackInput;
use crate::config::config::Config;
use crate::game::commands::Command;
use crate::game::event_fx::{Effect, EffectWithoutPosition};
use crate::game::events::Event;
use crate::game::fog::*;
use crate::game::game::Game;
use crate::map::board::{Board, BoardView};
use crate::map::direction::*;
use crate::map::map::Map;
use crate::map::pipe::PipeState;
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
}

impl MovementType {
    pub const FOOT: MovementType = MovementType(0);
    pub const AMPHIBIOUS: MovementType = MovementType(12);
    pub const HOVER: MovementType = MovementType(4);
}

// actual tests

#[test]
fn unit_builder_transported() {
    let config = Urc::new(Config::default());
    let map = WMBuilder::<Direction4>::new(PointMap::new(5, 5, false));
    let map = Map::new(map.build(), &config);
    let map_env = map.environment().clone();
    let unit: Unit<Direction4> = UnitType::TRANSPORT_HELI.instance(&map_env).set_owner_id(0).set_transported(vec![
        UnitType::MARINE.instance(&map_env).set_hp(34).build(),
    ]).build();
    assert_eq!(unit.get_transported().len(), 1);
}

#[test]
fn fog_replacement() {
    let config = Urc::new(Config::default());
    let map = PointMap::new(5, 5, false);
    let map = WMBuilder::<Direction4>::new(map);
    let mut map = Map::new(map.build(), &config);
    let map_env = map.environment().clone();
    map.set_terrain(Point::new(1, 1), TerrainType::City.instance(&map_env).build());
    let unit = UnitType::SNIPER.instance(&map_env).set_owner_id(0).set_flag(FLAG_CAPTURING).build();
    let map_view = Board::new(&map);
    assert_eq!(
        unit.fog_replacement(&map_view, Point::new(1, 1), FogIntensity::Light),
        Some(map_env.config.unknown_unit().instance(&map_env).build())
    );
}

#[test]
fn transported_visibility() {
    let config = Urc::new(Config::default());
    let map = PointMap::new(8, 8, false);
    let map = WMBuilder::<Direction4>::new(map);
    let map = Map::new(map.build(), &config);
    let environment = map.environment().clone();
    let origin = Point::new(0, 0);
    let board = Board::from(&map);
    let transporter = UnitType::TRANSPORT_HELI.instance::<Direction4>(&environment).set_owner_id(0).set_transported(vec![
        UnitType::BAZOOKA.instance(&environment).set_owner_id(0).build(),
    ]).build();
    assert_eq!(1, transporter.get_transported().len());
    assert_eq!(1, transporter.fog_replacement(&board, origin, FogIntensity::TrueSight).unwrap().get_transported().len());
    assert_eq!(1, transporter.fog_replacement(&board, origin, FogIntensity::NormalVision).unwrap().get_transported().len());
    assert_eq!(0, transporter.fog_replacement(&board, origin, FogIntensity::Light).unwrap().get_transported().len());
    assert_eq!(None, transporter.fog_replacement(&board, origin, FogIntensity::Dark));
}

#[test]
fn drone() {
    let config = Urc::new(Config::default());
    let map = PointMap::new(5, 7, false);
    let map = WMBuilder::<Direction6>::new(map);
    let mut map = Map::new(map.build(), &config);
    let map_env = map.environment().clone();
    for x in 0..5 {
        for y in 0..7 {
            map.set_terrain(Point::new(x, y), TerrainType::Sea.instance(&map_env).build());
        }
    }
    map.set_unit(Point::new(3, 4), Some(UnitType::DRONE_BOAT.instance(&map_env).set_owner_id(0).set_hp(100).build()));
    map.set_unit(Point::new(1, 6), Some(UnitType::WAR_SHIP.instance(&map_env).set_owner_id(1).build()));
    let mut settings = map.settings().unwrap();
    settings.fog_mode = FogMode::Constant(FogSetting::None);
    settings.players[0].get_tag_bag_mut().set_tag(&map_env, TAG_FUNDS, 1000.into());
    let (mut server, events) = Game::new_server(map.clone(), &settings, settings.build_default(), Urc::new(|| 0.));
    let mut client = Game::new_client(map, &settings, settings.build_default(), events.get(&Perspective::Team(0)).unwrap());
    assert!(server.get_unit(Point::new(3, 4)).unwrap().get_tag(TAG_DRONE_STATION_ID).is_none());
    let events = server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(Point::new(3, 4)),
        action: UnitAction::custom(CA_UNIT_BUILD_UNIT, vec![CustomActionInput::ShopItem(0.into())]),
    }), Urc::new(|| 0.)).unwrap();
    for ev in events.get(&Perspective::Team(0)).unwrap() {
        ev.apply(&mut client);
    }
    assert!(server.get_unit(Point::new(3, 4)).unwrap().get_tag(TAG_DRONE_STATION_ID).is_some());
    assert!(client.get_unit(Point::new(3, 4)).unwrap().get_tag(TAG_DRONE_STATION_ID).is_some());
    assert_eq!(
        server.get_unit(Point::new(3, 4)).unwrap().get_transported().len(),
        1
    );
    assert_eq!(
        client.get_unit(Point::new(3, 4)).unwrap().get_transported()[0],
        UnitType::LIGHT_DRONE.instance(&server.environment())
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
    server.handle_command(Command::EndTurn, Urc::new(|| 0.)).unwrap();
    server.handle_command(Command::EndTurn, Urc::new(|| 0.)).unwrap();
    // drone boat is unexhausted next turn
    assert!(!server.get_unit(Point::new(3, 4)).unwrap().has_flag(FLAG_EXHAUSTED));
    // built drone is unexhausted next turn
    assert!(!server.get_unit(Point::new(3, 4)).unwrap().get_transported()[0].has_flag(FLAG_EXHAUSTED));
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: Some(0.into()),
        path: Path::with_steps(Point::new(3, 4), vec![PathStep::Dir(Direction6::D0)]),
        action: UnitAction::Wait,
    }), Urc::new(|| 0.)).unwrap();
    // can't build another drone
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(Point::new(3, 4)),
        action: UnitAction::custom(CA_UNIT_BUILD_UNIT, vec![CustomActionInput::ShopItem(0.into())]),
    }), Urc::new(|| 0.)).unwrap_err();
    server.handle_command(Command::EndTurn, Urc::new(|| 0.)).unwrap();
    let drone = server.get_unit(Point::new(4, 4)).unwrap().clone();
    server.get_map_mut().set_unit(Point::new(0, 0), Some(drone));
    assert_eq!(server.get_unit(Point::new(3, 4)).unwrap().get_transported().len(), 0);
    let events = server.handle_command(Command::EndTurn, Urc::new(|| 0.)).unwrap();
    // one drone has returned to the boat, the other died
    assert_eq!(server.get_unit(Point::new(3, 4)).unwrap().get_transported().len(), 1);
    assert!(server.get_unit(Point::new(0, 0)).is_none());
    assert!(server.get_unit(Point::new(4, 4)).is_none());
    let death_effect = config.find_effect_by_name("Explosion").unwrap();
    assert!(events.get(&Perspective::Server).unwrap().iter()
        .any(|e| matches!(e, Event::Effect(Effect::Point(EffectWithoutPosition { typ, .. }, _)) if *typ == death_effect)), 
        "{:?}", events.get(&Perspective::Server));
}

#[test]
fn cannot_buy_unit_without_money() {
    let config = Urc::new(Config::default());
    let map = PointMap::new(4, 4, false);
    let wmap: WrappingMap<Direction6> = WMBuilder::new(map).build();
    let mut map = Map::new(wmap, &config);
    let map_env = map.environment().clone();
    map.set_unit(Point::new(0, 0), Some(UnitType::DRONE_BOAT.instance(&map_env).set_owner_id(0).set_hp(100).build()));
    map.set_unit(Point::new(1, 1), Some(UnitType::WAR_SHIP.instance(&map_env).set_owner_id(1).build()));
    let settings = map.settings().unwrap();
    let (mut game, _) = Game::new_server(map, &settings, settings.build_default(), Urc::new(|| 0.));
    assert_eq!(game.current_player().get_tag(TAG_FUNDS).unwrap().into_dynamic().cast::<i32>(), 0);
    let path = Path::new(Point::new(0, 0));
    game.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path,
        action: UnitAction::custom(CA_UNIT_BUILD_UNIT, vec![CustomActionInput::ShopItem(0.into())]),
    }), Urc::new(|| 0.)).unwrap_err();
}

#[test]
fn repair_unit() {
    let config = Urc::new(Config::default());
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
    map.set_unit(Point::new(3, 4), Some(UnitType::SMALL_TANK.instance(&map_env).set_owner_id(0).set_hp(1).build()));
    map.set_unit(Point::new(1, 6), Some(UnitType::WAR_SHIP.instance(&map_env).set_owner_id(1).build()));
    let mut settings = map.settings().unwrap();
    settings.fog_mode = FogMode::Constant(FogSetting::None);
    settings.players[0].get_tag_bag_mut().set_tag(&map_env, TAG_FUNDS, 1000.into());
    let (mut server, _) = Game::new_server(map.clone(), &settings, settings.build_default(), Urc::new(|| 0.));
    assert_eq!(server.get_unit(Point::new(3, 4)).unwrap().get_hp(), 1);
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(Point::new(3, 4)),
        action: UnitAction::custom(CA_UNIT_REPAIR, Vec::new()),
    }), Urc::new(|| 0.)).unwrap();
    assert!(server.get_owning_player(0).unwrap().get_tag(TAG_FUNDS).unwrap().into_dynamic().cast::<i32>() < 1000);
    assert!(server.get_unit(Point::new(3, 4)).unwrap().get_hp() > 1);
    assert!(server.get_unit(Point::new(3, 4)).unwrap().has_flag(FLAG_REPAIRING));
    assert!(server.get_unit(Point::new(3, 4)).unwrap().has_flag(FLAG_EXHAUSTED));
    server.handle_command(Command::EndTurn, Urc::new(|| 0.)).unwrap();
    server.handle_command(Command::EndTurn, Urc::new(|| 0.)).unwrap();
    assert!(!server.get_unit(Point::new(3, 4)).unwrap().has_flag(FLAG_REPAIRING));
    assert!(!server.get_unit(Point::new(3, 4)).unwrap().has_flag(FLAG_EXHAUSTED));
}


#[test]
fn end_game() {
    let config = Urc::new(Config::default());
    let map = PointMap::new(4, 4, false);
    let wmap: WrappingMap<Direction4> = WMBuilder::new(map).build();
    let mut map = Map::new(wmap, &config);
    let map_env = map.environment().clone();
    map.set_unit(Point::new(0, 0), Some(UnitType::SMALL_TANK.instance(&map_env).set_owner_id(0).set_hp(100).build()));
    map.set_unit(Point::new(0, 1), Some(UnitType::SMALL_TANK.instance(&map_env).set_owner_id(1).set_hp(1).build()));
    let settings = map.settings().unwrap();
    let (mut game, _) = Game::new_server(map, &settings, settings.build_default(), Urc::new(|| 0.));
    game.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(Point::new(0, 0)),
        action: UnitAction::Attack(AttackInput::SplashPattern(OrientedPoint::simple(Point::new(0, 1), Direction4::D270))),
    }), Urc::new(|| 0.)).unwrap();
    assert_eq!(game.get_map().get_unit(Point::new(0, 1)), None);
    assert!(game.has_ended());
    for (i, player) in game.players.iter().enumerate() {
        assert_eq!(player.dead, i != 0);
    }
}

#[test]
fn defeat_player_of_3() {
    let config = Urc::new(Config::default());
    let map = PointMap::new(4, 4, false);
    let wmap: WrappingMap<Direction4> = WMBuilder::new(map).build();
    let mut map = Map::new(wmap, &config);
    let map_env = map.environment().clone();
    map.set_unit(Point::new(0, 0), Some(UnitType::SMALL_TANK.instance(&map_env).set_owner_id(0).set_hp(1).build()));
    map.set_unit(Point::new(0, 1), Some(UnitType::SMALL_TANK.instance(&map_env).set_owner_id(1).build()));
    map.set_unit(Point::new(0, 2), Some(UnitType::SMALL_TANK.instance(&map_env).set_owner_id(2).build()));
    let settings = map.settings().unwrap();
    let (mut game, _) = Game::new_server(map, &settings, settings.build_default(), Urc::new(|| 0.));
    game.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(Point::new(0, 0)),
        action: UnitAction::Attack(AttackInput::SplashPattern(OrientedPoint::simple(Point::new(0, 1), Direction4::D270))),
    }), Urc::new(|| 0.)).unwrap();
    assert!(!game.has_ended());
    for (i, player) in game.players.iter().enumerate() {
        assert_eq!(player.dead, i == 0);
    }
    assert_eq!(game.current_owner(), 1);
    game.handle_command(Command::EndTurn, Urc::new(|| 0.)).unwrap();
    assert_eq!(game.current_owner(), 2);
    game.handle_command(Command::EndTurn, Urc::new(|| 0.)).unwrap();
    assert_eq!(game.current_owner(), 1);
}

#[test]
fn on_death_lose_game() {
    let config = Urc::new(Config::default());
    let map = PointMap::new(4, 4, false);
    let wmap: WrappingMap<Direction4> = WMBuilder::new(map).build();
    let mut map = Map::new(wmap, &config);
    let map_env = map.environment().clone();
    map.set_unit(Point::new(0, 0), Some(UnitType::SMALL_TANK.instance(&map_env).set_owner_id(0).build()));
    map.set_unit(Point::new(0, 1), Some(UnitType::LIFE_CRYSTAL.instance(&map_env).set_owner_id(1).set_hp(1).build()));
    map.set_unit(Point::new(0, 2), Some(UnitType::SMALL_TANK.instance(&map_env).set_owner_id(1).build()));
    map.set_unit(Point::new(0, 3), Some(UnitType::SMALL_TANK.instance(&map_env).set_owner_id(1).build()));
    let settings = map.settings().unwrap();
    let (mut game, _) = Game::new_server(map, &settings, settings.build_default(), Urc::new(|| 0.));
    game.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(Point::new(0, 0)),
        action: UnitAction::Attack(AttackInput::SplashPattern(OrientedPoint::simple(Point::new(0, 1), Direction4::D270))),
    }), Urc::new(|| 0.)).unwrap();
    assert!(game.get_unit(Point::new(0, 1)).is_none());
    assert!(game.has_ended());
    for (i, player) in game.players.iter().enumerate() {
        assert_eq!(player.dead, i != 0);
    }
}

#[test]
fn puffer_fish() {
    let config = Urc::new(Config::default());
    let map = PointMap::new(4, 4, false);
    let wmap: WrappingMap<Direction4> = WMBuilder::new(map).build();
    let mut map = Map::new(wmap, &config);
    let map_env = map.environment().clone();
    let sea = TerrainType::ShallowSea.instance(&map_env).build();
    // experiment
    map.set_terrain(Point::new(1, 1), sea.clone());
    map.set_terrain(Point::new(2, 1), sea.clone());
    map.set_unit(Point::new(0, 1), Some(UnitType::SMALL_TANK.instance(&map_env).set_owner_id(0).set_hp(100).build()));
    map.set_unit(Point::new(0, 2), Some(UnitType::ARTILLERY.instance(&map_env).set_owner_id(0).set_hp(100).build()));
    map.set_unit(Point::new(1, 1), Some(UnitType::PUFFER_FISH.instance(&map_env).build()));
    map.set_unit(Point::new(2, 0), Some(UnitType::SMALL_TANK.instance(&map_env).set_owner_id(1).set_hp(100).build()));
    let settings = map.settings().unwrap();
    let (mut game, _) = Game::new_server(map, &settings, settings.build_default(), Urc::new(|| 0.));
    game.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(Point::new(0, 1)),
        action: UnitAction::Attack(AttackInput::SplashPattern(OrientedPoint::simple(Point::new(1, 1), Direction4::D0))),
    }), Urc::new(|| 0.)).unwrap();
    assert_eq!(game.get_unit(Point::new(2, 1)).unwrap().typ(), UnitType::PUFFER_FISH);
    assert_eq!(game.get_unit(Point::new(0, 1)).unwrap().get_hp(), 100);
    assert_eq!(game.get_unit(Point::new(2, 1)).unwrap().get_hp(), 100);
    let hp = game.get_unit(Point::new(2, 0)).unwrap().get_hp();
    assert!(hp < 100);
    game.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(Point::new(0, 2)),
        action: UnitAction::Attack(AttackInput::SplashPattern(OrientedPoint::simple(Point::new(2, 1), Direction4::D90))),
    }), Urc::new(|| 0.)).unwrap();
    assert_eq!(game.get_unit(Point::new(2, 1)).unwrap().get_hp(), 100);
    assert!(game.get_unit(Point::new(2, 0)).unwrap().get_hp() < hp);
}


#[test]
fn capture_pyramid() {
    let config = Urc::new(Config::default());
    let map = PointMap::new(4, 4, false);
    let wmap: WrappingMap<Direction4> = WMBuilder::new(map).build();
    let mut map = Map::new(wmap, &config);
    let map_env = map.environment().clone();
    map.set_unit(Point::new(0, 0), Some(UnitType::SMALL_TANK.instance(&map_env).set_owner_id(0).build()));
    map.set_unit(Point::new(0, 1), Some(UnitType::PYRAMID.instance(&map_env).set_owner_id(1).set_hp(1).build()));
    map.set_unit(Point::new(0, 2), Some(UnitType::SMALL_TANK.instance(&map_env).set_owner_id(0).build()));
    map.set_unit(Point::new(0, 3), Some(UnitType::SMALL_TANK.instance(&map_env).set_owner_id(1).build()));
    let settings = map.settings().unwrap();
    let (mut game, _) = Game::new_server(map, &settings, settings.build_default(), Urc::new(|| 0.));
    game.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(Point::new(0, 0)),
        action: UnitAction::Attack(AttackInput::SplashPattern(OrientedPoint::simple(Point::new(0, 1), Direction4::D270))),
    }), Urc::new(|| 0.)).unwrap();
    assert_eq!(game.get_unit(Point::new(0, 1)).unwrap().get_owner_id(), -1);
    game.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(Point::new(0, 2)),
        action: UnitAction::Attack(AttackInput::SplashPattern(OrientedPoint::simple(Point::new(0, 1), Direction4::D90))),
    }), Urc::new(|| 0.)).unwrap();
    assert_eq!(game.get_unit(Point::new(0, 1)).unwrap().get_owner_id(), 0);
}

#[test]
fn s_factory() {
    let config = Urc::new(Config::default());
    let map = PointMap::new(4, 4, false);
    let wmap: WrappingMap<Direction6> = WMBuilder::new(map).build();
    let mut map = Map::new(wmap, &config);
    let map_env = map.environment().clone();
    map.set_unit(Point::new(1, 1), Some(UnitType::FACTORY.instance(&map_env).set_owner_id(0).set_hp(1).build()));
    map.set_unit(Point::new(1, 3), Some(UnitType::PYRAMID.instance(&map_env).set_owner_id(0).set_hp(1).build()));
    map.set_unit(Point::new(0, 3), Some(UnitType::SMALL_TANK.instance(&map_env).set_owner_id(1).build()));
    let settings = map.settings().unwrap();
    let (mut game, _) = Game::new_server(map, &settings, settings.build_default(), Urc::new(|| 0.));
    assert_eq!(
        game.current_player().get_tag(TAG_FUNDS).unwrap().into_dynamic().cast::<i32>(),
        game.current_player().get_tag(TAG_INCOME).unwrap().into_dynamic().cast::<i32>() + 100
    );
    assert!(!game.get_unit(Point::new(1, 1)).unwrap().has_flag(FLAG_EXHAUSTED));
    let to_build = UnitType::MARINE.instance(&game.environment())
        .set_owner_id(0)
        .set_hp(100);
    game.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(Point::new(1, 1)),
        action: UnitAction::custom(CA_UNIT_BUILD_UNIT, vec![CustomActionInput::ShopItem(0.into()), CustomActionInput::Direction(Direction6::D180)]),
    }), Urc::new(|| 0.)).unwrap();
    assert_eq!(*game.get_unit(Point::new(0, 1)).unwrap(), to_build.set_flag(FLAG_EXHAUSTED).build());
    assert!(game.get_unit(Point::new(1, 1)).unwrap().has_flag(FLAG_EXHAUSTED));
}

#[test]
fn marine_movement_types() {
    let config = Urc::new(Config::default());
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
    map.set_unit(Point::new(3, 3), Some(UnitType::SNIPER.instance(&map_env).set_owner_id(1).build()));
    let mut settings = map.settings().unwrap();
    settings.players[0].get_tag_bag_mut().set_tag(&map_env, TAG_FUNDS, 1000.into());
    let (mut game, _) = Game::new_server(map, &settings, settings.build_default(), Urc::new(|| 0.));
    let environment = game.environment().clone();
    game.handle_command(Command::TokenAction(Point::new(0, 0), vec![
        CustomActionInput::ShopItem(0.into()),
    ].try_into().unwrap()), Urc::new(|| 0.)).unwrap();
    assert_eq!(game.get_unit(Point::new(0, 0)), Some(&UnitType::MARINE.instance(&environment).set_owner_id(0).set_hp(100).set_movement_type(MovementType::FOOT).build()));
    game.handle_command(Command::TokenAction(Point::new(1, 0), vec![
        CustomActionInput::ShopItem(0.into()),
    ].try_into().unwrap()), Urc::new(|| 0.)).unwrap();
    assert_eq!(game.get_unit(Point::new(1, 0)), Some(&UnitType::MARINE.instance(&environment).set_owner_id(0).set_hp(100).set_movement_type(MovementType::AMPHIBIOUS).build()));
}

#[test]
fn enter_transporter() {
    let config = Urc::new(Config::default());
    let map = PointMap::new(4, 4, false);
    let wmap: WrappingMap<Direction4> = WMBuilder::new(map).build();
    let mut map = Map::new(wmap, &config);
    let map_env = map.environment().clone();
    // map setup
    map.set_unit(Point::new(0, 0), Some(UnitType::SNIPER.instance(&map_env).set_owner_id(0).set_hp(100).build()));
    map.set_unit(Point::new(2, 0), Some(UnitType::TRANSPORT_HELI.instance(&map_env).set_owner_id(0).set_hp(100).build()));
    map.set_unit(Point::new(3, 3), Some(UnitType::SNIPER.instance(&map_env).set_owner_id(1).set_hp(100).build()));
    // create game
    let settings = map.settings().unwrap();
    let (mut game, _) = Game::new_server(map, &settings, settings.build_default(), Urc::new(|| 0.));
    // test
    let transporter = game.get_unit(Point::new(2, 0)).unwrap();
    assert_eq!(transporter.get_transported().len(), 0);
    let path = Path::with_steps(Point::new(0, 0), vec![PathStep::Dir(Direction4::D0), PathStep::Dir(Direction4::D0)]);
    let board = Board::new(&game);
    assert!(game.get_unit(Point::new(0, 0)).unwrap().options_after_path(&board, &path, None, &[]).contains(&UnitAction::Enter));
    game.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path,
        action: UnitAction::Enter,
    }), Urc::new(|| 0.)).unwrap();
    let transporter = game.get_unit(Point::new(2, 0)).unwrap();
    assert_eq!(transporter.typ(), UnitType::TRANSPORT_HELI);
    assert_eq!(transporter.get_transported().len(), 1);
    assert_eq!(transporter.get_transported()[0].typ(), UnitType::SNIPER);
    assert_eq!(game.get_unit(Point::new(0, 0)), None);
}

#[test]
fn chess_movement_exhausts_all() {
    let map = chess_board();
    let settings = map.settings().unwrap();
    let mut server = Game::new_server(map.clone(), &settings, settings.build_default(), Urc::new(|| 0.)).0;
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::with_steps(Point::new(0, 6), vec![PathStep::Dir(Direction4::D90), PathStep::Dir(Direction4::D90)]),
        action: UnitAction::Wait,
    }), Urc::new(|| 0.)).unwrap();
    assert!(server.get_unit(Point::new(0, 4)).unwrap().has_flag(FLAG_EXHAUSTED));
    for x in 0..8 {
        crate::debug!("x = {x}");
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
    let config = Urc::new(Config::default());
    let map = PointMap::new(5, 5, false);
    let map = WMBuilder::<Direction4>::new(map);
    let mut map = Map::new(map.build(), &config);
    let map_env = map.environment().clone();

    for p in map.all_points() {
        map.set_terrain(p, TerrainType::ChessTile.instance(&map_env).build());
    }
    map.set_unit(Point::new(0, 0), Some(UnitType::KING.instance(&map_env).set_owner_id(0).set_flag(FLAG_UNMOVED).build()));
    map.set_unit(Point::new(4, 0), Some(UnitType::ROOK.instance(&map_env).set_owner_id(0).set_flag(FLAG_UNMOVED).build()));
    map.set_unit(Point::new(4, 4), Some(UnitType::ROOK.instance(&map_env).set_owner_id(0).set_flag(FLAG_UNMOVED).build()));
    map.set_unit(Point::new(2, 2), Some(UnitType::SMALL_TANK.instance(&map_env).set_owner_id(1).set_hp(1).build()));

    let settings = map.settings().unwrap();
    let (mut server, _) = Game::new_server(map.clone(), &settings, settings.build_default(), Urc::new(|| 0.));
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::with_steps(Point::new(4, 4), vec![PathStep::Dir(Direction4::D180), PathStep::Dir(Direction4::D180), PathStep::Dir(Direction4::D180), PathStep::Dir(Direction4::D180)]),
        action: UnitAction::Wait,
    }), Urc::new(|| 0.)).unwrap();
    assert!(server.get_unit(Point::new(0, 0)).unwrap().has_flag(FLAG_UNMOVED));
    assert!(server.get_unit(Point::new(4, 0)).unwrap().has_flag(FLAG_UNMOVED));
    assert!(!server.get_unit(Point::new(0, 4)).unwrap().has_flag(FLAG_UNMOVED));
    server.handle_command(Command::EndTurn, Urc::new(|| 0.)).unwrap();
    server.handle_command(Command::EndTurn, Urc::new(|| 0.)).unwrap();
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::with_steps(Point::new(0, 4), vec![PathStep::Dir(Direction4::D90), PathStep::Dir(Direction4::D90), PathStep::Dir(Direction4::D90)]),
        action: UnitAction::Attack(AttackInput::SplashPattern(OrientedPoint::simple(Point::new(0, 3), Direction4::D90))),
    }), Urc::new(|| 0.)).unwrap_err();
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::with_steps(Point::new(4, 0), vec![PathStep::Dir(Direction4::D180), PathStep::Dir(Direction4::D180), PathStep::Dir(Direction4::D180)]),
        action: UnitAction::Attack(AttackInput::SplashPattern(OrientedPoint::simple(Point::new(0, 0), Direction4::D180))),
    }), Urc::new(|| 0.)).unwrap();
    assert!(!server.get_unit(Point::new(1, 0)).unwrap().has_flag(FLAG_UNMOVED));
    assert!(!server.get_unit(Point::new(2, 0)).unwrap().has_flag(FLAG_UNMOVED));
    assert!(!server.get_unit(Point::new(0, 4)).unwrap().has_flag(FLAG_UNMOVED));
}

#[test]
fn chess_en_passant() {
    let config = Urc::new(Config::default());
    let map = PointMap::new(5, 5, false);
    let map = WMBuilder::<Direction4>::new(map);
    let mut map = Map::new(map.build(), &config);
    let map_env = map.environment().clone();

    for p in map.all_points() {
        map.set_terrain(p, TerrainType::ChessPawnTile.instance(&map_env).build());
    }
    map.set_unit(Point::new(0, 0), Some(UnitType::PAWN.instance(&map_env).set_owner_id(0).set_tag(TAG_PAWN_DIRECTION, TagValue::Direction(Direction4::D270)).set_hp(100).build()));
    map.set_unit(Point::new(1, 2), Some(UnitType::PAWN.instance(&map_env).set_owner_id(1).set_tag(TAG_PAWN_DIRECTION, TagValue::Direction(Direction4::D90)).set_hp(100).build()));
    map.set_unit(Point::new(4, 3), Some(UnitType::ROOK.instance(&map_env).set_owner_id(0).set_hp(100).build()));
    map.set_unit(Point::new(4, 4), Some(UnitType::ROOK.instance(&map_env).set_owner_id(1).set_hp(100).build()));

    let mut settings = map.settings().unwrap();
    settings.fog_mode = FogMode::Constant(FogSetting::None);
    let (mut server, _) = Game::new_server(map.clone(), &settings, settings.build_default(), Urc::new(|| 0.));
    let unchanged = server.clone();
    // take pawn normally
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::with_steps(Point::new(0, 0), vec![PathStep::Dir(Direction4::D270)]),
        action: UnitAction::Wait,
    }), Urc::new(|| 0.)).unwrap();
    assert!(server.get_unit(Point::new(0, 1)).unwrap().get_tag(TAG_EN_PASSANT).is_none());
    server.handle_command(Command::EndTurn, Urc::new(|| 0.)).unwrap();
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::with_steps(Point::new(1, 2), vec![PathStep::Diagonal(Direction4::D90)]),
        action: UnitAction::Take,
    }), Urc::new(|| 0.)).unwrap();
    // unable to take pawn that wasn't moved (out of range)
    let mut server = unchanged.clone();
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::with_steps(Point::new(4, 3), vec![PathStep::Dir(Direction4::D180)]),
        action: UnitAction::Wait,
    }), Urc::new(|| 0.)).unwrap();
    server.handle_command(Command::EndTurn, Urc::new(|| 0.)).unwrap();
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::with_steps(Point::new(1, 2), vec![PathStep::Diagonal(Direction4::D90)]),
        action: UnitAction::Take,
    }), Urc::new(|| 0.)).unwrap_err();
    // en passant
    let mut server = unchanged.clone();
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::with_steps(Point::new(0, 0), vec![PathStep::Dir(Direction4::D270), PathStep::Dir(Direction4::D270)]),
        action: UnitAction::Wait,
    }), Urc::new(|| 0.)).unwrap();
    server.handle_command(Command::EndTurn, Urc::new(|| 0.)).unwrap();
    assert_eq!(server.get_unit(Point::new(0, 2)).unwrap().get_tag(TAG_EN_PASSANT), Some(TagValue::Point(Point::new(0, 1))));
    assert_eq!(server.get_unit(Point::new(0, 2)).unwrap().get_en_passant(), Some(Point::new(0, 1)));
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::with_steps(Point::new(1, 2), vec![PathStep::Diagonal(Direction4::D90)]),
        action: UnitAction::Wait,
    }), Urc::new(|| 0.)).unwrap();
    // pawn moved twice, no en passant possible
    let mut server = unchanged.clone();
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::with_steps(Point::new(0, 0), vec![PathStep::Dir(Direction4::D270)]),
        action: UnitAction::Wait,
    }), Urc::new(|| 0.)).unwrap();
    server.handle_command(Command::EndTurn, Urc::new(|| 0.)).unwrap();
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::with_steps(Point::new(4, 4), vec![PathStep::Dir(Direction4::D180)]),
        action: UnitAction::Wait,
    }), Urc::new(|| 0.)).unwrap();
    server.handle_command(Command::EndTurn, Urc::new(|| 0.)).unwrap();
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::with_steps(Point::new(0, 1), vec![PathStep::Dir(Direction4::D270)]),
        action: UnitAction::Wait,
    }), Urc::new(|| 0.)).unwrap();
    server.handle_command(Command::EndTurn, Urc::new(|| 0.)).unwrap();
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::with_steps(Point::new(1, 2), vec![PathStep::Diagonal(Direction4::D90)]),
        action: UnitAction::Take,
    }), Urc::new(|| 0.)).unwrap_err();
    // en passant not possible when tried one turn later
    let mut server = unchanged.clone();
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::with_steps(Point::new(0, 0), vec![PathStep::Dir(Direction4::D270), PathStep::Dir(Direction4::D270)]),
        action: UnitAction::Wait,
    }), Urc::new(|| 0.)).unwrap();
    server.handle_command(Command::EndTurn, Urc::new(|| 0.)).unwrap();
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::with_steps(Point::new(4, 4), vec![PathStep::Dir(Direction4::D180)]),
        action: UnitAction::Wait,
    }), Urc::new(|| 0.)).unwrap();
    assert_eq!(server.get_unit(Point::new(0, 2)).unwrap().get_tag(TAG_EN_PASSANT), Some(TagValue::Point(Point::new(0, 1))));
    server.handle_command(Command::EndTurn, Urc::new(|| 0.)).unwrap();
    assert!(server.get_unit(Point::new(0, 2)).unwrap().get_tag(TAG_EN_PASSANT).is_none());
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::with_steps(Point::new(4, 3), vec![PathStep::Dir(Direction4::D180)]),
        action: UnitAction::Wait,
    }), Urc::new(|| 0.)).unwrap();
    server.handle_command(Command::EndTurn, Urc::new(|| 0.)).unwrap();
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::with_steps(Point::new(1, 2), vec![PathStep::Diagonal(Direction4::D90)]),
        action: UnitAction::Take,
    }), Urc::new(|| 0.)).unwrap_err();
}

#[test]
fn magnet_pulls_through_pipe() {
    let config = Urc::new(Config::default());
    let map = PointMap::new(5, 4, false);
    let wmap: WrappingMap<Direction4> = WMBuilder::new(map).build();
    let mut map = Map::new(wmap, &config);
    let map_env = map.environment().clone();
    map.set_pipes(Point::new(1, 0), vec![PipeState::new(Direction4::D0, Direction4::D270).unwrap()]);
    map.set_pipes(Point::new(2, 0), vec![PipeState::new(Direction4::D0, Direction4::D180).unwrap()]);
    map.set_unit(Point::new(1, 2), Some(UnitType::MAGNET.instance(&map_env).set_owner_id(0).set_hp(100).build()));
    map.set_unit(Point::new(3, 0), Some(UnitType::SNIPER.instance(&map_env).set_owner_id(1).set_hp(100).build()));
    let settings = map.settings().unwrap();
    let (mut game, _) = Game::new_server(map, &settings, settings.build_default(), Urc::new(|| 0.));

    game.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(Point::new(1, 2)),
        action: UnitAction::Attack(AttackInput::SplashPattern(OrientedPoint::simple(Point::new(3, 0), Direction4::D0))),
    }), Urc::new(|| 0.)).unwrap();
    assert!(game.get_unit(Point::new(1, 1)).is_some());
    assert!(game.get_unit(Point::new(1, 2)).is_some());
    assert_eq!(None, game.get_unit(Point::new(3, 0)));
}

#[test]
fn fog_surprise() {
    let config = Urc::new(Config::default());
    let map = PointMap::new(8, 4, false);
    let wmap: WrappingMap<Direction4> = WMBuilder::new(map).build();
    let mut map = Map::new(wmap, &config);
    let map_env = map.environment().clone();
    map.set_unit(Point::new(0, 0), Some(UnitType::SMALL_TANK.instance(&map_env).set_owner_id(0).set_hp(100).build()));
    map.set_unit(Point::new(6, 0), Some(UnitType::SMALL_TANK.instance(&map_env).set_owner_id(1).set_hp(100).build()));
    let mut settings = map.settings().unwrap();
    settings.fog_mode = FogMode::Constant(FogSetting::ExtraDark(0));
    let (mut game, _) = Game::new_server(map, &settings, settings.build_default(), Urc::new(|| 0.));

    // no fog during the first turn
    game.handle_command(Command::EndTurn, Urc::new(|| 0.)).unwrap();
    game.handle_command(Command::EndTurn, Urc::new(|| 0.)).unwrap();

    let path = Path::with_steps(Point::new(0, 0), [PathStep::Dir(Direction4::D0); 6].to_vec());
    let events = game.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path,
        action: UnitAction::Wait,
    }), Urc::new(|| 0.)).unwrap();
    assert!(game.get_unit(Point::new(0, 0)).is_none());
    assert!(game.get_unit(Point::new(5, 0)).is_some());
    assert!(events.get(&Perspective::Team(0)).unwrap().contains(&Event::Effect(Effect::new_fog_surprise(Point::new(6, 0)))));
}
