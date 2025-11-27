use interfaces::*;
use semver::Version;
use uniform_smart_pointer::Urc;
use zipper::*;
use crate::combat::AttackInput;
use crate::commander::commander_type::CommanderType;
use crate::config::config::Config;
use crate::config::environment::Environment;
use crate::map::board::Board;
use crate::map::board::BoardView;
use crate::map::map::get_neighbors;
use crate::map::map::valid_points;
use crate::map::point_map::MapSize;
use crate::map::wrapping_map::OrientedPoint;
use crate::script::custom_action::CustomActionInput;
use crate::script::custom_action::test::CA_UNIT_CLEAN_SLUDGE;
use crate::tokens::token::*;
use crate::game::commands::*;
use crate::game::fog::*;
use crate::game::game::Game;
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
use crate::units::movement::PathStep;
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
    let config = Urc::new(Config::default());
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
    let config = Urc::new(Config::default());
    let map = PointMap::new(5, 5, false);
    let map = WMBuilder::<Direction6>::new(map);
    let mut map = Map::new(map.build(), &config);
    let map_env = map.environment().clone();
    map.set_unit(Point::new(1, 1), Some(UnitType::SMALL_TANK.instance(&map_env).set_owner_id(0).set_hp(100).build()));
    map.set_unit(Point::new(2, 1), Some(UnitType::MAGNET.instance(&map_env).set_owner_id(1).set_hp(100).build()));

    let mut map_settings = map.settings().unwrap();
    map_settings.fog_mode = FogMode::Constant(FogSetting::None);
    let mut settings = map_settings.build_default();
    settings.players[0].set_commander(CommanderType::Zombie);
    settings.players[1].set_commander(CommanderType::Zombie);
    let (mut server, _) = Game::new_server(map.clone(), &map_settings, settings, Urc::new(|| 0.));
    assert_eq!(server.players.get(0).unwrap().commander.get_charge(), 0);
    assert_eq!(server.players.get(1).unwrap().commander.get_charge(), 0);
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(Point::new(1, 1)),
        action: UnitAction::Attack(AttackInput::SplashPattern(OrientedPoint::simple(Point::new(2, 1), Direction6::D0))),
    }), Urc::new(|| 0.)).unwrap();
    let charge_0 = server.players.get(0).unwrap().commander.get_charge();
    let charge_1 = server.players.get(1).unwrap().commander.get_charge();
    crate::debug!("charges: {charge_0}, {charge_1}");
    assert!(charge_0 > 0);
    assert_eq!(charge_0 * 2, charge_1);
}

#[test]
fn zombie() {
    let config = Urc::new(Config::default());
    let map = PointMap::new(5, 5, false);
    let map = WMBuilder::<Direction6>::new(map);
    let mut map = Map::new(map.build(), &config);
    let map_env = map.environment().clone();
    map.set_unit(Point::new(1, 1), Some(UnitType::SMALL_TANK.instance(&map_env).set_owner_id(0).build()));
    map.set_unit(Point::new(2, 1), Some(UnitType::SMALL_TANK.instance(&map_env).set_owner_id(1).set_hp(1).build()));

    map.set_unit(Point::new(4, 4), Some(UnitType::SMALL_TANK.instance(&map_env).set_owner_id(1).set_hp(1).build()));

    let mut skull = Token::new(map_env.clone(), TokenType::SKULL);
    skull.set_owner_id(0);
    skull.set_tag(TAG_UNIT_TYPE, TagValue::UnitType(UnitType::MARINE));
    skull.set_tag(TAG_MOVEMENT_TYPE, TagValue::MovementType(MovementType::HOVER));
    map.set_tokens(Point::new(0, 4), vec![skull]);
    let mut skull2 = Token::new(map_env.clone(), TokenType::SKULL);
    skull2.set_owner_id(0);
    skull2.set_tag(TAG_UNIT_TYPE, TagValue::UnitType(UnitType::MARINE));
    skull2.set_tag(TAG_MOVEMENT_TYPE, TagValue::MovementType(MovementType::FOOT));
    map.set_tokens(Point::new(1, 4), vec![skull2]);

    map.set_terrain(Point::new(3, 1), TerrainType::Factory.instance(&map_env).set_owner_id(0).build());

    let mut map_settings = map.settings().unwrap();
    for player in &map_settings.players {
        assert!(player.get_commander_options().contains(&CommanderType::Zombie));
    }
    map_settings.fog_mode = FogMode::Constant(FogSetting::None);
    let mut settings = map_settings.build_default();
    settings.players[0].set_commander(CommanderType::Zombie);
    let (mut server, _) = Game::new_server(map.clone(), &map_settings, settings, Urc::new(|| 0.));
    server.players.get_mut(0).unwrap().commander.charge = server.players.get_mut(0).unwrap().commander.get_max_charge();
    let unchanged = server.clone();
    let environment = server.environment().clone();
    let mut skull = Token::new(environment.clone(), TokenType::SKULL);
    skull.set_owner_id(0);
    skull.set_tag(TAG_UNIT_TYPE, TagValue::UnitType(UnitType::SMALL_TANK));
    // no power
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(Point::new(1, 1)),
        action: UnitAction::Attack(AttackInput::SplashPattern(OrientedPoint::simple(Point::new(2, 1), Direction6::D0))),
    }), Urc::new(|| 0.)).unwrap();
    assert_eq!(server.get_unit(Point::new(2, 1)), None);
    assert_eq!(server.get_tokens(Point::new(1, 1)), vec![]);
    assert_eq!(server.get_tokens(Point::new(2, 1)), vec![skull.clone()]);
    // small power
    let mut server = unchanged.clone();
    server.handle_command(Command::commander_power(1, Vec::new()), Urc::new(|| 0.)).unwrap();
    assert_eq!(server.get_tokens(Point::new(0, 4)), Vec::new());
    assert_eq!(server.get_unit(Point::new(0, 4)), Some(&UnitType::MARINE.instance(&environment).set_owner_id(0).set_hp(50).set_flag(FLAG_ZOMBIFIED).set_movement_type(MovementType::HOVER).build()));
    assert_eq!(server.get_unit(Point::new(1, 4)), Some(&UnitType::MARINE.instance(&environment).set_owner_id(0).set_hp(50).set_flag(FLAG_ZOMBIFIED).set_movement_type(MovementType::FOOT).build()));
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(Point::new(1, 1)),
        action: UnitAction::Attack(AttackInput::SplashPattern(OrientedPoint::simple(Point::new(2, 1), Direction6::D0))),
    }), Urc::new(|| 0.)).unwrap();
    assert_eq!(server.get_unit(Point::new(2, 1)), None);
    assert_eq!(server.get_tokens(Point::new(2, 1)), vec![skull]);
    // big power
    let mut server = unchanged.clone();
    server.handle_command(Command::commander_power(2, Vec::new()), Urc::new(|| 0.)).unwrap();
    assert_eq!(server.get_tokens(Point::new(0, 4)), Vec::new());
    assert_eq!(server.get_unit(Point::new(0, 4)), Some(&UnitType::MARINE.instance(&environment).set_owner_id(0).set_hp(50).set_flag(FLAG_ZOMBIFIED).set_movement_type(MovementType::HOVER).build()));
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(Point::new(1, 1)),
        action: UnitAction::Attack(AttackInput::SplashPattern(OrientedPoint::simple(Point::new(2, 1), Direction6::D0))),
    }), Urc::new(|| 0.)).unwrap();
    assert_eq!(server.get_tokens(Point::new(2, 1)), Vec::new());
    assert_eq!(server.get_unit(Point::new(2, 1)), Some(&UnitType::SMALL_TANK.instance(&environment).set_owner_id(0).set_hp(50).set_flag(FLAG_ZOMBIFIED).build()));
    // buy unit
    let mut server = unchanged.clone();
    server.players.get_mut(0).unwrap().set_tag(&environment, TAG_FUNDS, 1000.into());
    server.handle_command(Command::TerrainAction(Point::new(3, 1), vec![
        CustomActionInput::ShopItem(0.into()),
    ].try_into().unwrap()), Urc::new(|| 0.)).unwrap();
    assert!(server.get_unit(Point::new(3, 1)).is_some());
    for p in valid_points(&server) {
        if let Some(unit) = server.get_unit(p) {
            assert!(!unit.has_flag(FLAG_ZOMBIFIED));
        }
    }
}

#[test]
fn simo() {
    let config = Urc::new(Config::default());
    let map = PointMap::new(6, 5, false);
    let map = WMBuilder::<Direction4>::new(map);
    let mut map = Map::new(map.build(), &config);
    let map_env = map.environment().clone();
    let arty_pos = Point::new(0, 1);
    map.set_unit(arty_pos, Some(UnitType::ARTILLERY.instance(&map_env).set_owner_id(0).set_hp(50).build()));

    let target_close = Point::new(3, 1);
    map.set_unit(target_close, Some(UnitType::SMALL_TANK.instance(&map_env).set_owner_id(1).set_hp(100).build()));
    let target_far = Point::new(4, 1);
    map.set_unit(target_far, Some(UnitType::SMALL_TANK.instance(&map_env).set_owner_id(1).set_hp(100).build()));
    let target_farthest = Point::new(5, 1);
    map.set_unit(target_farthest, Some(UnitType::SMALL_TANK.instance(&map_env).set_owner_id(1).set_hp(100).build()));

    let mut map_settings = map.settings().unwrap();
    for player in &map_settings.players {
        assert!(player.get_commander_options().contains(&CommanderType::Simo));
    }
    map_settings.fog_mode = FogMode::Constant(FogSetting::None);
    let mut settings = map_settings.build_default();
    settings.players[0].set_commander(CommanderType::Simo);
    let (mut server, _) = Game::new_server(map.clone(), &map_settings, settings, Urc::new(|| 0.));
    server.players.get_mut(0).unwrap().commander.charge = server.players.get_mut(0).unwrap().commander.get_max_charge();
    let unchanged = server.clone();

    // before chaos/order
    let arty = server.get_unit(arty_pos).unwrap();
    let board = Board::new(&server);
    assert!(arty.shortest_path_to_attack(&board, &Path::new(arty_pos), None, target_close).is_some());
    assert_eq!(arty.shortest_path_to_attack(&board, &Path::new(arty_pos), None, target_far), None);
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(arty_pos),
        action: UnitAction::Attack(AttackInput::SplashPattern(OrientedPoint::simple(target_close, Direction4::D0))),
    }), Urc::new(|| 0.)).unwrap();
    assert!(server.get_unit(target_close).unwrap().get_hp() < 100);
    assert_eq!(server.get_unit(target_far).unwrap().get_hp(), 100);

    // embrace chaos
    let mut server = unchanged.clone();
    server.handle_command(Command::commander_power(1, Vec::new()), Urc::new(|| 0.)).unwrap();
    server.handle_command(Command::commander_power(4, Vec::new()), Urc::new(|| 0.)).err().unwrap();
    server.handle_command(Command::commander_power(5, Vec::new()), Urc::new(|| 0.)).err().unwrap();
    let arty = server.get_unit(arty_pos).unwrap();
    let board = Board::new(&server);
    assert!(arty.shortest_path_to_attack(&board, &Path::new(arty_pos), None, target_close).is_some());
    assert!(arty.shortest_path_to_attack(&board, &Path::new(arty_pos), None, target_far).is_none());
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(arty_pos),
        action: UnitAction::Attack(AttackInput::SplashPattern(OrientedPoint::simple(target_close, Direction4::D0))),
    }), Urc::new(|| 0.)).unwrap();
    let hp_close = server.get_unit(target_close).unwrap().get_hp();
    let hp_far = server.get_unit(target_far).unwrap().get_hp();
    assert!(hp_far < 100);
    assert!(hp_close < hp_far);

    // chaos power
    let mut server = unchanged.clone();
    server.handle_command(Command::commander_power(1, Vec::new()), Urc::new(|| 0.)).unwrap();
    server.handle_command(Command::commander_power(3, Vec::new()), Urc::new(|| 0.)).unwrap();
    //let arty = server.get_unit(arty_pos).unwrap();
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(arty_pos),
        action: UnitAction::Attack(AttackInput::SplashPattern(OrientedPoint::simple(target_far, Direction4::D0))),
    }), Urc::new(|| 0.)).err().expect("range shouldn't be increased");
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(arty_pos),
        action: UnitAction::Attack(AttackInput::SplashPattern(OrientedPoint::simple(target_close, Direction4::D0))),
    }), Urc::new(|| 0.)).unwrap();
    assert!(server.get_unit(target_close).unwrap().get_hp() < hp_close);
    assert!(server.get_unit(target_far).unwrap().get_hp() < hp_far);
    assert_eq!(server.get_unit(target_farthest).unwrap().get_hp(), 100);

    // order power (small)
    let mut server = unchanged.clone();
    server.handle_command(Command::commander_power(2, Vec::new()), Urc::new(|| 0.)).unwrap();
    server.handle_command(Command::commander_power(3, Vec::new()), Urc::new(|| 0.)).err().unwrap();
    server.handle_command(Command::commander_power(4, Vec::new()), Urc::new(|| 0.)).unwrap();
    let arty = server.get_unit(arty_pos).unwrap();
    let board = Board::new(&server);
    assert!(arty.shortest_path_to_attack(&board, &Path::new(arty_pos), None, target_far).is_some());
    assert!(arty.shortest_path_to_attack(&board, &Path::new(arty_pos), None, target_farthest).is_none());
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(arty_pos),
        action: UnitAction::Attack(AttackInput::SplashPattern(OrientedPoint::simple(target_far, Direction4::D0))),
    }), Urc::new(|| 0.)).unwrap();
    assert_eq!(server.get_unit(target_close).unwrap().get_hp(), 100);
    assert!(server.get_unit(target_far).unwrap().get_hp() < 100);

    // order power (big)
    let mut server = unchanged.clone();
    server.handle_command(Command::commander_power(2, Vec::new()), Urc::new(|| 0.)).unwrap();
    server.handle_command(Command::commander_power(5, Vec::new()), Urc::new(|| 0.)).unwrap();
    let arty = server.get_unit(arty_pos).unwrap();
    let board = Board::new(&server);
    assert!(arty.shortest_path_to_attack(&board, &Path::new(arty_pos), None, target_close).is_some());
    assert!(arty.shortest_path_to_attack(&board, &Path::new(arty_pos), None, target_far).is_some());
    assert!(arty.shortest_path_to_attack(&board, &Path::new(arty_pos), None, target_farthest).is_some());
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(arty_pos),
        action: UnitAction::Attack(AttackInput::SplashPattern(OrientedPoint::simple(target_farthest, Direction4::D0))),
    }), Urc::new(|| 0.)).unwrap();
    assert_eq!(server.get_unit(target_far).unwrap().get_hp(), 100);
    assert!(server.get_unit(target_farthest).unwrap().get_hp() < 100);
}

#[test]
fn vlad() {
    let config = Urc::new(Config::default());
    let map = PointMap::new(6, 5, false);
    let map = WMBuilder::<Direction4>::new(map);
    let mut map = Map::new(map.build(), &config);
    let map_env = map.environment().clone();
    let arty_pos = Point::new(0, 1);
    map.set_unit(arty_pos, Some(UnitType::ARTILLERY.instance(&map_env).set_owner_id(0).set_hp(50).build()));

    let target_close = Point::new(3, 1);
    map.set_unit(target_close, Some(UnitType::SMALL_TANK.instance(&map_env).set_owner_id(1).set_hp(50).build()));
    map.set_terrain(target_close, TerrainType::Flame.instance(&map_env).build());
    let target_far = Point::new(5, 4);
    map.set_unit(target_far, Some(UnitType::SMALL_TANK.instance(&map_env).set_owner_id(1).set_hp(50).build()));

    let mut map_settings = map.settings().unwrap();
    for player in &map_settings.players {
        assert!(player.get_commander_options().contains(&CommanderType::Vlad));
    }
    map_settings.fog_mode = FogMode::Constant(FogSetting::None);
    let mut settings = map_settings.build_default();
    settings.players[0].set_commander(CommanderType::Vlad);
    let (mut server, _) = Game::new_server(map.clone(), &map_settings, settings, Urc::new(|| 0.));

    // d2d daylight
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(arty_pos),
        action: UnitAction::Attack(AttackInput::SplashPattern(OrientedPoint::simple(target_close, Direction4::D0))),
    }), Urc::new(|| 0.)).unwrap();
    assert_eq!(server.get_unit(arty_pos).unwrap().get_hp(), 50);

    // d2d night
    map_settings.fog_mode = FogMode::Constant(FogSetting::Sharp(0));
    let mut settings = map_settings.build_default();
    settings.players[0].set_commander(CommanderType::Vlad);
    let (mut server, _) = Game::new_server(map.clone(), &map_settings, settings, Urc::new(|| 0.));
    server.players.get_mut(0).unwrap().commander.charge = server.players.get_mut(0).unwrap().commander.get_max_charge();
    let unchanged = server.clone();
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(arty_pos),
        action: UnitAction::Attack(AttackInput::SplashPattern(OrientedPoint::simple(target_close, Direction4::D0))),
    }), Urc::new(|| 0.)).unwrap();
    assert!(server.get_unit(arty_pos).unwrap().get_hp() > 50);

    // small power
    let mut server = unchanged.clone();
    server.handle_command(Command::commander_power(1, Vec::new()), Urc::new(|| 0.)).unwrap();
    assert_eq!(server.get_unit(arty_pos).unwrap().get_hp(), 50);
    assert!(server.get_unit(target_close).unwrap().get_hp() < 50);
    assert_eq!(server.get_unit(target_far).unwrap().get_hp(), 50);

    // big power
    let mut server = unchanged.clone();
    server.handle_command(Command::commander_power(2, Vec::new()), Urc::new(|| 0.)).unwrap();
    assert!(server.get_unit(arty_pos).unwrap().get_hp() > 50);
    assert!(server.get_unit(target_close).unwrap().get_hp() < 50);
    assert_eq!(server.get_unit(target_far).unwrap().get_hp(), 50);
}

#[test]
fn tapio() {
    let config = Urc::new(Config::default());
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
        map.set_unit(Point::new(0, i), Some(UnitType::SMALL_TANK.instance(&map_env).set_owner_id(0).set_hp(100).build()));
        map.set_unit(Point::new(1, i), Some(UnitType::SMALL_TANK.instance(&map_env).set_owner_id(1).set_hp(100).build()));
        map.set_terrain(Point::new(2, i), TerrainType::Grass.instance(&map_env).build());
    }

    map.set_terrain(Point::new(5, 3), TerrainType::FairyForest.instance(&map_env).set_owner_id(0).build());
    map.set_terrain(Point::new(5, 4), TerrainType::FairyForest.instance(&map_env).set_owner_id(0).build());
    map.set_unit(Point::new(5, 4), Some(UnitType::SMALL_TANK.instance(&map_env).set_owner_id(1).set_hp(1).build()));

    map.set_terrain(Point::new(3, 0), TerrainType::FairyForest.instance(&map_env).set_owner_id(1).build());
    map.set_unit(Point::new(3, 0), Some(UnitType::SMALL_TANK.instance(&map_env).set_owner_id(1).set_hp(100).build()));

    let mut map_settings = map.settings().unwrap();
    for player in &map_settings.players {
        assert!(player.get_commander_options().contains(&CommanderType::Tapio));
    }
    map_settings.fog_mode = FogMode::Constant(FogSetting::Sharp(0));
    let mut settings = map_settings.build_default();
    settings.players[0].set_commander(CommanderType::Tapio);
    let (mut server, _) = Game::new_server(map.clone(), &map_settings, settings, Urc::new(|| 0.));
    server.players.get_mut(0).unwrap().commander.charge = server.players.get_mut(0).unwrap().commander.get_max_charge();
    let unchanged = server.clone();

    // passive: deal more damage when attacking from forest / grass
    for i in 0..4 {
        server.handle_command(Command::UnitCommand(UnitCommand {
            unload_index: None,
            path: Path::new(Point::new(0, i)),
            action: UnitAction::Attack(AttackInput::SplashPattern(OrientedPoint::simple(Point::new(1, i), Direction4::D0))),
        }), Urc::new(|| 0.)).unwrap();
    }
    assert!(server.get_unit(Point::new(1, 0)).unwrap().get_hp() > server.get_unit(Point::new(1, 1)).unwrap().get_hp(), "stronger attack from grass than street");
    assert!(server.get_unit(Point::new(1, 1)).unwrap().get_hp() > server.get_unit(Point::new(1, 2)).unwrap().get_hp(), "stronger attack from forest than grass");
    assert_eq!(server.get_unit(Point::new(1, 2)).unwrap().get_hp(), server.get_unit(Point::new(1, 3)).unwrap().get_hp(), "fairy forest == normal forest");
    let fairy_forest_hp = server.get_unit(Point::new(0, 3)).unwrap().get_hp();

    // fairy forest heals
    server.handle_command(Command::EndTurn, Urc::new(|| 0.)).unwrap();
    assert!(server.get_unit(Point::new(5, 4)).unwrap().get_hp() > 1, "heals even enemy units");
    assert_eq!(server.get_unit(Point::new(0, 3)).unwrap().get_hp(), fairy_forest_hp, "only heal on your own start turn event");
    server.handle_command(Command::EndTurn, Urc::new(|| 0.)).unwrap();
    assert!(server.get_unit(Point::new(0, 3)).unwrap().get_hp() > fairy_forest_hp, "heal on your own start turn event");

    // ACTIVE: turn grass into fairy forests
    let mut server = unchanged.clone();
    server.handle_command(Command::commander_power(1, vec![CustomActionInput::Point(Point::new(3, 0))]), Urc::new(|| 0.)).err().expect("can't turn street into fairy forest");
    for i in 0..2 {
        let charge_before = server.players[0].commander.charge;
        server.handle_command(Command::commander_power(1, vec![CustomActionInput::Point(Point::new(2, i))]), Urc::new(|| 0.)).expect(&format!("loop {i}"));
        assert!(server.players[0].commander.charge < charge_before);
        assert_eq!(server.get_terrain(Point::new(2, i)).unwrap().typ(), TerrainType::FairyForest);
    }

    // ACTIVE: destroy own fairy forests, dealing damage to enemies nearby
    let mut server = unchanged.clone();
    server.handle_command(Command::commander_power(2, Vec::new()), Urc::new(|| 0.)).unwrap();
    assert_eq!(server.get_terrain(Point::new(5, 3)).unwrap().typ(), TerrainType::Grass);
    assert_eq!(server.get_terrain(Point::new(3, 0)).unwrap().typ(), TerrainType::FairyForest, "power doesn't affect others players' fairy forests");
    assert_eq!(server.get_unit(Point::new(0, 2)).unwrap().get_hp(), 100, "don't hurt your own units");
    assert!(server.get_unit(Point::new(1, 2)).unwrap().get_hp() < 100);

    // ACTIVE: see into fairy forests, build units from fairy forests
    let mut server = unchanged.clone();
    let environment = server.environment().clone();
    server.players.get_mut(0).unwrap().set_tag(&environment, TAG_FUNDS, 10000.into());
    server.handle_command(Command::EndTurn, Urc::new(|| 0.)).unwrap();
    server.handle_command(Command::EndTurn, Urc::new(|| 0.)).unwrap();
    assert_ne!(server.get_fog_at(ClientPerspective::Team(0), Point::new(5, 3)), FogIntensity::TrueSight);
    server.handle_command(Command::TerrainAction(Point::new(5, 3), vec![
        CustomActionInput::ShopItem(UnitType::SMALL_TANK.0.into()),
    ].try_into().unwrap()), Urc::new(|| 0.)).err().unwrap();
    server.handle_command(Command::commander_power(3, Vec::new()), Urc::new(|| 0.)).unwrap();
    assert_eq!(server.get_fog_at(ClientPerspective::Team(0), Point::new(5, 3)), FogIntensity::TrueSight);
    server.handle_command(Command::TerrainAction(Point::new(5, 3), vec![
        CustomActionInput::ShopItem(UnitType::SMALL_TANK.0.into()),
    ].try_into().unwrap()), Urc::new(|| 0.)).unwrap();
    assert!(server.get_unit(Point::new(5, 3)).unwrap().has_flag(FLAG_EXHAUSTED));
    assert_eq!(server.get_terrain(Point::new(5, 3)).unwrap().typ(), TerrainType::Grass);
}

#[test]
fn sludge_monster() {
    let config = Urc::new(Config::default());
    let map = PointMap::new(7, 7, false);
    let map = WMBuilder::<Direction4>::new(map);
    let mut map = Map::new(map.build(), &config);
    let map_env = map.environment().clone();
    let sludge_token = Token::new(map_env.clone(), TokenType::SLUDGE);

    // 1. for testing damage from token after turn-start
    let p_sludge_owned = Point::new(1, 0);
    map.set_tokens(p_sludge_owned, vec![sludge_token.clone()]);
    map.set_unit(p_sludge_owned, Some(UnitType::SNIPER.instance(&map_env).set_owner_id(0).set_hp(100).build()));
    let p_sludge_enemy = Point::new(0, 1);
    map.set_tokens(p_sludge_enemy, vec![sludge_token.clone()]);
    map.set_unit(p_sludge_enemy, Some(UnitType::SNIPER.instance(&map_env).set_owner_id(1).set_hp(100).build()));

    // 2. for testing bonus-attack from standing on token
    map.set_tokens(Point::new(6, 0), vec![sludge_token.clone()]);
    let p_attack_owned = Point::new(5, 0);
    map.set_unit(p_attack_owned, Some(UnitType::SMALL_TANK.instance(&map_env).set_owner_id(0).set_hp(100).build()));
    let p_attack_enemy = Point::new(6, 1);
    map.set_unit(p_attack_enemy, Some(UnitType::SMALL_TANK.instance(&map_env).set_owner_id(1).set_hp(100).build()));

    // 3. for testing if dying units leave behind a sludge token
    let p_die_owned = Point::new(1, 5);
    map.set_unit(p_die_owned, Some(UnitType::SMALL_TANK.instance(&map_env).set_owner_id(0).set_hp(1).build()));
    let p_die_enemy = Point::new(1, 6);
    map.set_unit(p_die_enemy, Some(UnitType::SMALL_TANK.instance(&map_env).set_owner_id(1).set_hp(1).build()));

    let mut map_settings = map.settings().unwrap();
    for player in &map_settings.players {
        assert!(player.get_commander_options().contains(&CommanderType::SludgeMonster));
    }
    map_settings.fog_mode = FogMode::Constant(FogSetting::None);
    let mut settings = map_settings.build_default();
    settings.players[0].set_commander(CommanderType::SludgeMonster);
    let (mut server, _) = Game::new_server(map.clone(), &map_settings, settings, Urc::new(|| 0.));
    server.players.get_mut(0).unwrap().commander.charge = server.players.get_mut(0).unwrap().commander.get_max_charge();
    let unchanged = server.clone();
    let sludge_token = Token::new(unchanged.environment().clone(), TokenType::SLUDGE);

    // 1. SludgeMonster's units don't get damaged by sludge, but other players' units do
    assert_eq!(server.get_unit(p_sludge_owned).unwrap().get_hp(), 100);
    assert_eq!(server.get_unit(p_sludge_enemy).unwrap().get_hp(), 100);
    server.handle_command(Command::EndTurn, Urc::new(|| 0.)).unwrap();
    assert_eq!(server.get_unit(p_sludge_owned).unwrap().get_hp(), 100);
    assert!(server.get_unit(p_sludge_enemy).unwrap().get_hp() < 100);

    // 2. SludgeMonster gets attack bonus when attacking from sludge token, other players don't
    let mut server = unchanged.clone();
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::with_steps(p_attack_owned, vec![PathStep::Dir(Direction4::D270)]),
        action: UnitAction::Attack(AttackInput::SplashPattern(OrientedPoint::simple(p_attack_enemy, Direction4::D0))),
    }), Urc::new(|| 0.)).unwrap();
    let base_dmg_owned = 100 - server.get_unit(p_attack_enemy).unwrap().get_hp();
    let mut server = unchanged.clone();
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::with_steps(p_attack_owned, vec![PathStep::Dir(Direction4::D0)]),
        action: UnitAction::Attack(AttackInput::SplashPattern(OrientedPoint::simple(p_attack_enemy, Direction4::D270))),
    }), Urc::new(|| 0.)).unwrap();
    assert!(server.get_unit(p_attack_enemy).unwrap().get_hp() < 100 - base_dmg_owned);
    // now check that enemies don't get the same bonus
    let mut server = unchanged.clone();
    server.handle_command(Command::EndTurn, Urc::new(|| 0.)).unwrap();
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::with_steps(p_attack_enemy, vec![PathStep::Dir(Direction4::D90)]),
        action: UnitAction::Attack(AttackInput::SplashPattern(OrientedPoint::simple(p_attack_owned, Direction4::D180))),
    }), Urc::new(|| 0.)).unwrap();
    let base_dmg_owned = 100 - server.get_unit(p_attack_owned).unwrap().get_hp();
    let mut server = unchanged.clone();
    server.handle_command(Command::EndTurn, Urc::new(|| 0.)).unwrap();
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::with_steps(p_attack_enemy, vec![PathStep::Dir(Direction4::D180)]),
        action: UnitAction::Attack(AttackInput::SplashPattern(OrientedPoint::simple(p_attack_owned, Direction4::D90))),
    }), Urc::new(|| 0.)).unwrap();
    assert_eq!(server.get_unit(p_attack_owned).unwrap().get_hp(), 100 - base_dmg_owned);

    // 3. SludgeMonster's units leave a sludge token behind when dying, other players don't
    let mut server = unchanged.clone();
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(p_die_owned),
        action: UnitAction::Attack(AttackInput::SplashPattern(OrientedPoint::simple(p_die_enemy, Direction4::D270))),
    }), Urc::new(|| 0.)).unwrap();
    assert_eq!(server.get_tokens(p_die_owned).len(), 0);
    assert_eq!(server.get_tokens(p_die_enemy).len(), 0);
    let mut server = unchanged.clone();
    server.handle_command(Command::EndTurn, Urc::new(|| 0.)).unwrap();
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(p_die_enemy),
        action: UnitAction::Attack(AttackInput::SplashPattern(OrientedPoint::simple(p_die_owned, Direction4::D90))),
    }), Urc::new(|| 0.)).unwrap();
    assert_eq!(server.get_tokens(p_die_owned).len(), 1);
    assert_eq!(server.get_tokens(p_die_enemy).len(), 0);

    // 4. SludgeMonster's power spreads sludge (at its unit positions, around its units if there's already sludge under the unit)
    let mut server = unchanged.clone();
    server.handle_command(Command::commander_power(1, Vec::new()), Urc::new(|| 0.)).unwrap();
    for p in get_neighbors(&server, p_sludge_owned, crate::map::map::NeighborMode::Direct) {
        assert_eq!(server.get_tokens(p.point), vec![sludge_token.clone()]);
    }
    assert_eq!(server.get_tokens(p_die_owned), vec![sludge_token.clone()]);
    assert_eq!(server.get_tokens(p_die_enemy), Vec::new()); // enemy doesn't spread sludge

    // 5. While SludgeMonster's power is active, more sludge is spread when its units die
    server.handle_command(Command::EndTurn, Urc::new(|| 0.)).unwrap();
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(p_die_enemy),
        action: UnitAction::Attack(AttackInput::SplashPattern(OrientedPoint::simple(p_die_owned, Direction4::D90))),
    }), Urc::new(|| 0.)).unwrap();
    assert_eq!(server.get_tokens(p_die_owned).len(), 1);
    for p in get_neighbors(&server, p_die_owned, crate::map::map::NeighborMode::Direct) {
        assert_eq!(server.get_tokens(p.point), vec![sludge_token.clone()]);
    }

    // 6. sludge token can be removed by Infantry
    let mut server = unchanged.clone();
    server.handle_command(Command::EndTurn, Urc::new(|| 0.)).unwrap();
    let board = Board::new(&server);
    let path = Path::new(p_die_enemy);
    assert!(!server.get_unit(p_die_enemy).unwrap().options_after_path(&board, &path, None, &[]).contains(&UnitAction::custom(CA_UNIT_CLEAN_SLUDGE, Vec::new())));
    let path = Path::new(p_sludge_enemy);
    let options = server.get_unit(p_sludge_enemy).unwrap().options_after_path(&board, &path, None, &[]);
    assert!(options.contains(&UnitAction::custom(CA_UNIT_CLEAN_SLUDGE, Vec::new())), "{options:?}");
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path,
        action: UnitAction::custom(CA_UNIT_CLEAN_SLUDGE, Vec::new()),
    }), Urc::new(|| 0.)).unwrap();
    assert_eq!(server.get_tokens(p_sludge_enemy).len(), 0);
}

#[test]
fn celerity() {
    let config = Urc::new(Config::default());
    let map = PointMap::new(5, 5, false);
    let map = WMBuilder::<Direction4>::new(map);
    let mut map = Map::new(map.build(), &config);
    let map_env = map.environment().clone();
    map.set_unit(Point::new(0, 0), Some(UnitType::SMALL_TANK.instance(&map_env).set_owner_id(0).set_hp(1).build()));

    map.set_unit(Point::new(4, 0), Some(UnitType::CONVOY.instance(&map_env).set_owner_id(0).set_hp(100).set_transported(vec![
        UnitType::MARINE.instance(&map_env).set_hp(34).build(),
        UnitType::SNIPER.instance(&map_env).set_hp(69).build(),
    ]).set_hp(89).build()));

    map.set_unit(Point::new(2, 1), Some(UnitType::SMALL_TANK.instance(&map_env).set_owner_id(1).set_hp(1).build()));
    map.set_unit(Point::new(1, 2), Some(UnitType::SMALL_TANK.instance(&map_env).set_owner_id(1).set_hp(1).build()));
    map.set_unit(Point::new(2, 2), Some(UnitType::SMALL_TANK.instance(&map_env).set_owner_id(0).set_hp(100).build()));
    map.set_unit(Point::new(3, 2), Some(UnitType::SMALL_TANK.instance(&map_env).set_owner_id(1).set_hp(100).build()));
    map.set_unit(Point::new(2, 3), Some(UnitType::SMALL_TANK.instance(&map_env).set_owner_id(1).set_hp(1).build()));

    map.set_terrain(Point::new(0, 4), TerrainType::Factory.instance(&map_env).set_owner_id(0).build());

    let mut map_settings = map.settings().unwrap();
    for player in &map_settings.players {
        assert!(player.get_commander_options().contains(&CommanderType::Celerity));
    }
    map_settings.fog_mode = FogMode::Constant(FogSetting::None);
    let funds = 10000;
    map_settings.players[0].get_tag_bag_mut().set_tag(&map_env, TAG_FUNDS, funds.into());

    // get some default values without using Celerity
    let (mut server, _) = Game::new_server(map.clone(), &map_settings, map_settings.build_default(), Urc::new(|| 0.));
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(Point::new(2, 2)),
        action: UnitAction::Attack(AttackInput::SplashPattern(OrientedPoint::simple(Point::new(3, 2), Direction4::D0))),
    }), Urc::new(|| 0.)).unwrap();
    let default_attack = 100 - server.get_unit(Point::new(3, 2)).unwrap().get_hp();
    server.handle_command(Command::TerrainAction(Point::new(0, 4), vec![
        CustomActionInput::ShopItem(UnitType::SMALL_TANK.0.into()),
    ].try_into().unwrap()), Urc::new(|| 0.)).unwrap();
    let default_cost = funds - server.current_player().get_tag(TAG_FUNDS).unwrap().into_dynamic().cast::<i32>();
    assert!(!server.get_unit(Point::new(0, 4)).unwrap().get_tag(TAG_LEVEL).is_some());

    let mut settings = map_settings.build_default();
    settings.players[0].set_commander(CommanderType::Celerity);
    let (mut server, _) = Game::new_server(map.clone(), &map_settings, settings, Urc::new(|| 0.));
    let environment = server.environment().clone();
    server.players.get_mut(0).unwrap().commander.charge = server.players.get_mut(0).unwrap().commander.get_max_charge();
    let unchanged = server.clone();

    server.handle_command(Command::TerrainAction(Point::new(0, 4), vec![
        CustomActionInput::ShopItem(UnitType::SMALL_TANK.0.into()),
    ].try_into().unwrap()), Urc::new(|| 0.)).unwrap();
    assert!(funds - server.current_player().get_tag(TAG_FUNDS).unwrap().into_dynamic().cast::<i32>() < default_cost);
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
            }), Urc::new(|| 0.)).unwrap();
            server.handle_command(Command::EndTurn, Urc::new(|| 0.)).unwrap();
            server.handle_command(Command::EndTurn, Urc::new(|| 0.)).unwrap();
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
    server.handle_command(Command::commander_power(1, Vec::new()), Urc::new(|| 0.)).unwrap();
    assert!(server.get_unit(Point::new(0, 0)).unwrap().get_hp() > 1);
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(Point::new(2, 2)),
        action: UnitAction::Attack(AttackInput::SplashPattern(OrientedPoint::simple(Point::new(3, 2), Direction4::D0))),
    }), Urc::new(|| 0.)).unwrap();
    assert_eq!(default_attack, 100 - server.get_unit(Point::new(3, 2)).unwrap().get_hp());

    // big power
    let mut server = unchanged.clone();
    server.handle_command(Command::commander_power(2, Vec::new()), Urc::new(|| 0.)).unwrap();
    assert_eq!(server.get_unit(Point::new(0, 0)).unwrap().get_hp(), 1);
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(Point::new(2, 2)),
        action: UnitAction::Attack(AttackInput::SplashPattern(OrientedPoint::simple(Point::new(3, 2), Direction4::D0))),
    }), Urc::new(|| 0.)).unwrap();
    assert_eq!(attack_damage[3], 100 - server.get_unit(Point::new(3, 2)).unwrap().get_hp());

    let convoy: Unit<Direction6> = UnitType::CONVOY.instance(&environment).set_owner_id(0).set_transported(vec![
        UnitType::MARINE.instance(&environment).set_hp(34).build(),
        UnitType::SNIPER.instance(&environment).set_hp(69).build(),
    ]).set_hp(89).build();

    let mut zipper = Zipper::new();
    convoy.zip(&mut zipper, false);
    let mut unzipper = Unzipper::new(zipper.finish(), Version::parse(VERSION).unwrap());
    let convoy2 = Unit::unzip(&mut unzipper, &environment, false);
    assert_eq!(Ok(convoy), convoy2);

    let exported = unchanged.export();
    let imported = Game::import_server(exported, &config, Version::parse(VERSION).unwrap()).unwrap();
    assert_eq!(imported, unchanged);
    assert_eq!(server.get_unit(Point::new(4, 0)), Some(&UnitType::CONVOY.instance(&environment).set_owner_id(0).set_transported(vec![
        UnitType::MARINE.instance(&environment).set_hp(34).build(),
        UnitType::SNIPER.instance(&environment).set_hp(69).build(),
    ]).set_hp(89).build()));
}

#[test]
fn lageos() {
    let config = Urc::new(Config::default());
    let map = PointMap::new(8, 5, false);
    let map = WMBuilder::<Direction4>::new(map);
    let mut map = Map::new(map.build(), &config);
    let map_env = map.environment().clone();
    map.set_unit(Point::new(0, 0), Some(UnitType::ATTACK_HELI.instance(&map_env).set_owner_id(1).set_hp(100).build()));
    map.set_unit(Point::new(0, 1), Some(UnitType::ATTACK_HELI.instance(&map_env).set_owner_id(1).set_hp(100).build()));
    map.set_unit(Point::new(1, 0), Some(UnitType::ATTACK_HELI.instance(&map_env).set_owner_id(0).set_hp(100).build()));
    map.set_unit(Point::new(1, 1), Some(UnitType::SMALL_TANK.instance(&map_env).set_owner_id(0).set_hp(100).build()));

    let map_settings = map.settings().unwrap();
    for player in &map_settings.players {
        assert!(player.get_commander_options().contains(&CommanderType::Lageos));
    }
    // get some default values without using Lageos
    let mut settings = map_settings.build_default();
    settings.fog_mode = FogMode::Constant(FogSetting::Sharp(0));
    let (mut server, _) = Game::new_server(map.clone(), &map_settings, settings, Urc::new(|| 0.));
    server.handle_command(Command::EndTurn, Urc::new(|| 0.)).unwrap();
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(Point::new(0, 0)),
        action: UnitAction::Attack(AttackInput::SplashPattern(OrientedPoint::simple(Point::new(1, 0), Direction4::D0))),
    }), Urc::new(|| 0.)).unwrap();
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(Point::new(0, 1)),
        action: UnitAction::Attack(AttackInput::SplashPattern(OrientedPoint::simple(Point::new(1, 1), Direction4::D0))),
    }), Urc::new(|| 0.)).unwrap();
    let damage_to_neutral_heli = 100 - server.get_unit(Point::new(1, 0)).unwrap().get_hp();
    let damage_to_neutral_tank = 100 - server.get_unit(Point::new(1, 1)).unwrap().get_hp();
    let neutral_vision_range = (0..map.width()).find(|x| server.get_fog_at(ClientPerspective::Team(0), Point::new(*x, 0)) == FogIntensity::Dark).unwrap();

    let mut settings = map_settings.build_default();
    settings.fog_mode = FogMode::Constant(FogSetting::Sharp(0));
    settings.players[0].set_commander(CommanderType::Lageos);
    let (mut server, _) = Game::new_server(map.clone(), &map_settings, settings, Urc::new(|| 0.));
    server.players.get_mut(0).unwrap().commander.charge = server.players.get_mut(0).unwrap().commander.get_max_charge();
    let unchanged = server.clone();

    // Lageos' air-units have have higher defense, Lageos has +1 vision
    server.handle_command(Command::EndTurn, Urc::new(|| 0.)).unwrap();
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(Point::new(0, 0)),
        action: UnitAction::Attack(AttackInput::SplashPattern(OrientedPoint::simple(Point::new(1, 0), Direction4::D0))),
    }), Urc::new(|| 0.)).unwrap();
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(Point::new(0, 1)),
        action: UnitAction::Attack(AttackInput::SplashPattern(OrientedPoint::simple(Point::new(1, 1), Direction4::D0))),
    }), Urc::new(|| 0.)).unwrap();
    let damage_to_lageos_heli = 100 - server.get_unit(Point::new(1, 0)).unwrap().get_hp();
    assert!(damage_to_neutral_heli > damage_to_lageos_heli, "{damage_to_neutral_heli} > {damage_to_lageos_heli}");
    assert_eq!(damage_to_neutral_tank, 100 - server.get_unit(Point::new(1, 1)).unwrap().get_hp());
    assert!(neutral_vision_range < (0..map.width()).find(|x| server.get_fog_at(ClientPerspective::Team(0), Point::new(*x, 0)) == FogIntensity::Dark).unwrap());

    // small power
    let mut server = unchanged.clone();
    server.handle_command(Command::commander_power(1, Vec::new()), Urc::new(|| 0.)).unwrap_err();
    server.handle_command(Command::commander_power(1, vec![
        CustomActionInput::Point(Point::new(0, 1)),
        CustomActionInput::Point(Point::new(0, 1)),
        CustomActionInput::Point(Point::new(2, 1)),
    ]), Urc::new(|| 0.)).unwrap();
    assert_eq!(server.get_unit(Point::new(1, 0)).unwrap().get_hp(), 100);
    assert!(server.get_unit(Point::new(0, 1)).unwrap().get_hp() < 100);
    assert!(server.get_unit(Point::new(0, 0)).unwrap().get_hp() < 100);
    assert_eq!(3 * (100 - server.get_unit(Point::new(0, 1)).unwrap().get_hp()), 2 * (100 - server.get_unit(Point::new(1, 1)).unwrap().get_hp()));
    assert!(!server.get_unit(Point::new(0, 1)).unwrap().has_flag(FLAG_EXHAUSTED));

    // big power
    let mut server = unchanged.clone();
    server.handle_command(Command::commander_power(2, Vec::new()), Urc::new(|| 0.)).unwrap_err();
    server.handle_command(Command::commander_power(2, vec![
        CustomActionInput::Point(Point::new(0, 1)),
        CustomActionInput::Point(Point::new(0, 1)),
    ]), Urc::new(|| 0.)).unwrap_err();
    server.handle_command(Command::commander_power(2, vec![
        CustomActionInput::Point(Point::new(0, 1)),
    ]), Urc::new(|| 0.)).unwrap();
    assert_eq!(server.get_unit(Point::new(1, 0)).unwrap().get_hp(), 100);
    assert!(server.get_unit(Point::new(0, 1)).unwrap().get_hp() < 100);
    assert!(server.get_unit(Point::new(1, 1)).unwrap().get_hp() < 100);
    assert!(server.get_unit(Point::new(0, 1)).unwrap().has_flag(FLAG_STUNNED));
    assert!(server.get_unit(Point::new(1, 1)).unwrap().has_flag(FLAG_EXHAUSTED));
    assert!(!server.get_unit(Point::new(1, 1)).unwrap().has_flag(FLAG_STUNNED));
    server.handle_command(Command::EndTurn, Urc::new(|| 0.)).unwrap();
    assert!(server.get_unit(Point::new(0, 1)).unwrap().has_flag(FLAG_EXHAUSTED));
}
