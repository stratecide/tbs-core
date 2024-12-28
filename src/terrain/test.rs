use std::sync::Arc;

use crate::commander::commander_type::CommanderType;
use crate::config::config::Config;
use crate::config::environment::Environment;
use crate::game::commands::Command;
use crate::game::fog::*;
use crate::game::game::Game;
use crate::game::game_view::GameView;
use crate::map::direction::*;
use crate::map::map::Map;
use crate::map::point::*;
use crate::map::point_map::{MapSize, PointMap};
use crate::script::custom_action::test::CA_UNIT_CAPTURE;
use crate::script::custom_action::CustomActionInput;
use crate::terrain::TerrainType;
use crate::map::wrapping_map::*;
use crate::units::combat::AttackVector;
use crate::units::commands::{UnitCommand, UnitAction};
use crate::units::movement::{Path, PathStep};
use crate::tags::tests::*;
use crate::units::unit_types::UnitType;

// helpers
#[allow(non_upper_case_globals)]
impl TerrainType {
    pub const Airport: Self = Self(0);
    pub const ChessPawnTile: Self = Self(3);
    pub const ChessTile: Self = Self(4);
    pub const City: Self = Self(5);
    pub const Factory: Self = Self(7);
    pub const Flame: Self = Self(8);
    pub const Forest: Self = Self(9);
    pub const Grass: Self = Self(11);
    pub const Kraken: Self = Self(15);
    pub const Mountain: Self = Self(17);
    pub const OilPlatform: Self = Self(18);
    pub const Sea: Self = Self(21);
    pub const ShallowSea: Self = Self(22);
    pub const Street: Self = Self(23);
    pub const StatueLand: Self = Self(24);
    pub const TentacleDepths: Self = Self(25);
    pub const FairyForest: Self = Self(28);
}
#[test]
fn verify_terrain_type_constants() {
    let config = Arc::new(Config::test_config());
    let environment = Environment::new_map(config, MapSize::new(5, 5));
    assert_eq!(environment.config.terrain_name(TerrainType::ChessPawnTile), "ChessPawnTile");
    assert_eq!(environment.config.terrain_name(TerrainType::ChessTile), "ChessTile");
    assert_eq!(environment.config.terrain_name(TerrainType::City), "City");
    assert_eq!(environment.config.terrain_name(TerrainType::Factory), "Factory");
    assert_eq!(environment.config.terrain_name(TerrainType::Flame), "Flame");
    assert_eq!(environment.config.terrain_name(TerrainType::Forest), "Forest");
    assert_eq!(environment.config.terrain_name(TerrainType::Grass), "Grass");
    assert_eq!(environment.config.terrain_name(TerrainType::Kraken), "Kraken");
    assert_eq!(environment.config.terrain_name(TerrainType::Mountain), "Mountain");
    assert_eq!(environment.config.terrain_name(TerrainType::OilPlatform), "OilPlatform");
    assert_eq!(environment.config.terrain_name(TerrainType::Sea), "Sea");
    assert_eq!(environment.config.terrain_name(TerrainType::ShallowSea), "ShallowSea");
    assert_eq!(environment.config.terrain_name(TerrainType::Street), "Street");
    assert_eq!(environment.config.terrain_name(TerrainType::StatueLand), "StatueLand");
    assert_eq!(environment.config.terrain_name(TerrainType::TentacleDepths), "TentacleDepths");
    assert_eq!(environment.config.terrain_name(TerrainType::FairyForest), "FairyForest");
}

// actual tests

#[test]
fn capture_city() {
    let map = PointMap::new(4, 4, false);
    let environment = Environment::new_map(Arc::new(Config::test_config()), map.size());
    let wmap: WrappingMap<Direction4> = WMBuilder::new(map).build();
    let mut map = Map::new2(wmap, &environment);
    map.set_terrain(Point::new(1, 1), TerrainType::City.instance(&environment).set_owner_id(-1).build());
    map.set_unit(Point::new(0, 0), Some(UnitType::sniper().instance(&environment).set_owner_id(0).set_hp(55).build()));
    map.set_unit(Point::new(3, 3), Some(UnitType::sniper().instance(&environment).set_owner_id(1).build()));
    let settings = map.settings().unwrap().build_default();
    let (mut game, _) = Game::new_server(map, settings, Arc::new(|| 0.));
    game.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::with_steps(Point::new(0, 0), vec![
            PathStep::Dir(Direction4::D0),
            PathStep::Dir(Direction4::D270),
        ]),
        action: UnitAction::custom(CA_UNIT_CAPTURE, Vec::new()),
    }), Arc::new(|| 0.)).unwrap();
    game.handle_command(Command::EndTurn, Arc::new(|| 0.)).unwrap();
    assert_eq!(-1, game.get_terrain(Point::new(1, 1)).unwrap().get_owner_id());
    assert!(game.get_unit(Point::new(1, 1)).unwrap().has_flag(FLAG_CAPTURING));
    game.handle_command(Command::EndTurn, Arc::new(|| 0.)).unwrap();
    assert_eq!(-1, game.get_terrain(Point::new(1, 1)).unwrap().get_owner_id());
    assert!(!game.get_unit(Point::new(1, 1)).unwrap().has_flag(FLAG_CAPTURING));
    assert_eq!(Some(6.into()), game.get_terrain(Point::new(1, 1)).unwrap().get_tag(TAG_CAPTURE_PROGRESS));
    assert_eq!(Some(0.into()), game.get_terrain(Point::new(1, 1)).unwrap().get_tag(TAG_CAPTURE_OWNER));
    game.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::with_steps(Point::new(1, 1), Vec::new()),
        action: UnitAction::custom(CA_UNIT_CAPTURE, Vec::new()),
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
    map.set_terrain(Point::new(0, 0), TerrainType::Factory.instance(&environment).set_owner_id(0).build());
    map.set_terrain(Point::new(1, 1), TerrainType::City.instance(&environment).set_owner_id(0).build());
    map.set_unit(Point::new(3, 3), Some(UnitType::sniper().instance(&environment).set_owner_id(1).build()));
    let mut settings = map.settings().unwrap();
    settings.players[0].set_income(1000);
    let (mut game, _) = Game::new_server(map, settings.build_default(), Arc::new(|| 0.));
    assert_eq!(1000, game.with(|game| *game.current_player().funds));
    game.handle_command(Command::TerrainAction(Point::new(0, 0), vec![
        CustomActionInput::ShopItem(0.into()),
    ].try_into().unwrap()), Arc::new(|| 0.)).unwrap();
    assert!(game.with(|game| *game.current_player().funds) < 1000);
    assert_eq!(0, game.get_unit(Point::new(0, 0)).unwrap().get_owner_id());
    assert!(game.get_unit(Point::new(0, 0)).unwrap().has_flag(FLAG_EXHAUSTED));
    //assert_eq!(Some(TagValue::Int(Int32(1))), game.get_terrain(Point::new(0, 0)).unwrap().get_tag(TAG_BUILT_THIS_TURN));
    assert!(game.get_terrain(Point::new(0, 0)).unwrap().has_flag(FLAG_EXHAUSTED));
}

#[test]
fn kraken() {
    let map = PointMap::new(5, 5, false);
    let map_env = Environment::new_map(Arc::new(Config::test_config()), map.size());
    let wmap: WrappingMap<Direction4> = WMBuilder::new(map).build();
    let mut map = Map::new2(wmap, &map_env);
    for x in 0..map.width() {
        for y in 0..map.height() {
            map.set_terrain(Point::new(x, y), TerrainType::TentacleDepths.instance(&map_env).build());
        }
    }
    map.set_terrain(Point::new(2, 2), TerrainType::Kraken.instance(&map_env).set_tag(TAG_ANGER, 7.into()).build());
    map.set_terrain(Point::new(0, 0), TerrainType::Kraken.instance(&map_env).build());
    map.set_unit(Point::new(2, 1), Some(UnitType::war_ship().instance(&map_env).set_owner_id(1).set_hp(100).build()));
    map.set_unit(Point::new(1, 2), Some(UnitType::war_ship().instance(&map_env).set_owner_id(0).set_hp(100).build()));
    map.set_unit(Point::new(3, 2), Some(UnitType::war_ship().instance(&map_env).set_owner_id(0).set_hp(100).build()));
    let settings = map.settings().unwrap();
    let (mut game, _) = Game::new_server(map, settings.build_default(), Arc::new(|| 0.));
    let environment = game.environment();
    assert_eq!(game.get_unit(Point::new(1, 0)), Some(UnitType::tentacle().instance(&environment).set_hp(100).build()));
    assert_eq!(game.get_terrain(Point::new(2, 2)).unwrap().get_tag(TAG_ANGER), Some(7.into()));
    game.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(Point::new(3, 2)),
        action: UnitAction::Attack(AttackVector::Direction(Direction4::D0)),
    }), Arc::new(|| 0.)).unwrap();
    assert_eq!(game.get_unit(Point::new(4, 2)), None);
    assert_eq!(game.get_terrain(Point::new(2, 2)).unwrap().get_tag(TAG_ANGER), Some(8.into()));
    assert_eq!(game.get_unit(Point::new(3, 2)).unwrap().get_hp(), 100);
    game.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(Point::new(1, 2)),
        action: UnitAction::Attack(AttackVector::Direction(Direction4::D180)),
    }), Arc::new(|| 0.)).unwrap();
    assert_eq!(game.get_terrain(Point::new(2, 2)).unwrap().get_tag(TAG_ANGER), None);
    assert_eq!(game.get_terrain(Point::new(0, 0)).unwrap().get_tag(TAG_ANGER), Some(2.into()));
    assert!(game.get_unit(Point::new(3, 2)).unwrap().get_hp() < 100);
}

#[test]
fn terrain_vision() {
    let map = PointMap::new(4, 4, false);
    let environment = Environment::new_map(Arc::new(Config::test_config()), map.size());
    let wmap: WrappingMap<Direction4> = WMBuilder::new(map).build();
    let mut map = Map::new2(wmap, &environment);
    map.set_terrain(Point::new(0, 0), TerrainType::Factory.instance(&environment).set_owner_id(0).build());
    map.set_terrain(Point::new(3, 0), TerrainType::City.instance(&environment).set_owner_id(0).build());
    map.set_terrain(Point::new(0, 3), TerrainType::Airport.instance(&environment).set_owner_id(1).build());
    map.set_terrain(Point::new(3, 3), TerrainType::City.instance(&environment).set_owner_id(-1).build());
    let mut settings = map.settings().unwrap();
    settings.fog_mode = FogMode::Constant(FogSetting::Sharp(0));
    let mut settings = settings.build_default();
    settings.players[0].set_commander(CommanderType::Lageos);
    let (mut game, _) = Game::new_server(map, settings, Arc::new(|| 0.));
    game.handle_command(Command::EndTurn, Arc::new(|| 0.)).unwrap();
    // neutral city gives no vision to anybody
    assert_eq!(FogIntensity::Dark, game.get_fog_at(interfaces::ClientPerspective::Neutral, Point::new(3, 3)));
    assert_eq!(FogIntensity::Dark, game.get_fog_at(interfaces::ClientPerspective::Team(0), Point::new(3, 3)));
    assert_eq!(FogIntensity::Dark, game.get_fog_at(interfaces::ClientPerspective::Team(1), Point::new(3, 3)));
    // player 0's factory gives true vision, but only to its owner
    assert_eq!(FogIntensity::Dark, game.get_fog_at(interfaces::ClientPerspective::Neutral, Point::new(0, 0)));
    assert_eq!(FogIntensity::TrueSight, game.get_fog_at(interfaces::ClientPerspective::Team(0), Point::new(0, 0)));
    assert_eq!(FogIntensity::Dark, game.get_fog_at(interfaces::ClientPerspective::Team(1), Point::new(0, 0)));
    // player 0's factory doesn't give vision next to itself, even if the commander's units have +1 vision
    assert_eq!(FogIntensity::Dark, game.get_fog_at(interfaces::ClientPerspective::Team(0), Point::new(1, 0)));
    // player 1's airport gives true vision, but only to its owner
    assert_eq!(FogIntensity::Dark, game.get_fog_at(interfaces::ClientPerspective::Neutral, Point::new(0, 3)));
    assert_eq!(FogIntensity::Dark, game.get_fog_at(interfaces::ClientPerspective::Team(0), Point::new(0, 3)));
    assert_eq!(FogIntensity::TrueSight, game.get_fog_at(interfaces::ClientPerspective::Team(1), Point::new(0, 3)));
    // player 0's city gives vision, but only to its owner
    assert_eq!(FogIntensity::Dark, game.get_fog_at(interfaces::ClientPerspective::Neutral, Point::new(3, 0)));
    assert!(FogIntensity::Dark > game.get_fog_at(interfaces::ClientPerspective::Team(0), Point::new(3, 0)));
    assert_eq!(FogIntensity::Dark, game.get_fog_at(interfaces::ClientPerspective::Team(1), Point::new(3, 0)));
}
