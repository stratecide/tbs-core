use std::sync::Arc;

use interfaces::*;
use semver::Version;
use zipper::*;
use crate::combat::AttackInput;
use crate::commander::commander_type::CommanderType;
use crate::config::config::Config;
use crate::config::environment::Environment;
use crate::map::point_map::MapSize;
use crate::map::wrapping_map::OrientedPoint;
use crate::script::custom_action::CustomActionInput;
use crate::tokens::token::*;
use crate::game::commands::*;
use crate::game::fog::*;
use crate::game::game::Game;
use crate::game::game_view::GameView;
use crate::map::direction::*;
use crate::map::map::Map;
use crate::map::point::*;
use crate::map::point_map::PointMap;
use crate::map::wrapping_map::WMBuilder;
use crate::tags::*;
use crate::terrain::TerrainType;
use crate::tokens::token_types::TokenType;
use crate::units::commands::*;
use crate::units::movement::MovementType;
use crate::units::movement::Path;
use crate::tags::tests::*;
use crate::units::unit::Unit;
use crate::units::unit_types::UnitType;
use crate::VERSION;

// helpers
#[allow(non_upper_case_globals)]
impl CommanderType {
    pub const None: Self = Self(0);
    pub const Zombie: Self = Self(1);
    pub const Simo: Self = Self(2);
    pub const Vlad: Self = Self(3);
    pub const Tapio: Self = Self(4);
    pub const Lageos: Self = Self(5);
    pub const SludgeMonster: Self = Self(6);
    pub const Celerity: Self = Self(7);
}
#[test]
fn verify_commander_type_constants() {
    let config = Arc::new(Config::test_config());
    let environment = Environment::new_map(config, MapSize::new(5, 5));
    assert_eq!(environment.config.commander_name(CommanderType::None), "None");
    assert_eq!(environment.config.commander_name(CommanderType::Zombie), "Zombie");
    assert_eq!(environment.config.commander_name(CommanderType::Simo), "Simo");
    assert_eq!(environment.config.commander_name(CommanderType::Vlad), "Vlad");
    assert_eq!(environment.config.commander_name(CommanderType::Tapio), "Tapio");
    assert_eq!(environment.config.commander_name(CommanderType::Lageos), "Lageos");
    assert_eq!(environment.config.commander_name(CommanderType::SludgeMonster), "SludgeMonster");
    assert_eq!(environment.config.commander_name(CommanderType::Celerity), "Celerity");
}

// actual tests

#[test]
fn gain_charge() {
    let config = Arc::new(Config::test_config());
    let map = PointMap::new(5, 5, false);
    let map = WMBuilder::<Direction6>::new(map);
    let mut map = Map::new(map.build(), &config);
    let map_env = map.environment().clone();
    map.set_unit(Point::new(1, 1), Some(UnitType::small_tank().instance(&map_env).set_owner_id(0).set_hp(100).build()));
    map.set_unit(Point::new(2, 1), Some(UnitType::magnet().instance(&map_env).set_owner_id(1).set_hp(100).build()));

    let mut settings = map.settings().unwrap();
    settings.fog_mode = FogMode::Constant(FogSetting::None);
    let mut settings = settings.build_default();
    settings.players[0].set_commander(CommanderType::Zombie);
    settings.players[1].set_commander(CommanderType::Zombie);
    let (mut server, _) = Game::new_server(map.clone(), settings, Arc::new(|| 0.));
    server.with(|server| {
        assert_eq!(server.players.get(0).unwrap().commander.get_charge(), 0);
        assert_eq!(server.players.get(1).unwrap().commander.get_charge(), 0);
    });
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(Point::new(1, 1)),
        action: UnitAction::Attack(AttackInput::SplashPattern(OrientedPoint::simple(Point::new(2, 1), Direction6::D0))),
    }), Arc::new(|| 0.)).unwrap();
    server.with(|server| {
        let charge_0 = server.players.get(0).unwrap().commander.get_charge();
        let charge_1 = server.players.get(1).unwrap().commander.get_charge();
        println!("charges: {charge_0}, {charge_1}");
        assert!(charge_0 > 0);
        assert_eq!(charge_0 * 2, charge_1);
    });
}

#[test]
fn zombie() {
    let config = Arc::new(Config::test_config());
    let map = PointMap::new(5, 5, false);
    let map = WMBuilder::<Direction6>::new(map);
    let mut map = Map::new(map.build(), &config);
    let map_env = map.environment().clone();
    map.set_unit(Point::new(1, 1), Some(UnitType::small_tank().instance(&map_env).set_owner_id(0).build()));
    map.set_unit(Point::new(2, 1), Some(UnitType::small_tank().instance(&map_env).set_owner_id(1).set_hp(1).build()));

    map.set_unit(Point::new(4, 4), Some(UnitType::small_tank().instance(&map_env).set_owner_id(1).set_hp(1).build()));

    let mut skull = Token::new(map_env.clone(), TokenType::SKULL);
    skull.set_owner_id(0);
    skull.set_tag(TAG_UNIT_TYPE, TagValue::UnitType(UnitType::marine()));
    skull.set_tag(TAG_MOVEMENT_TYPE, TagValue::MovementType(MovementType::HOVER));
    map.set_tokens(Point::new(0, 4), vec![skull]);
    let mut skull2 = Token::new(map_env.clone(), TokenType::SKULL);
    skull2.set_owner_id(0);
    skull2.set_tag(TAG_UNIT_TYPE, TagValue::UnitType(UnitType::marine()));
    skull2.set_tag(TAG_MOVEMENT_TYPE, TagValue::MovementType(MovementType::FOOT));
    map.set_tokens(Point::new(1, 4), vec![skull2]);

    map.set_terrain(Point::new(3, 1), TerrainType::Factory.instance(&map_env).set_owner_id(0).build());

    let settings = map.settings().unwrap();
    let mut settings = settings.clone();
    for player in &settings.players {
        assert!(player.get_commander_options().contains(&CommanderType::Zombie));
    }
    settings.fog_mode = FogMode::Constant(FogSetting::None);
    let mut settings = settings.build_default();
    settings.players[0].set_commander(CommanderType::Zombie);
    let (mut server, _) = Game::new_server(map.clone(), settings, Arc::new(|| 0.));
    server.with_mut(|server| {
        server.players.get_mut(0).unwrap().commander.charge = server.players.get_mut(0).unwrap().commander.get_max_charge();
    });
    let unchanged = server.clone();
    let environment = server.environment().clone();
    let mut skull = Token::new(environment.clone(), TokenType::SKULL);
    skull.set_owner_id(0);
    skull.set_tag(TAG_UNIT_TYPE, TagValue::UnitType(UnitType::small_tank()));
    // no power
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(Point::new(1, 1)),
        action: UnitAction::Attack(AttackInput::SplashPattern(OrientedPoint::simple(Point::new(2, 1), Direction6::D0))),
    }), Arc::new(|| 0.)).unwrap();
    assert_eq!(server.get_unit(Point::new(2, 1)), None);
    assert_eq!(server.get_tokens(Point::new(1, 1)), vec![]);
    assert_eq!(server.get_tokens(Point::new(2, 1)), vec![skull.clone()]);
    // small power
    let mut server = unchanged.clone();
    server.handle_command(Command::commander_power(1, Vec::new()), Arc::new(|| 0.)).unwrap();
    assert_eq!(server.get_tokens(Point::new(0, 4)), Vec::new());
    assert_eq!(server.get_unit(Point::new(0, 4)), Some(UnitType::marine().instance(&environment).set_owner_id(0).set_hp(50).set_flag(FLAG_ZOMBIFIED).set_movement_type(MovementType::HOVER).build()));
    assert_eq!(server.get_unit(Point::new(1, 4)), Some(UnitType::marine().instance(&environment).set_owner_id(0).set_hp(50).set_flag(FLAG_ZOMBIFIED).set_movement_type(MovementType::FOOT).build()));
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(Point::new(1, 1)),
        action: UnitAction::Attack(AttackInput::SplashPattern(OrientedPoint::simple(Point::new(2, 1), Direction6::D0))),
    }), Arc::new(|| 0.)).unwrap();
    assert_eq!(server.get_unit(Point::new(2, 1)), None);
    assert_eq!(server.get_tokens(Point::new(2, 1)), vec![skull]);
    // big power
    let mut server = unchanged.clone();
    server.handle_command(Command::commander_power(2, Vec::new()), Arc::new(|| 0.)).unwrap();
    assert_eq!(server.get_tokens(Point::new(0, 4)), Vec::new());
    assert_eq!(server.get_unit(Point::new(0, 4)), Some(UnitType::marine().instance(&environment).set_owner_id(0).set_hp(50).set_flag(FLAG_ZOMBIFIED).set_movement_type(MovementType::HOVER).build()));
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(Point::new(1, 1)),
        action: UnitAction::Attack(AttackInput::SplashPattern(OrientedPoint::simple(Point::new(2, 1), Direction6::D0))),
    }), Arc::new(|| 0.)).unwrap();
    assert_eq!(server.get_tokens(Point::new(2, 1)), Vec::new());
    assert_eq!(server.get_unit(Point::new(2, 1)), Some(UnitType::small_tank().instance(&environment).set_owner_id(0).set_hp(50).set_flag(FLAG_ZOMBIFIED).build()));
    // buy unit
    let mut server = unchanged.clone();
    server.with_mut(|server| {
        server.players.get_mut(0).unwrap().funds = 1000.into();
    });
    server.handle_command(Command::TerrainAction(Point::new(3, 1), vec![
        CustomActionInput::ShopItem(0.into()),
    ].try_into().unwrap()), Arc::new(|| 0.)).unwrap();
    assert!(server.get_unit(Point::new(3, 1)).is_some());
    for p in server.all_points() {
        if let Some(unit) = server.get_unit(p) {
            assert!(!unit.has_flag(FLAG_ZOMBIFIED));
        }
    }
}

#[test]
fn simo() {
    let config = Arc::new(Config::test_config());
    let map = PointMap::new(6, 5, false);
    let map = WMBuilder::<Direction4>::new(map);
    let mut map = Map::new(map.build(), &config);
    let map_env = map.environment().clone();
    let arty_pos = Point::new(0, 1);
    map.set_unit(arty_pos, Some(UnitType::artillery().instance(&map_env).set_owner_id(0).set_hp(50).build()));

    let target_close = Point::new(3, 1);
    map.set_unit(target_close, Some(UnitType::small_tank().instance(&map_env).set_owner_id(1).set_hp(100).build()));
    let target_far = Point::new(4, 1);
    map.set_unit(target_far, Some(UnitType::small_tank().instance(&map_env).set_owner_id(1).set_hp(100).build()));
    let target_farthest = Point::new(5, 1);
    map.set_unit(target_farthest, Some(UnitType::small_tank().instance(&map_env).set_owner_id(1).set_hp(100).build()));

    let settings = map.settings().unwrap();

    let mut settings = settings.clone();
    for player in &settings.players {
        assert!(player.get_commander_options().contains(&CommanderType::Simo));
    }
    settings.fog_mode = FogMode::Constant(FogSetting::None);
    let mut settings = settings.build_default();
    settings.players[0].set_commander(CommanderType::Simo);
    let (mut server, _) = Game::new_server(map.clone(), settings, Arc::new(|| 0.));
    server.with_mut(|server| {
        server.players.get_mut(0).unwrap().commander.charge = server.players.get_mut(0).unwrap().commander.get_max_charge();
    });
    let unchanged = server.clone();

    // before chaos/order
    let arty = server.get_unit(arty_pos).unwrap();
    assert!(arty.shortest_path_to_attack(&*server, &Path::new(arty_pos), None, target_close).is_some());
    assert_eq!(arty.shortest_path_to_attack(&*server, &Path::new(arty_pos), None, target_far), None);
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(arty_pos),
        action: UnitAction::Attack(AttackInput::SplashPattern(OrientedPoint::simple(target_close, Direction4::D0))),
    }), Arc::new(|| 0.)).unwrap();
    assert!(server.get_unit(target_close).unwrap().get_hp() < 100);
    assert_eq!(server.get_unit(target_far).unwrap().get_hp(), 100);

    // embrace chaos
    let mut server = unchanged.clone();
    server.handle_command(Command::commander_power(1, Vec::new()), Arc::new(|| 0.)).unwrap();
    server.handle_command(Command::commander_power(4, Vec::new()), Arc::new(|| 0.)).err().unwrap();
    server.handle_command(Command::commander_power(5, Vec::new()), Arc::new(|| 0.)).err().unwrap();
    let arty = server.get_unit(arty_pos).unwrap();
    assert!(arty.shortest_path_to_attack(&*server, &Path::new(arty_pos), None, target_close).is_some());
    assert!(arty.shortest_path_to_attack(&*server, &Path::new(arty_pos), None, target_far).is_none());
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(arty_pos),
        action: UnitAction::Attack(AttackInput::SplashPattern(OrientedPoint::simple(target_close, Direction4::D0))),
    }), Arc::new(|| 0.)).unwrap();
    let hp_close = server.get_unit(target_close).unwrap().get_hp();
    let hp_far = server.get_unit(target_far).unwrap().get_hp();
    assert!(hp_far < 100);
    assert!(hp_close < hp_far);

    // chaos power
    let mut server = unchanged.clone();
    server.handle_command(Command::commander_power(1, Vec::new()), Arc::new(|| 0.)).unwrap();
    server.handle_command(Command::commander_power(3, Vec::new()), Arc::new(|| 0.)).unwrap();
    //let arty = server.get_unit(arty_pos).unwrap();
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(arty_pos),
        action: UnitAction::Attack(AttackInput::SplashPattern(OrientedPoint::simple(target_far, Direction4::D0))),
    }), Arc::new(|| 0.)).err().expect("range shouldn't be increased");
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(arty_pos),
        action: UnitAction::Attack(AttackInput::SplashPattern(OrientedPoint::simple(target_close, Direction4::D0))),
    }), Arc::new(|| 0.)).unwrap();
    assert!(server.get_unit(target_close).unwrap().get_hp() < hp_close);
    assert!(server.get_unit(target_far).unwrap().get_hp() < hp_far);
    assert_eq!(server.get_unit(target_farthest).unwrap().get_hp(), 100);

    // order power (small)
    let mut server = unchanged.clone();
    server.handle_command(Command::commander_power(2, Vec::new()), Arc::new(|| 0.)).unwrap();
    server.handle_command(Command::commander_power(3, Vec::new()), Arc::new(|| 0.)).err().unwrap();
    server.handle_command(Command::commander_power(4, Vec::new()), Arc::new(|| 0.)).unwrap();
    let arty = server.get_unit(arty_pos).unwrap();
    assert!(arty.shortest_path_to_attack(&*server, &Path::new(arty_pos), None, target_far).is_some());
    assert!(arty.shortest_path_to_attack(&*server, &Path::new(arty_pos), None, target_farthest).is_none());
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(arty_pos),
        action: UnitAction::Attack(AttackInput::SplashPattern(OrientedPoint::simple(target_far, Direction4::D0))),
    }), Arc::new(|| 0.)).unwrap();
    assert_eq!(server.get_unit(target_close).unwrap().get_hp(), 100);
    assert!(server.get_unit(target_far).unwrap().get_hp() < 100);

    // order power (big)
    let mut server = unchanged.clone();
    server.handle_command(Command::commander_power(2, Vec::new()), Arc::new(|| 0.)).unwrap();
    server.handle_command(Command::commander_power(5, Vec::new()), Arc::new(|| 0.)).unwrap();
    let arty = server.get_unit(arty_pos).unwrap();
    assert!(arty.shortest_path_to_attack(&*server, &Path::new(arty_pos), None, target_close).is_some());
    assert!(arty.shortest_path_to_attack(&*server, &Path::new(arty_pos), None, target_far).is_some());
    assert!(arty.shortest_path_to_attack(&*server, &Path::new(arty_pos), None, target_farthest).is_some());
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(arty_pos),
        action: UnitAction::Attack(AttackInput::SplashPattern(OrientedPoint::simple(target_farthest, Direction4::D0))),
    }), Arc::new(|| 0.)).unwrap();
    assert_eq!(server.get_unit(target_far).unwrap().get_hp(), 100);
    assert!(server.get_unit(target_farthest).unwrap().get_hp() < 100);
}

#[test]
fn vlad() {
    let config = Arc::new(Config::test_config());
    let map = PointMap::new(6, 5, false);
    let map = WMBuilder::<Direction4>::new(map);
    let mut map = Map::new(map.build(), &config);
    let map_env = map.environment().clone();
    let arty_pos = Point::new(0, 1);
    map.set_unit(arty_pos, Some(UnitType::artillery().instance(&map_env).set_owner_id(0).set_hp(50).build()));

    let target_close = Point::new(3, 1);
    map.set_unit(target_close, Some(UnitType::small_tank().instance(&map_env).set_owner_id(1).set_hp(50).build()));
    map.set_terrain(target_close, TerrainType::Flame.instance(&map_env).build());
    let target_far = Point::new(5, 4);
    map.set_unit(target_far, Some(UnitType::small_tank().instance(&map_env).set_owner_id(1).set_hp(50).build()));

    let mut settings = map.settings().unwrap();
    for player in &settings.players {
        assert!(player.get_commander_options().contains(&CommanderType::Vlad));
    }
    settings.fog_mode = FogMode::Constant(FogSetting::None);
    let mut sett = settings.build_default();
    sett.players[0].set_commander(CommanderType::Vlad);
    let (mut server, _) = Game::new_server(map.clone(), sett, Arc::new(|| 0.));

    // d2d daylight
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(arty_pos),
        action: UnitAction::Attack(AttackInput::SplashPattern(OrientedPoint::simple(target_close, Direction4::D0))),
    }), Arc::new(|| 0.)).unwrap();
    assert_eq!(server.get_unit(arty_pos).unwrap().get_hp(), 50);

    // d2d night
    settings.fog_mode = FogMode::Constant(FogSetting::Sharp(0));
    let mut settings = settings.build_default();
    settings.players[0].set_commander(CommanderType::Vlad);
    let (mut server, _) = Game::new_server(map.clone(), settings, Arc::new(|| 0.));
    server.with_mut(|server| {
        server.players.get_mut(0).unwrap().commander.charge = server.players.get_mut(0).unwrap().commander.get_max_charge();
    });
    let unchanged = server.clone();
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(arty_pos),
        action: UnitAction::Attack(AttackInput::SplashPattern(OrientedPoint::simple(target_close, Direction4::D0))),
    }), Arc::new(|| 0.)).unwrap();
    assert!(server.get_unit(arty_pos).unwrap().get_hp() > 50);

    // small power
    let mut server = unchanged.clone();
    server.handle_command(Command::commander_power(1, Vec::new()), Arc::new(|| 0.)).unwrap();
    assert_eq!(server.get_unit(arty_pos).unwrap().get_hp(), 50);
    assert!(server.get_unit(target_close).unwrap().get_hp() < 50);
    assert_eq!(server.get_unit(target_far).unwrap().get_hp(), 50);

    // big power
    let mut server = unchanged.clone();
    server.handle_command(Command::commander_power(2, Vec::new()), Arc::new(|| 0.)).unwrap();
    assert!(server.get_unit(arty_pos).unwrap().get_hp() > 50);
    assert!(server.get_unit(target_close).unwrap().get_hp() < 50);
    assert_eq!(server.get_unit(target_far).unwrap().get_hp(), 50);
}

#[test]
fn tapio() {
    let config = Arc::new(Config::test_config());
    let map = PointMap::new(6, 5, false);
    let map = WMBuilder::<Direction4>::new(map);
    let mut map = Map::new(map.build(), &config);
    let map_env = map.environment().clone();
    for p in map.all_points() {
        map.set_terrain(p, TerrainType::Street.instance(&map_env).build());
    }
    map.set_terrain(Point::new(0, 1), TerrainType::Grass.instance(&map_env).build());
    map.set_terrain(Point::new(0, 2), TerrainType::Forest.instance(&map_env).build());
    map.set_terrain(Point::new(0, 3), TerrainType::FairyForest.instance(&map_env).set_owner_id(0).build());
    for i in 0..4 {
        map.set_unit(Point::new(0, i), Some(UnitType::small_tank().instance(&map_env).set_owner_id(0).set_hp(100).build()));
        map.set_unit(Point::new(1, i), Some(UnitType::small_tank().instance(&map_env).set_owner_id(1).set_hp(100).build()));
        map.set_terrain(Point::new(2, i), TerrainType::Grass.instance(&map_env).build());
    }

    map.set_terrain(Point::new(5, 3), TerrainType::FairyForest.instance(&map_env).set_owner_id(0).build());
    map.set_terrain(Point::new(5, 4), TerrainType::FairyForest.instance(&map_env).set_owner_id(0).build());
    map.set_unit(Point::new(5, 4), Some(UnitType::small_tank().instance(&map_env).set_owner_id(1).set_hp(1).build()));

    map.set_terrain(Point::new(3, 0), TerrainType::FairyForest.instance(&map_env).set_owner_id(1).build());
    map.set_unit(Point::new(3, 0), Some(UnitType::small_tank().instance(&map_env).set_owner_id(1).set_hp(100).build()));

    let mut settings = map.settings().unwrap();
    for player in &settings.players {
        assert!(player.get_commander_options().contains(&CommanderType::Tapio));
    }
    settings.fog_mode = FogMode::Constant(FogSetting::Sharp(0));
    let mut settings = settings.build_default();
    settings.players[0].set_commander(CommanderType::Tapio);
    let (mut server, _) = Game::new_server(map.clone(), settings, Arc::new(|| 0.));
    server.with_mut(|server| {
        server.players.get_mut(0).unwrap().commander.charge = server.players.get_mut(0).unwrap().commander.get_max_charge();
    });
    let unchanged = server.clone();

    // passive: deal more damage when attacking from forest / grass
    for i in 0..4 {
        server.handle_command(Command::UnitCommand(UnitCommand {
            unload_index: None,
            path: Path::new(Point::new(0, i)),
            action: UnitAction::Attack(AttackInput::SplashPattern(OrientedPoint::simple(Point::new(1, i), Direction4::D0))),
        }), Arc::new(|| 0.)).unwrap();
    }
    assert!(server.get_unit(Point::new(1, 0)).unwrap().get_hp() > server.get_unit(Point::new(1, 1)).unwrap().get_hp(), "stronger attack from grass than street");
    assert!(server.get_unit(Point::new(1, 1)).unwrap().get_hp() > server.get_unit(Point::new(1, 2)).unwrap().get_hp(), "stronger attack from forest than grass");
    assert_eq!(server.get_unit(Point::new(1, 2)).unwrap().get_hp(), server.get_unit(Point::new(1, 3)).unwrap().get_hp(), "fairy forest == normal forest");
    let fairy_forest_hp = server.get_unit(Point::new(0, 3)).unwrap().get_hp();

    // fairy forest heals
    server.handle_command(Command::EndTurn, Arc::new(|| 0.)).unwrap();
    assert!(server.get_unit(Point::new(5, 4)).unwrap().get_hp() > 1, "heals even enemy units");
    assert_eq!(server.get_unit(Point::new(0, 3)).unwrap().get_hp(), fairy_forest_hp, "only heal on your own start turn event");
    server.handle_command(Command::EndTurn, Arc::new(|| 0.)).unwrap();
    assert!(server.get_unit(Point::new(0, 3)).unwrap().get_hp() > fairy_forest_hp, "heal on your own start turn event");

    // ACTIVE: turn grass into fairy forests
    let mut server = unchanged.clone();
    server.handle_command(Command::commander_power(1, vec![CustomActionInput::Point(Point::new(3, 0))]), Arc::new(|| 0.)).err().expect("can't turn street into fairy forest");
    for i in 0..2 {
        let charge_before = server.with(|server| server.players[0].commander.charge);
        server.handle_command(Command::commander_power(1, vec![CustomActionInput::Point(Point::new(2, i))]), Arc::new(|| 0.)).expect(&format!("loop {i}"));
        assert!(server.with(|server| server.players[0].commander.charge) < charge_before);
        assert_eq!(server.get_terrain(Point::new(2, i)).unwrap().typ(), TerrainType::FairyForest);
    }

    // ACTIVE: destroy own fairy forests, dealing damage to enemies nearby
    let mut server = unchanged.clone();
    server.handle_command(Command::commander_power(2, Vec::new()), Arc::new(|| 0.)).unwrap();
    assert_eq!(server.get_terrain(Point::new(5, 3)).unwrap().typ(), TerrainType::Grass);
    assert_eq!(server.get_terrain(Point::new(3, 0)).unwrap().typ(), TerrainType::FairyForest, "power doesn't affect others players' fairy forests");
    assert_eq!(server.get_unit(Point::new(0, 2)).unwrap().get_hp(), 100, "don't hurt your own units");
    assert!(server.get_unit(Point::new(1, 2)).unwrap().get_hp() < 100);

    // ACTIVE: see into fairy forests, build units from fairy forests
    let mut server = unchanged.clone();
    server.with_mut(|server| {
        server.players.get_mut(0).unwrap().funds = 10000.into();
    });
    server.handle_command(Command::EndTurn, Arc::new(|| 0.)).unwrap();
    server.handle_command(Command::EndTurn, Arc::new(|| 0.)).unwrap();
    assert_ne!(server.get_fog_at(ClientPerspective::Team(0), Point::new(5, 3)), FogIntensity::TrueSight);
    server.handle_command(Command::TerrainAction(Point::new(5, 3), vec![
        CustomActionInput::ShopItem(UnitType::small_tank().0.into()),
    ].try_into().unwrap()), Arc::new(|| 0.)).err().unwrap();
    server.handle_command(Command::commander_power(3, Vec::new()), Arc::new(|| 0.)).unwrap();
    assert_eq!(server.get_fog_at(ClientPerspective::Team(0), Point::new(5, 3)), FogIntensity::TrueSight);
    server.handle_command(Command::TerrainAction(Point::new(5, 3), vec![
        CustomActionInput::ShopItem(UnitType::small_tank().0.into()),
    ].try_into().unwrap()), Arc::new(|| 0.)).unwrap();
    assert!(server.get_unit(Point::new(5, 3)).unwrap().has_flag(FLAG_EXHAUSTED));
    assert_eq!(server.get_terrain(Point::new(5, 3)).unwrap().typ(), TerrainType::Grass);
}

#[test]
fn sludge_monster() {
    let config = Arc::new(Config::test_config());
    let map = PointMap::new(5, 5, false);
    let map = WMBuilder::<Direction4>::new(map);
    let mut map = Map::new(map.build(), &config);
    let map_env = map.environment().clone();
    map.set_terrain(Point::new(1, 0), TerrainType::City.instance(&map_env).set_owner_id(0).build());
    map.set_unit(Point::new(1, 0), Some(UnitType::small_tank().instance(&map_env).set_owner_id(0).set_hp(1).build()));
    map.set_unit(Point::new(0, 1), Some(UnitType::small_tank().instance(&map_env).set_owner_id(0).set_hp(50).build()));
    let mut sludge = Token::new(map_env.clone(), TokenType::SLUDGE);
    sludge.set_owner_id(1);
    sludge.set_tag(TAG_SLUDGE_COUNTER, 0.into());
    map.set_tokens(Point::new(2, 1), vec![sludge]);
    map.set_unit(Point::new(2, 1), Some(UnitType::small_tank().instance(&map_env).set_owner_id(0).set_hp(60).build()));
    map.set_terrain(Point::new(1, 2), TerrainType::OilPlatform.instance(&map_env).build());
    map.set_unit(Point::new(1, 2), Some(UnitType::small_tank().instance(&map_env).set_owner_id(0).set_hp(50).build()));
    map.set_unit(Point::new(1, 1), Some(UnitType::small_tank().instance(&map_env).set_owner_id(1).set_hp(100).build()));

    let settings = map.settings().unwrap();

    let mut settings = settings.clone();
    for player in &settings.players {
        assert!(player.get_commander_options().contains(&CommanderType::SludgeMonster));
    }
    settings.fog_mode = FogMode::Constant(FogSetting::None);
    let mut settings = settings.build_default();
    settings.players[0].set_commander(CommanderType::SludgeMonster);
    let (mut server, _) = Game::new_server(map.clone(), settings, Arc::new(|| 0.));
    let environment = server.environment();
    let sludge_token = move |owner_id: i8, counter: i32| {
        let mut sludge = Token::new(environment.clone(), TokenType::SLUDGE);
        sludge.set_owner_id(owner_id);
        sludge.set_tag(TAG_SLUDGE_COUNTER, counter.into());
        sludge
    };
    server.with_mut(|server| {
        server.players.get_mut(0).unwrap().commander.charge = server.players.get_mut(0).unwrap().commander.get_max_charge();
    });
    let unchanged = server.clone();

    // no power attack bonuses
    assert_eq!(server.get_unit(Point::new(0, 1)).unwrap().get_hp(), server.get_unit(Point::new(2, 1)).unwrap().get_hp(), "Sludge token should deal 10 damage");
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(Point::new(0, 1)),
        action: UnitAction::Attack(AttackInput::SplashPattern(OrientedPoint::simple(Point::new(1, 1), Direction4::D0))),
    }), Arc::new(|| 0.)).unwrap();
    let default_attack = 100 - server.get_unit(Point::new(1, 1)).unwrap().get_hp();
    let mut server = unchanged.clone();
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(Point::new(2, 1)),
        action: UnitAction::Attack(AttackInput::SplashPattern(OrientedPoint::simple(Point::new(1, 1), Direction4::D180))),
    }), Arc::new(|| 0.)).unwrap();
    let sludge_attack = 100 - server.get_unit(Point::new(1, 1)).unwrap().get_hp();
    let mut server = unchanged.clone();
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(Point::new(1, 2)),
        action: UnitAction::Attack(AttackInput::SplashPattern(OrientedPoint::simple(Point::new(1, 1), Direction4::D90))),
    }), Arc::new(|| 0.)).unwrap();
    let oil_platform_attack = 100 - server.get_unit(Point::new(1, 1)).unwrap().get_hp();
    assert!(default_attack < oil_platform_attack, "{default_attack} < {oil_platform_attack}");
    assert!(sludge_attack > oil_platform_attack, "{sludge_attack} > {oil_platform_attack} but sludge token should give +25% and oil platform only +20%");

    // leave token after unit death
    let mut server = unchanged.clone();
    assert_eq!(server.get_tokens(Point::new(0, 1)), &[]);
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(Point::new(1, 0)),
        action: UnitAction::Attack(AttackInput::SplashPattern(OrientedPoint::simple(Point::new(1, 1), Direction4::D270))),
    }), Arc::new(|| 0.)).unwrap();
    assert_eq!(server.get_tokens(Point::new(1, 0)), &[sludge_token(0, 0)]);

    // small power
    let mut server = unchanged.clone();
    assert_eq!(server.get_tokens(Point::new(0, 1)), &[]);
    assert_eq!(server.get_tokens(Point::new(1, 1)), &[]);
    server.handle_command(Command::commander_power(1, Vec::new()), Arc::new(|| 0.)).unwrap();
    assert_eq!(server.get_tokens(Point::new(0, 1)), &[sludge_token(0, 0)]);
    assert_eq!(server.get_tokens(Point::new(1, 1)), &[sludge_token(0, 0)]);

    // big power
    let mut server = unchanged.clone();
    assert_eq!(server.get_tokens(Point::new(0, 1)), &[]);
    assert_eq!(server.get_tokens(Point::new(1, 1)), &[]);
    assert_eq!(server.get_tokens(Point::new(2, 1)), &[sludge_token(1, 0)]);
    server.handle_command(Command::commander_power(2, Vec::new()), Arc::new(|| 0.)).unwrap();
    assert_eq!(server.get_tokens(Point::new(0, 1)), &[sludge_token(0, 1)]);
    assert_eq!(server.get_tokens(Point::new(2, 1)), &[sludge_token(0, 1)]);
    // tokens vanish over time
    server.handle_command(Command::EndTurn, Arc::new(|| 0.)).unwrap();
    assert_eq!(server.get_tokens(Point::new(0, 1)), &[sludge_token(0, 1)]);
    server.handle_command(Command::EndTurn, Arc::new(|| 0.)).unwrap();
    assert_eq!(server.get_tokens(Point::new(0, 1)), &[sludge_token(0, 0)]);
    server.handle_command(Command::EndTurn, Arc::new(|| 0.)).unwrap();
    assert_eq!(server.get_tokens(Point::new(0, 1)), &[sludge_token(0, 0)]);
    server.handle_command(Command::EndTurn, Arc::new(|| 0.)).unwrap();
    assert_eq!(server.get_tokens(Point::new(0, 1)), &[]);
    let mut server = unchanged.clone();
    assert_eq!(server.get_tokens(Point::new(2, 1)), &[sludge_token(1, 0)]);
    server.handle_command(Command::EndTurn, Arc::new(|| 0.)).unwrap();
    assert_eq!(server.get_tokens(Point::new(2, 1)), &[]);

    // can't repair
    let mut server = unchanged.clone();
    assert_eq!(server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(Point::new(1, 0)),
        action: UnitAction::custom(1, Vec::new()),
    }), Arc::new(|| 0.)), Err(CommandError::InvalidAction));
}

#[test]
fn celerity() {
    let config = Arc::new(Config::test_config());
    let map = PointMap::new(5, 5, false);
    let map = WMBuilder::<Direction4>::new(map);
    let mut map = Map::new(map.build(), &config);
    let map_env = map.environment().clone();
    map.set_unit(Point::new(0, 0), Some(UnitType::small_tank().instance(&map_env).set_owner_id(0).set_hp(1).build()));

    map.set_unit(Point::new(4, 0), Some(UnitType::convoy().instance(&map_env).set_owner_id(0).set_hp(100).set_transported(vec![
        UnitType::marine().instance(&map_env).set_hp(34).build(),
        UnitType::sniper().instance(&map_env).set_hp(69).build(),
    ]).set_hp(89).build()));

    map.set_unit(Point::new(2, 1), Some(UnitType::small_tank().instance(&map_env).set_owner_id(1).set_hp(1).build()));
    map.set_unit(Point::new(1, 2), Some(UnitType::small_tank().instance(&map_env).set_owner_id(1).set_hp(1).build()));
    map.set_unit(Point::new(2, 2), Some(UnitType::small_tank().instance(&map_env).set_owner_id(0).set_hp(100).build()));
    map.set_unit(Point::new(3, 2), Some(UnitType::small_tank().instance(&map_env).set_owner_id(1).set_hp(100).build()));
    map.set_unit(Point::new(2, 3), Some(UnitType::small_tank().instance(&map_env).set_owner_id(1).set_hp(1).build()));

    map.set_terrain(Point::new(0, 4), TerrainType::Factory.instance(&map_env).set_owner_id(0).build());

    let settings = map.settings().unwrap();

    let mut settings = settings.clone();
    for player in &settings.players {
        assert!(player.get_commander_options().contains(&CommanderType::Celerity));
    }
    settings.fog_mode = FogMode::Constant(FogSetting::None);
    let funds = 10000;
    settings.players[0].set_funds(funds);

    // get some default values without using Celerity
    let (mut server, _) = Game::new_server(map.clone(), settings.build_default(), Arc::new(|| 0.));
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(Point::new(2, 2)),
        action: UnitAction::Attack(AttackInput::SplashPattern(OrientedPoint::simple(Point::new(3, 2), Direction4::D0))),
    }), Arc::new(|| 0.)).unwrap();
    let default_attack = 100 - server.get_unit(Point::new(3, 2)).unwrap().get_hp();
    server.handle_command(Command::TerrainAction(Point::new(0, 4), vec![
        CustomActionInput::ShopItem(UnitType::small_tank().0.into()),
    ].try_into().unwrap()), Arc::new(|| 0.)).unwrap();
    let default_cost = funds - server.with(|server| *server.current_player().funds);
    assert!(!server.get_unit(Point::new(0, 4)).unwrap().get_tag(TAG_LEVEL).is_some());

    let mut settings = settings.build_default();
    settings.players[0].set_commander(CommanderType::Celerity);
    let (mut server, _) = Game::new_server(map.clone(), settings, Arc::new(|| 0.));
    let environment = server.environment().clone();
    server.with_mut(|server| {
        server.players.get_mut(0).unwrap().commander.charge = server.players.get_mut(0).unwrap().commander.get_max_charge();
    });
    let unchanged = server.clone();

    server.handle_command(Command::TerrainAction(Point::new(0, 4), vec![
        CustomActionInput::ShopItem(UnitType::small_tank().0.into()),
    ].try_into().unwrap()), Arc::new(|| 0.)).unwrap();
    assert!(funds - server.with(|server| *server.current_player().funds) < default_cost);
    assert_eq!(server.get_unit(Point::new(0, 4)).unwrap().get_tag(TAG_LEVEL), None, "New units are Level 0");

    // level attack bonuses
    let mut attack_damage = Vec::new();
    for i in 0..=3 {
        let mut server = unchanged.clone();
        for d in Direction4::list().into_iter().take(i + 1).rev() {
            let target = map.get_neighbor(Point::new(2, 2), d).unwrap().0;
            server.handle_command(Command::UnitCommand(UnitCommand {
                unload_index: None,
                path: Path::new(Point::new(2, 2)),
                action: UnitAction::Attack(AttackInput::SplashPattern(OrientedPoint::simple(target, d))),
            }), Arc::new(|| 0.)).unwrap();
            server.handle_command(Command::EndTurn, Arc::new(|| 0.)).unwrap();
            server.handle_command(Command::EndTurn, Arc::new(|| 0.)).unwrap();
        }
        if i > 0 {
            assert_eq!(server.get_unit(Point::new(2, 1)), None);
            assert_eq!(server.get_unit(Point::new(2, 2)).unwrap().get_tag(TAG_LEVEL), Some(TagValue::Int(Int32(i as i32))));
        }
        attack_damage.push(100 - server.get_unit(Point::new(3, 2)).unwrap().get_hp());
    }
    for i in 0..attack_damage.len() - 1 {
        assert!(attack_damage[i] < attack_damage[i + 1], "attack damage by level: {:?}", attack_damage);
    }
    assert_eq!(default_attack, attack_damage[1]);

    // small power
    let mut server = unchanged.clone();
    server.handle_command(Command::commander_power(1, Vec::new()), Arc::new(|| 0.)).unwrap();
    assert!(server.get_unit(Point::new(0, 0)).unwrap().get_hp() > 1);
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(Point::new(2, 2)),
        action: UnitAction::Attack(AttackInput::SplashPattern(OrientedPoint::simple(Point::new(3, 2), Direction4::D0))),
    }), Arc::new(|| 0.)).unwrap();
    assert_eq!(default_attack, 100 - server.get_unit(Point::new(3, 2)).unwrap().get_hp());

    // big power
    let mut server = unchanged.clone();
    server.handle_command(Command::commander_power(2, Vec::new()), Arc::new(|| 0.)).unwrap();
    assert_eq!(server.get_unit(Point::new(0, 0)).unwrap().get_hp(), 1);
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(Point::new(2, 2)),
        action: UnitAction::Attack(AttackInput::SplashPattern(OrientedPoint::simple(Point::new(3, 2), Direction4::D0))),
    }), Arc::new(|| 0.)).unwrap();
    assert_eq!(attack_damage[3], 100 - server.get_unit(Point::new(3, 2)).unwrap().get_hp());

    let convoy: Unit<Direction6> = UnitType::convoy().instance(&environment).set_owner_id(0).set_transported(vec![
        UnitType::marine().instance(&environment).set_hp(34).build(),
        UnitType::sniper().instance(&environment).set_hp(69).build(),
    ]).set_hp(89).build();

    let mut zipper = Zipper::new();
    convoy.zip(&mut zipper, false);
    let mut unzipper = Unzipper::new(zipper.finish(), Version::parse(VERSION).unwrap());
    let convoy2 = Unit::unzip(&mut unzipper, &environment, false);
    assert_eq!(Ok(convoy), convoy2);

    let exported = unchanged.export();
    let imported = *Game::import_server(exported, &config, Version::parse(VERSION).unwrap()).unwrap();
    unchanged.with(|unchanged| assert_eq!(imported, *unchanged));
    assert_eq!(server.get_unit(Point::new(4, 0)), Some(UnitType::convoy().instance(&environment).set_owner_id(0).set_transported(vec![
        UnitType::marine().instance(&environment).set_hp(34).build(),
        UnitType::sniper().instance(&environment).set_hp(69).build(),
    ]).set_hp(89).build()));
}

#[test]
fn lageos() {
    let config = Arc::new(Config::test_config());
    let map = PointMap::new(8, 5, false);
    let map = WMBuilder::<Direction4>::new(map);
    let mut map = Map::new(map.build(), &config);
    let map_env = map.environment().clone();
    map.set_unit(Point::new(0, 0), Some(UnitType::attack_heli().instance(&map_env).set_owner_id(1).set_hp(100).build()));
    map.set_unit(Point::new(0, 1), Some(UnitType::attack_heli().instance(&map_env).set_owner_id(1).set_hp(100).build()));
    map.set_unit(Point::new(1, 0), Some(UnitType::attack_heli().instance(&map_env).set_owner_id(0).set_hp(100).build()));
    map.set_unit(Point::new(1, 1), Some(UnitType::small_tank().instance(&map_env).set_owner_id(0).set_hp(100).build()));

    let settings = map.settings().unwrap();
    for player in &settings.players {
        assert!(player.get_commander_options().contains(&CommanderType::Lageos));
    }
    // get some default values without using Lageos
    let mut settings = map.settings().unwrap().build_default();
    settings.fog_mode = FogMode::Constant(FogSetting::Sharp(0));
    let (mut server, _) = Game::new_server(map.clone(), settings, Arc::new(|| 0.));
    server.handle_command(Command::EndTurn, Arc::new(|| 0.)).unwrap();
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(Point::new(0, 0)),
        action: UnitAction::Attack(AttackInput::SplashPattern(OrientedPoint::simple(Point::new(1, 0), Direction4::D0))),
    }), Arc::new(|| 0.)).unwrap();
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(Point::new(0, 1)),
        action: UnitAction::Attack(AttackInput::SplashPattern(OrientedPoint::simple(Point::new(1, 1), Direction4::D0))),
    }), Arc::new(|| 0.)).unwrap();
    let damage_to_neutral_heli = 100 - server.get_unit(Point::new(1, 0)).unwrap().get_hp();
    let damage_to_neutral_tank = 100 - server.get_unit(Point::new(1, 1)).unwrap().get_hp();
    let neutral_vision_range = (0..map.width()).find(|x| server.get_fog_at(ClientPerspective::Team(0), Point::new(*x, 0)) == FogIntensity::Dark).unwrap();

    let mut settings = map.settings().unwrap().build_default();
    settings.fog_mode = FogMode::Constant(FogSetting::Sharp(0));
    settings.players[0].set_commander(CommanderType::Lageos);
    let (mut server, _) = Game::new_server(map.clone(), settings, Arc::new(|| 0.));
    server.with_mut(|server| {
        server.players.get_mut(0).unwrap().commander.charge = server.players.get_mut(0).unwrap().commander.get_max_charge();
    });
    let unchanged = server.clone();

    // Lageos' air-units have have higher defense, Lageos has +1 vision
    server.handle_command(Command::EndTurn, Arc::new(|| 0.)).unwrap();
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(Point::new(0, 0)),
        action: UnitAction::Attack(AttackInput::SplashPattern(OrientedPoint::simple(Point::new(1, 0), Direction4::D0))),
    }), Arc::new(|| 0.)).unwrap();
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(Point::new(0, 1)),
        action: UnitAction::Attack(AttackInput::SplashPattern(OrientedPoint::simple(Point::new(1, 1), Direction4::D0))),
    }), Arc::new(|| 0.)).unwrap();
    let damage_to_lageos_heli = 100 - server.get_unit(Point::new(1, 0)).unwrap().get_hp();
    assert!(damage_to_neutral_heli > damage_to_lageos_heli, "{damage_to_neutral_heli} > {damage_to_lageos_heli}");
    assert_eq!(damage_to_neutral_tank, 100 - server.get_unit(Point::new(1, 1)).unwrap().get_hp());
    assert!(neutral_vision_range < (0..map.width()).find(|x| server.get_fog_at(ClientPerspective::Team(0), Point::new(*x, 0)) == FogIntensity::Dark).unwrap());

    // small power
    let mut server = unchanged.clone();
    server.handle_command(Command::commander_power(1, Vec::new()), Arc::new(|| 0.)).unwrap_err();
    server.handle_command(Command::commander_power(1, vec![
        CustomActionInput::Point(Point::new(0, 1)),
        CustomActionInput::Point(Point::new(0, 1)),
        CustomActionInput::Point(Point::new(2, 1)),
    ]), Arc::new(|| 0.)).unwrap();
    assert_eq!(server.get_unit(Point::new(1, 0)).unwrap().get_hp(), 100);
    assert!(server.get_unit(Point::new(0, 1)).unwrap().get_hp() < 100);
    assert!(server.get_unit(Point::new(0, 0)).unwrap().get_hp() < 100);
    assert_eq!(3 * (100 - server.get_unit(Point::new(0, 1)).unwrap().get_hp()), 2 * (100 - server.get_unit(Point::new(1, 1)).unwrap().get_hp()));
    assert!(!server.get_unit(Point::new(0, 1)).unwrap().has_flag(FLAG_EXHAUSTED));

    // big power
    let mut server = unchanged.clone();
    server.handle_command(Command::commander_power(2, Vec::new()), Arc::new(|| 0.)).unwrap_err();
    server.handle_command(Command::commander_power(2, vec![
        CustomActionInput::Point(Point::new(0, 1)),
        CustomActionInput::Point(Point::new(0, 1)),
    ]), Arc::new(|| 0.)).unwrap_err();
    server.handle_command(Command::commander_power(2, vec![
        CustomActionInput::Point(Point::new(0, 1)),
    ]), Arc::new(|| 0.)).unwrap();
    assert_eq!(server.get_unit(Point::new(1, 0)).unwrap().get_hp(), 100);
    assert!(server.get_unit(Point::new(0, 1)).unwrap().get_hp() < 100);
    assert!(server.get_unit(Point::new(1, 1)).unwrap().get_hp() < 100);
    assert!(server.get_unit(Point::new(0, 1)).unwrap().has_flag(FLAG_STUNNED));
    assert!(server.get_unit(Point::new(1, 1)).unwrap().has_flag(FLAG_EXHAUSTED));
    assert!(!server.get_unit(Point::new(1, 1)).unwrap().has_flag(FLAG_STUNNED));
    server.handle_command(Command::EndTurn, Arc::new(|| 0.)).unwrap();
    assert!(server.get_unit(Point::new(0, 1)).unwrap().has_flag(FLAG_EXHAUSTED));
}
