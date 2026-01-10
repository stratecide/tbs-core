use interfaces::ClientPerspective;
use uniform_smart_pointer::Urc;

use crate::combat::AttackInput;
use crate::config::config::Config;
use crate::game::commands::*;
use crate::game::fog::*;
use crate::game::game::Game;
use crate::map::direction::*;
use crate::map::map::get_neighbors;
use crate::map::map::Map;
use crate::map::point::Point;
use crate::map::point::Position;
use crate::map::point_map::PointMap;
use crate::map::wrapping_map::OrientedPoint;
use crate::map::wrapping_map::WMBuilder;
use crate::script::custom_action::test::CA_UNIT_CAPTURE;
use crate::script::custom_action::*;
use crate::script::custom_action::test::CA_UNIT_BUY_HERO;
use crate::tags::TagValue;
use crate::terrain::TerrainType;
use crate::units::commands::*;
use crate::units::hero::*;
use crate::units::movement::*;
use crate::tags::tests::*;
use crate::units::unit_types::UnitType;

impl HeroType {
    pub const CRYSTAL: Self = Self(0);
    pub const CRYSTAL_OBELISK: Self = Self(1);
    pub const EARL_GREY: Self = Self(2);
    pub const BLUEBERRY: Self = Self(3);
    pub const TESS: Self = Self(4);
    pub const EDWIN: Self = Self(5);
    pub const JAX: Self = Self(6);
    pub const KANE: Self = Self(7);
    pub const JULIA: Self = Self(8);
    pub const CASTOR: Self = Self(9);
    pub const POLLUX: Self = Self(10);
    pub const REED: Self = Self(11);
}

#[test]
fn buy_hero() {
    let config = Urc::new(Config::default());
    let map = PointMap::new(5, 5, false);
    let map = WMBuilder::<Direction4>::new(map);
    let mut map = Map::new(map.build(), &config);
    let map_env = map.environment().clone();
    map.set_terrain(Point::new(1, 1), TerrainType::StatueLand.instance(&map_env).set_owner_id(0).build());
    map.set_terrain(Point::new(0, 0), TerrainType::StatueLand.instance(&map_env).set_owner_id(0).build());
    map.set_unit(Point::new(0, 0), Some(UnitType::SMALL_TANK.instance(&map_env).set_owner_id(0).build()));
    map.set_unit(Point::new(0, 1), Some(UnitType::SMALL_TANK.instance(&map_env).set_owner_id(0).build()));
    map.set_unit(Point::new(4, 4), Some(UnitType::SMALL_TANK.instance(&map_env).set_owner_id(1).set_hero(Hero::new(HeroType::CRYSTAL_OBELISK)).build()));

    let mut game_config = map.settings().unwrap();
    game_config.fog_mode = FogMode::Constant(FogSetting::None);
    game_config.players[0].get_tag_bag_mut().set_tag(&map_env, TAG_FUNDS, 999999.into());
    let mut settings = game_config.build_default();
    settings.players[0].set_hero(HeroType::JAX);

    let (mut server, _) = Game::new_server(map.clone(), &game_config, settings, Urc::new(|| 0.));
    let board = Board::new(&server);
    assert_eq!(Hero::aura_range(&board, &server.get_unit(Point::new(0, 1)).unwrap(), Point::new(0, 1), None), None);
    let path = Path::with_steps(Point::new(0, 1), vec![PathStep::Dir(Direction4::D0)]);
    let options = server.get_unit(Point::new(0, 1)).unwrap().options_after_path(&board, &path, None, &[]);
    crate::debug!("options: {:?}", options);
    assert!(options.contains(&UnitAction::custom(CA_UNIT_BUY_HERO, Vec::new())));
    let script = config.custom_actions()[CA_UNIT_BUY_HERO].script.0.unwrap();
    let test_result = run_unit_input_script(script, &board, &path, None, &[]);
    crate::debug!("test_result: {:?}", test_result);
    match test_result {
        CustomActionTestResult::Next(CustomActionDataOptions::Shop(_, items)) => {
            assert_eq!(items.len(), 1);
            assert_eq!(items[0].key, ShopItemKey::HeroType(HeroType::JAX));
            assert!(items[0].enabled);
        }
        _ => panic!("should be CustomActionTestResult::Next")
    }
    // summon Jax
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path,
        action: UnitAction::custom(CA_UNIT_BUY_HERO, vec![CustomActionInput::ShopItem(0.into())]),
    }), Urc::new(|| 0.)).unwrap();
    let board = Board::new(&server);
    assert_eq!(server.get_unit(Point::new(1, 1)).unwrap().get_hero(), Some(&Hero::new(HeroType::JAX)));
    assert!(server.get_unit(Point::new(1, 1)).unwrap().has_flag(FLAG_EXHAUSTED));
    assert_eq!(Hero::aura_range(&board, &server.get_unit(Point::new(1, 1)).unwrap(), Point::new(1, 1), None), Some(2));
    // can't summon another Jax
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(Point::new(0, 0)),
        action: UnitAction::custom(CA_UNIT_BUY_HERO, vec![CustomActionInput::ShopItem(0.into())]),
    }), Urc::new(|| 0.)).unwrap_err();
}

#[test]
fn gain_charge() {
    let config = Urc::new(Config::default());
    let map = PointMap::new(5, 5, false);
    let map = WMBuilder::<Direction4>::new(map);
    let mut map = Map::new(map.build(), &config);
    let map_env = map.environment().clone();
    let jax = Hero::new(HeroType::CRYSTAL);
    map.set_unit(Point::new(0, 1), Some(UnitType::DRAGON_HEAD.instance(&map_env).set_owner_id(0).set_hero(jax).set_hp(100).build()));
    map.set_unit(Point::new(2, 1), Some(UnitType::SMALL_TANK.instance(&map_env).set_owner_id(1).set_hp(100).build()));

    let settings = map.settings().unwrap();
    let mut settings = settings.clone();
    settings.fog_mode = FogMode::Constant(FogSetting::None);

    let (mut server, _) = Game::new_server(map.clone(), &settings, settings.build_default(), Urc::new(|| 0.));
    assert_eq!(server.get_unit(Point::new(0, 1)).unwrap().get_charge(), 0);
    let path = Path::new(Point::new(0, 1));
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path,
        action: UnitAction::Attack(AttackInput::AttackPattern(Point::new(2, 1), Direction4::D0)),
    }), Urc::new(|| 0.)).unwrap();
    assert!(server.get_unit(Point::new(0, 1)).unwrap().get_charge() > 0);
}

#[test]
fn crystal() {
    let config = Urc::new(Config::default());
    let map = PointMap::new(5, 5, false);
    let map = WMBuilder::<Direction4>::new(map);
    let mut map = Map::new(map.build(), &config);
    let map_env = map.environment().clone();
    let mut crystal = Hero::new(HeroType::CRYSTAL);
    crystal.set_charge(&map_env, crystal.max_charge(&map_env));
    map.set_unit(Point::new(1, 1), Some(UnitType::SMALL_TANK.instance(&map_env).set_owner_id(0).set_hero(crystal).set_hp(1).build()));
    map.set_unit(Point::new(2, 1), Some(UnitType::SMALL_TANK.instance(&map_env).set_owner_id(0).set_hp(100).build()));
    map.set_unit(Point::new(3, 1), Some(UnitType::SMALL_TANK.instance(&map_env).set_owner_id(1).set_hp(100).build()));

    map.set_unit(Point::new(4, 4), Some(UnitType::SMALL_TANK.instance(&map_env).set_owner_id(0).set_hp(100).set_hero(Hero::new(HeroType::CRYSTAL_OBELISK)).build()));

    let settings = map.settings().unwrap();
    let mut settings = settings.clone();
    settings.fog_mode = FogMode::Constant(FogSetting::None);

    let (mut server, _) = Game::new_server(map.clone(), &settings, settings.build_default(), Urc::new(|| 0.));
    let unchanged = server.clone();
    let environment: crate::config::environment::Environment = server.environment().clone();
    let board = Board::new(&server);
    assert_eq!(Hero::aura_range(&board, &server.get_unit(Point::new(1, 1)).unwrap(), Point::new(1, 1), None), Some(3));
    assert_eq!(Hero::aura_range(&board, &server.get_unit(Point::new(4, 4)).unwrap(), Point::new(4, 4), None), Some(2));
    // use power
    let path = Path::new(Point::new(1, 1));
    let options = server.get_unit(Point::new(1, 1)).unwrap().options_after_path(&board, &path, None, &[]);
    crate::debug!("options: {:?}", options);
    assert!(options.contains(&UnitAction::hero_power(1, Vec::new())));
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path,
        action: UnitAction::hero_power(1, vec![CustomActionInput::Point(Point::new(0, 1))]),
    }), Urc::new(|| 0.)).unwrap();
    let board = Board::new(&server);
    assert_eq!(server.get_unit(Point::new(0, 1)), Some(&UnitType::HERO_CRYSTAL.instance(&environment).set_owner_id(0).set_hp(100).set_hero(Hero::new(HeroType::CRYSTAL_OBELISK)).build()));
    assert_eq!(Hero::aura_range(&board, &server.get_unit(Point::new(1, 1)).unwrap(), Point::new(1, 1), None), Some(3));
    assert_eq!(Hero::aura_range(&board, &server.get_unit(Point::new(4, 4)).unwrap(), Point::new(4, 4), None), Some(2));
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(Point::new(2, 1)),
        action: UnitAction::Attack(AttackInput::SplashPattern(OrientedPoint::simple(Point::new(3, 1), Direction4::D0))),
    }), Urc::new(|| 0.)).unwrap();
    let power_aura_damage = 100 - server.get_unit(Point::new(3, 1)).unwrap().get_hp();
    server.handle_command(Command::EndTurn, Urc::new(|| 0.)).unwrap();
    let board = Board::new(&server);
    assert_eq!(Hero::aura_range(&board, &server.get_unit(Point::new(1, 1)).unwrap(), Point::new(1, 1), None), Some(3));
    assert_eq!(Hero::aura_range(&board, &server.get_unit(Point::new(4, 4)).unwrap(), Point::new(4, 4), None), Some(2));
    server.handle_command(Command::EndTurn, Urc::new(|| 0.)).unwrap();
    let board = Board::new(&server);
    assert_eq!(Hero::aura_range(&board, &server.get_unit(Point::new(1, 1)).unwrap(), Point::new(1, 1), None), Some(3));
    assert_eq!(Hero::aura_range(&board, &server.get_unit(Point::new(4, 4)).unwrap(), Point::new(4, 4), None), Some(2));

    // don't use power
    let mut server = unchanged.clone();
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(Point::new(2, 1)),
        action: UnitAction::Attack(AttackInput::SplashPattern(OrientedPoint::simple(Point::new(3, 1), Direction4::D0))),
    }), Urc::new(|| 0.)).unwrap();
    let aura_damage = 100 - server.get_unit(Point::new(3, 1)).unwrap().get_hp();
    server.handle_command(Command::EndTurn, Urc::new(|| 0.)).unwrap();
    server.handle_command(Command::EndTurn, Urc::new(|| 0.)).unwrap();
    let board = Board::new(&server);
    assert_eq!(server.get_unit(Point::new(4, 4)).unwrap().get_hp(), 80);
    assert_eq!(Hero::hero_influence_at(&board, Point::new(0, 0), Some(0)).len(), 1);
    assert_eq!(Hero::hero_influence_at(&board, Point::new(0, 0), Some(1)).len(), 0);
    assert_eq!(Hero::hero_influence_at(&board, Point::new(0, 0), None).len(), 1);

    assert!(aura_damage < power_aura_damage, "{aura_damage} < {power_aura_damage}");

    // test crystal obelisk behavior when hero is missing
    map.set_unit(Point::new(1, 1), None);
    let (mut server, _) = Game::new_server(map.clone(), &settings, settings.build_default(), Urc::new(|| 0.));
    let crystal_damage = 100 - server.get_unit(Point::new(4, 4)).unwrap().get_hp();
    assert!(crystal_damage > 0);
    server.handle_command(Command::EndTurn, Urc::new(|| 0.)).unwrap();
    server.handle_command(Command::EndTurn, Urc::new(|| 0.)).unwrap();
    assert_eq!(server.get_unit(Point::new(4, 4)).unwrap().get_hp(), 100 - 2 * crystal_damage);
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(Point::new(2, 1)),
        action: UnitAction::Attack(AttackInput::SplashPattern(OrientedPoint::simple(Point::new(3, 1), Direction4::D0))),
    }), Urc::new(|| 0.)).unwrap();
    let normal_damage = 100 - server.get_unit(Point::new(3, 1)).unwrap().get_hp();

    assert!(normal_damage < aura_damage);
}

#[test]
fn earl_grey() {
    let config = Urc::new(Config::default());
    let map = PointMap::new(5, 5, false);
    let map = WMBuilder::<Direction4>::new(map);
    let mut map = Map::new(map.build(), &config);
    let map_env = map.environment().clone();
    let mut earl_grey = Hero::new(HeroType::EARL_GREY);
    earl_grey.set_charge(&map_env, earl_grey.max_charge(&map_env));
    map.set_unit(Point::new(1, 1), Some(UnitType::MARINE.instance(&map_env).set_owner_id(0).set_hero(earl_grey).set_hp(1).build()));
    map.set_unit(Point::new(2, 1), Some(UnitType::MARINE.instance(&map_env).set_owner_id(0).build()));
    map.set_unit(Point::new(3, 1), Some(UnitType::MARINE.instance(&map_env).set_owner_id(1).build()));

    map.set_unit(Point::new(4, 4), Some(UnitType::MARINE.instance(&map_env).set_owner_id(0).build()));

    let settings = map.settings().unwrap();
    let mut settings = settings.clone();
    settings.fog_mode = FogMode::Constant(FogSetting::None);

    let (mut server, _) = Game::new_server(map.clone(), &settings, settings.build_default(), Urc::new(|| 0.));
    let board = Board::new(&server);
    let heroes = HeroMap::new(&board, None);
    assert_eq!(
        server.get_unit(Point::new(2, 1)).unwrap().movement_points(&board, Point::new(2, 1), None, &heroes),
        server.get_unit(Point::new(4, 4)).unwrap().movement_points(&board, Point::new(4, 4), None, &heroes),
    );
    // hero power shouldn't be available if the hero moves
    let mut path = Path::new(Point::new(1, 1));
    path.steps.push(PathStep::Dir(Direction4::D90));
    let error = server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path,
        action: UnitAction::hero_power(1, Vec::new()),
    }), Urc::new(|| 0.)).unwrap_err();
    let board = Board::new(&server);
    assert_eq!(error, CommandError::InvalidAction);
    // use power
    let path = Path::new(Point::new(1, 1));
    let options = server.get_unit(Point::new(1, 1)).unwrap().options_after_path(&board, &path, None, &[]);
    crate::debug!("options: {:?}", options);
    assert!(options.contains(&UnitAction::hero_power(1, Vec::new())));
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path,
        action: UnitAction::hero_power(1, Vec::new()),
    }), Urc::new(|| 0.)).unwrap();
    let board = Board::new(&server);
    assert!(!server.get_unit(Point::new(1, 1)).unwrap().has_flag(FLAG_EXHAUSTED));
    let heroes = HeroMap::new(&board, None);
    assert!(
        server.get_unit(Point::new(2, 1)).unwrap().movement_points(&board, Point::new(2, 1), None, &heroes)
        >
        server.get_unit(Point::new(4, 4)).unwrap().movement_points(&board, Point::new(4, 4), None, &heroes)
    );
}

#[test]
fn blue_berry() {
    let config = Urc::new(Config::default());
    let map = PointMap::new(5, 5, false);
    let map = WMBuilder::<Direction4>::new(map);
    let mut map = Map::new(map.build(), &config);
    let map_env = map.environment().clone();
    let mut blue_berry = Hero::new(HeroType::BLUEBERRY);
    blue_berry.set_charge(&map_env, blue_berry.max_charge(&map_env));
    let hp = 50;
    map.set_unit(Point::new(1, 1), Some(UnitType::MARINE.instance(&map_env).set_owner_id(0).set_hero(blue_berry).set_hp(hp).build()));
    map.set_unit(Point::new(4, 1), Some(UnitType::MARINE.instance(&map_env).set_owner_id(0).set_hp(hp).build()));
    map.set_unit(Point::new(3, 1), Some(UnitType::MARINE.instance(&map_env).set_owner_id(1).set_hp(hp).build()));

    map.set_unit(Point::new(4, 4), Some(UnitType::MARINE.instance(&map_env).set_owner_id(0).set_hp(hp).build()));

    let settings = map.settings().unwrap();
    let (mut server, _) = Game::new_server(map.clone(), &settings, settings.build_default(), Urc::new(|| 0.));
    let board = Board::new(&server);
    assert_eq!(server.get_unit(Point::new(4, 1)).unwrap().get_hp(), hp);

    // test: power heals friendly units in aura
    let path = Path::new(Point::new(1, 1));
    let options = server.get_unit(Point::new(1, 1)).unwrap().options_after_path(&board, &path, None, &[]);
    crate::debug!("options: {:?}", options);
    assert!(options.contains(&UnitAction::hero_power(1, Vec::new())));
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path,
        action: UnitAction::hero_power(1, Vec::new()),
    }), Urc::new(|| 0.)).unwrap();
    assert!(server.get_unit(Point::new(4, 1)).unwrap().get_hp() > hp);
    assert_eq!(server.get_unit(Point::new(3, 1)).unwrap().get_hp(), hp);
    assert_eq!(server.get_unit(Point::new(4, 4)).unwrap().get_hp(), hp);
}

#[test]
fn tess() {
    let config = Urc::new(Config::default());
    let map = PointMap::new(5, 5, false);
    let map = WMBuilder::<Direction4>::new(map);
    let mut map = Map::new(map.build(), &config);
    let map_env = map.environment().clone();
    let mut tess = Hero::new(HeroType::TESS);
    tess.set_charge(&map_env, tess.max_charge(&map_env));
    map.set_unit(Point::new(1, 1), Some(UnitType::MARINE.instance(&map_env).set_owner_id(0).set_hero(tess).set_hp(100).build()));
    map.set_unit(Point::new(1, 2), Some(UnitType::MARINE.instance(&map_env).set_owner_id(1).set_hp(100).build()));

    let settings = map.settings().unwrap();
    let mut settings = settings.clone();
    settings.fog_mode = FogMode::Constant(FogSetting::None);
    settings.players[0].get_tag_bag_mut().set_tag(&map_env, TAG_FUNDS, 9999.into());
    settings.players[1].get_tag_bag_mut().set_tag(&map_env, TAG_FUNDS, 42.into());
    let (server_backup, _) = Game::new_server(map.clone(), &settings, settings.build_default(), Urc::new(|| 0.));

    // get money from attacking
    let mut server = server_backup.clone();
    let funds_before = match server.players[0].get_tag(TAG_FUNDS).unwrap() {
        TagValue::Int(i) => i.0,
        e => panic!("funds are {e:?}")
    };
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(Point::new(1, 1)),
        action: UnitAction::Attack(AttackInput::SplashPattern(OrientedPoint::simple(Point::new(1, 2), Direction4::D270))),
    }), Urc::new(|| 0.)).unwrap();
    let funds_after = match server.players[0].get_tag(TAG_FUNDS).unwrap() {
        TagValue::Int(i) => i.0,
        e => panic!("funds are {e:?}")
    };
    assert!(funds_before < funds_after, "{funds_before} < {funds_after}");

    // use power
    let mut server = server_backup.clone();
    let path = Path::new(Point::new(1, 1));
    let board = Board::new(&server);
    let options = server.get_unit(Point::new(1, 1)).unwrap().options_after_path(&board, &path, None, &[]);
    crate::debug!("options: {:?}", options);
    assert!(options.contains(&UnitAction::hero_power(1, Vec::new())));
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path,
        action: UnitAction::hero_power(1, vec![CustomActionInput::ShopItem(3.into()), CustomActionInput::Direction(Direction4::D0)]),
    }), Urc::new(|| 0.)).unwrap();
    assert!(!server.get_unit(Point::new(2, 1)).unwrap().has_flag(FLAG_EXHAUSTED));
    assert!(server.get_owning_player(0).unwrap().get_tag(TAG_FUNDS).unwrap().into_dynamic().cast::<i32>() < 9999);
}

#[test]
fn edwin() {
    let config = Urc::new(Config::default());
    let map = PointMap::new(5, 5, false);
    let map = WMBuilder::<Direction4>::new(map);
    let mut map = Map::new(map.build(), &config);
    let map_env = map.environment().clone();
    let mut edwin = Hero::new(HeroType::EDWIN);
    edwin.set_charge(&map_env, edwin.max_charge(&map_env));
    map.set_unit(Point::new(1, 1), Some(UnitType::MARINE.instance(&map_env).set_owner_id(0).set_hero(edwin).build()));
    let enemy = UnitType::MARINE.instance(&map_env).set_owner_id(1).build();
    map.set_unit(Point::new(2, 1), Some(enemy.clone()));
    let friend = UnitType::SMALL_TANK.instance(&map_env).set_owner_id(0).build();
    map.set_unit(Point::new(0, 4), Some(friend.clone()));
    map.set_unit(Point::new(2, 2), Some(enemy.clone()));

    let settings = map.settings().unwrap();
    let mut settings = settings.clone();
    settings.fog_mode = FogMode::Constant(FogSetting::None);

    let (server_backup, _) = Game::new_server(map.clone(), &settings, settings.build_default(), Urc::new(|| 0.));
    let mut server = server_backup.clone();
    // use power
    let path = Path::new(Point::new(1, 1));
    let board = Board::new(&server);
    let options = server.get_unit(Point::new(1, 1)).unwrap().options_after_path(&board, &path, None, &[]);
    crate::debug!("options: {:?}", options);
    assert!(options.contains(&UnitAction::hero_power(1, Vec::new())));
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path,
        action: UnitAction::hero_power(1, vec![CustomActionInput::Point(Point::new(2, 1)), CustomActionInput::Point(Point::new(0, 4))]),
    }), Urc::new(|| 0.)).unwrap();
    assert_eq!(server.get_unit(Point::new(2, 1)), Some(&friend));
    assert_eq!(server.get_unit(Point::new(0, 4)), Some(&enemy));

    // knockback with power
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(Point::new(2, 1)),
        action: UnitAction::Attack(AttackInput::SplashPattern(OrientedPoint::simple(Point::new(2, 2), Direction4::D270))),
    }), Urc::new(|| 0.)).unwrap();
    assert_eq!(server.get_unit(Point::new(2, 2)), None);
    assert!(server.get_unit(Point::new(2, 3)).is_some());

    /*// no knockback without power
    let mut server = server_backup.clone();
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(Point::new(1, 1)),
        action: UnitAction::Attack(AttackInput::SplashPattern(OrientedPoint::simple(Point::new(2, 1), Direction4::D0))),
    }), Urc::new(|| 0.)).unwrap();
    assert_eq!(server.get_unit(Point::new(3, 1)), None);
    assert!(server.get_unit(Point::new(2, 1)).is_some());*/
}

#[test]
fn jax() {
    let config = Urc::new(Config::default());
    let map = PointMap::new(5, 5, false);
    let map = WMBuilder::<Direction4>::new(map);
    let mut map = Map::new(map.build(), &config);
    let map_env = map.environment().clone();

    // hero
    let mut jax = Hero::new(HeroType::JAX);
    let jax_hp = 40;
    jax.set_charge(&map_env, jax.max_charge(&map_env));
    map.set_unit(Point::new(2, 0), Some(UnitType::SMALL_TANK.instance(&map_env).set_owner_id(0).set_hero(jax).set_hp(jax_hp).build()));

    // attacker boosted by Jax bonus for forest
    let attacker_pos_forest = Point::new(1, 0);
    map.set_terrain(attacker_pos_forest, TerrainType::Forest.instance(&map_env).build());
    map.set_unit(attacker_pos_forest, Some(UnitType::SMALL_TANK.instance(&map_env).set_owner_id(0).set_hp(50).build()));

    // attacker boosted only by Jax
    let attacker_pos_aura = Point::new(3, 0);
    map.set_unit(attacker_pos_aura, Some(UnitType::SMALL_TANK.instance(&map_env).set_owner_id(0).set_hp(50).build()));

    // attacker not boosted at all
    let attacker_pos_none = Point::new(3, 4);
    map.set_unit(attacker_pos_none, Some(UnitType::SMALL_TANK.instance(&map_env).set_owner_id(0).set_hp(50).build()));

    // jump target blocked by mountain
    let jump_mountain = Point::new(2, 1);
    map.set_terrain(jump_mountain, TerrainType::Mountain.instance(&map_env).build());

    // prepare side blocked by hidden unit
    let jump_blocked = Point::new(1, 2);
    map.set_terrain(jump_blocked, TerrainType::Forest.instance(&map_env).build());
    map.set_unit(Point::new(1, 1), Some(UnitType::SMALL_TANK.instance(&map_env).set_owner_id(1).set_hp(100).build()));
    map.set_unit(Point::new(0, 2), Some(UnitType::SMALL_TANK.instance(&map_env).set_owner_id(1).set_hp(100).build()));
    map.set_unit(jump_blocked, Some(UnitType::SMALL_TANK.instance(&map_env).set_owner_id(1).set_hp(100).build()));
    map.set_unit(Point::new(1, 3), Some(UnitType::SMALL_TANK.instance(&map_env).set_owner_id(1).set_hp(100).build()));

    // prepare side that isn't blocked
    let jump_possible = Point::new(3, 2);
    map.set_terrain(jump_possible, TerrainType::Forest.instance(&map_env).build());
    map.set_unit(Point::new(3, 1), Some(UnitType::SMALL_TANK.instance(&map_env).set_owner_id(1).set_hp(100).build()));
    map.set_unit(Point::new(2, 2), Some(UnitType::SMALL_TANK.instance(&map_env).set_owner_id(1).set_hp(100).build()));
    map.set_unit(Point::new(4, 2), Some(UnitType::SMALL_TANK.instance(&map_env).set_owner_id(1).set_hp(100).build()));
    map.set_unit(Point::new(3, 3), Some(UnitType::SMALL_TANK.instance(&map_env).set_owner_id(1).set_hp(100).build()));

    // start game
    let settings = map.settings().unwrap();
    let mut settings = settings.clone();
    settings.fog_mode = FogMode::Constant(FogSetting::Light(0));
    let (mut server_backup, _) = Game::new_server(map.clone(), &settings, settings.build_default(), Urc::new(|| 0.));
    server_backup.handle_command(Command::EndTurn, Urc::new(|| 0.)).unwrap();
    server_backup.handle_command(Command::EndTurn, Urc::new(|| 0.)).unwrap();
    let server_backup = server_backup;

    // test: ground units get bonus from Jax aura. Bonus if in forest
    let mut server = server_backup.clone();
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(attacker_pos_none),
        action: UnitAction::Attack(AttackInput::SplashPattern(OrientedPoint::simple(Point::new(3, 3), Direction4::D90))),
    }), Urc::new(|| 0.)).unwrap();
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(attacker_pos_aura),
        action: UnitAction::Attack(AttackInput::SplashPattern(OrientedPoint::simple(Point::new(3, 1), Direction4::D270))),
    }), Urc::new(|| 0.)).unwrap();
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(attacker_pos_forest),
        action: UnitAction::Attack(AttackInput::SplashPattern(OrientedPoint::simple(Point::new(1, 1), Direction4::D270))),
    }), Urc::new(|| 0.)).unwrap();
    let damage_none = 100 - server.get_unit(Point::new(3, 3)).unwrap().get_hp();
    let damage_aura = 100 - server.get_unit(Point::new(3, 1)).unwrap().get_hp();
    let damage_jax = 100 - server.get_unit(Point::new(1, 1)).unwrap().get_hp();
    assert!(damage_none > 0, "{}", damage_none);
    assert!(damage_aura > damage_none, "{} > {}", damage_aura, damage_none);
    assert!(damage_jax > damage_aura, "{} > {}", damage_jax, damage_aura);

    // test: can't jump to mountain
    let client = server_backup.reimport_as_client(interfaces::ClientPerspective::Team(0));
    let board = Board::from(&client);
    assert_eq!(board.get_unit(jump_blocked), None);
    let path = Path::new(Point::new(2, 0));
    let options = board.get_unit(Point::new(1, 1)).unwrap().options_after_path(&board, &path, None, &[]);
    assert!(options.contains(&UnitAction::hero_power(1, Vec::new())));
    let script = config.hero_powers(HeroType::JAX)[1].script.unwrap().0.unwrap();
    let test_result = run_unit_input_script(script, &board, &path, None, &[]);
    crate::debug!("test_result: {:?}", test_result);
    match test_result {
        CustomActionTestResult::Next(CustomActionDataOptions::Point(points)) => {
            assert!(points.contains(&jump_possible));
            assert!(points.contains(&jump_blocked));
            assert!(!points.contains(&jump_mountain));
        }
        _ => panic!("should be CustomActionTestResult::Next")
    }

    // test: jump is blocked by unseen unit
    let mut server = server_backup.clone();
    assert_ne!(server.get_fog_at(ClientPerspective::Team(0), jump_blocked), FogIntensity::TrueSight);
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: path.clone(),
        action: UnitAction::hero_power(1, vec![
            CustomActionInput::Point(jump_blocked),
            CustomActionInput::Direction(Direction4::D270),
        ]),
    }), Urc::new(|| 0.)).unwrap();
    assert_eq!(server.get_fog_at(ClientPerspective::Team(0), jump_blocked), FogIntensity::TrueSight);
    assert!(server.get_unit(path.start).unwrap().has_flag(FLAG_EXHAUSTED));
    let hero = server.get_unit(path.start).unwrap().get_hero().unwrap();
    assert_eq!(hero.charge, hero.max_charge(server.environment()));
    assert_eq!(server.get_unit(Point::new(2, 2)).unwrap().get_hp(), 100);

    // test: jump succeeds
    let mut server = server_backup.clone();
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: path.clone(),
        action: UnitAction::hero_power(1, vec![
            CustomActionInput::Point(jump_possible),
            CustomActionInput::Direction(Direction4::D270),
        ]),
    }), Urc::new(|| 0.)).unwrap();
    let board = Board::from(&server);
    assert!(server.get_unit(path.start).is_none());
    for p in get_neighbors(&board, jump_possible, crate::map::map::NeighborMode::Direct) {
        assert_eq!(server.get_unit(p.point).unwrap().get_hp() < 100, p.direction != Direction4::D90);
    }
    assert_eq!(server.get_unit(jump_possible).unwrap().get_hp(), jax_hp);
}

#[test]
fn kane() {
    let config = Urc::new(Config::default());
    let map = PointMap::new(5, 5, false);
    let map = WMBuilder::<Direction4>::new(map);
    let mut map = Map::new(map.build(), &config);
    let map_env = map.environment().clone();

    for p in valid_points(&map) {
        map.set_terrain(p, TerrainType::Sea.instance(&map_env).build());
    }

    // hero
    let mut kane = Hero::new(HeroType::KANE);
    kane.set_charge(&map_env, kane.max_charge(&map_env));
    let hero_pos = Point::new(0, 0);
    map.set_unit(hero_pos, Some(UnitType::WAVE_BREAKER.instance(&map_env).set_owner_id(0).set_hero(kane).set_hp(100).build()));

    // enemy unit
    map.set_unit(Point::new(2, 0), Some(UnitType::WAVE_BREAKER.instance(&map_env).set_owner_id(1).set_hp(100).build()));

    // start game
    let settings = map.settings().unwrap();
    let mut settings = settings.clone();
    settings.fog_mode = FogMode::Constant(FogSetting::Light(0));
    let (mut server, _) = Game::new_server(map.clone(), &settings, settings.build_default(), Urc::new(|| 0.));

    // test: kane should be invisible to the enemy
    let board = Board::from(&server);
    assert!(!can_see_unit_at(&board, ClientPerspective::Team(1), hero_pos, server.get_unit(hero_pos).unwrap(), true));

    // test: kane's power summons a LaserShark
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(hero_pos),
        action: UnitAction::hero_power(1, vec![
            CustomActionInput::Direction(Direction4::D0),
        ]),
    }), Urc::new(|| 0.)).unwrap();
    assert!(server.get_unit(Point::new(1, 0)).is_some());
}

#[test]
fn julia() {
    let config = Urc::new(Config::default());
    let map = PointMap::new(5, 5, false);
    let map = WMBuilder::<Direction4>::new(map);
    let mut map = Map::new(map.build(), &config);
    let map_env = map.environment().clone();

    // hero
    let mut hero = Hero::new(HeroType::JULIA);
    hero.set_charge(&map_env, hero.max_charge(&map_env));
    let hero_pos = Point::new(0, 0);
    map.set_unit(hero_pos, Some(UnitType::MARINE.instance(&map_env).set_owner_id(0).set_hero(hero).set_hp(70).build()));
    let capturer_pos = Point::new(1, 0);
    map.set_terrain(capturer_pos, TerrainType::City.instance(&map_env).build());
    map.set_unit(capturer_pos, Some(UnitType::MARINE.instance(&map_env).set_owner_id(0).set_hp(70).build()));

    // enemy unit
    map.set_unit(Point::new(4, 4), Some(UnitType::WAVE_BREAKER.instance(&map_env).set_owner_id(1).set_hp(100).build()));

    // start game
    let settings = map.settings().unwrap();
    let mut settings = settings.clone();
    settings.fog_mode = FogMode::Constant(FogSetting::Light(0));
    let (server_backup, _) = Game::new_server(map.clone(), &settings, settings.build_default(), Urc::new(|| 0.));

    // test: units capture better if in aura
    let mut server = server_backup.clone();
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(capturer_pos),
        action: UnitAction::custom(CA_UNIT_CAPTURE, Vec::new()),
    }), Urc::new(|| 0.)).unwrap();
    server.handle_command(Command::EndTurn, Urc::new(|| 0.)).unwrap();
    assert_eq!(server.get_terrain(capturer_pos).unwrap().get_owner_id(), -1);
    server.handle_command(Command::EndTurn, Urc::new(|| 0.)).unwrap();
    assert_eq!(server.get_terrain(capturer_pos).unwrap().get_owner_id(), 0);

    // test: units capture instantly during power
    let mut server = server_backup.clone();
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(hero_pos),
        action: UnitAction::hero_power(1, Vec::new()),
    }), Urc::new(|| 0.)).unwrap();
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(capturer_pos),
        action: UnitAction::custom(CA_UNIT_CAPTURE, Vec::new()),
    }), Urc::new(|| 0.)).unwrap();
    assert_eq!(server.get_terrain(capturer_pos).unwrap().get_owner_id(), 0);
}

#[test]
fn castor_pollux() {
    let config = Urc::new(Config::default());
    let map = PointMap::new(5, 5, false);
    let map = WMBuilder::<Direction4>::new(map);
    let mut map = Map::new(map.build(), &config);
    let map_env = map.environment().clone();

    // heroes
    let hero_hp = 70;
    let mut castor = Hero::new(HeroType::CASTOR);
    castor.set_charge(&map_env, castor.max_charge(&map_env));
    let castor_pos = Point::new(3, 2);
    map.set_unit(castor_pos, Some(UnitType::SMALL_TANK.instance(&map_env).set_owner_id(0).set_hero(castor).set_hp(hero_hp).build()));
    let mut pollux = Hero::new(HeroType::POLLUX);
    pollux.set_charge(&map_env, pollux.max_charge(&map_env));
    let pollux_pos = Point::new(1, 2);
    map.set_unit(pollux_pos, Some(UnitType::SMALL_TANK.instance(&map_env).set_owner_id(0).set_hero(pollux).set_hp(hero_hp).build()));

    // friendlies
    let aura_both_hp = 70;
    let aura_both = Point::new(2, 1);
    map.set_unit(aura_both, Some(UnitType::SMALL_TANK.instance(&map_env).set_owner_id(0).set_hp(aura_both_hp).build()));
    map.set_unit(Point::new(4, 2), Some(UnitType::SMALL_TANK.instance(&map_env).set_owner_id(0).set_hp(aura_both_hp).build()));

    // enemies
    let enemy_pos = Point::new(2, 2);
    let enemy_hp = 50;
    map.set_unit(enemy_pos, Some(UnitType::SMALL_TANK.instance(&map_env).set_owner_id(1).set_hp(enemy_hp).build()));

    // start game
    let settings = map.settings().unwrap();
    let mut settings = settings.clone();
    settings.fog_mode = FogMode::Constant(FogSetting::Light(0));
    let (server_backup, _) = Game::new_server(map.clone(), &settings, settings.build_default(), Urc::new(|| 0.));

    // test: castor damages enemies
    let mut server = server_backup.clone();
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(pollux_pos),
        action: UnitAction::hero_power(1, Vec::new()),
    }), Urc::new(|| 0.)).unwrap();
    assert!(server.get_unit(enemy_pos).unwrap().get_hp() < enemy_hp);

    // test: castor heals friendlies (but not self)
    let mut server = server_backup.clone();
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(castor_pos),
        action: UnitAction::hero_power(1, Vec::new()),
    }), Urc::new(|| 0.)).unwrap();
    assert!(server.get_unit(Point::new(4, 2)).unwrap().get_hp() > aura_both_hp);
    assert_eq!(server.get_unit(castor_pos).unwrap().get_hp(), hero_hp);
}

#[test]
fn reed() {
    let config = Urc::new(Config::default());
    let map = PointMap::new(5, 5, false);
    let map = WMBuilder::<Direction4>::new(map);
    let mut map = Map::new(map.build(), &config);
    let map_env = map.environment().clone();
    let mut reed = Hero::new(HeroType::REED);
    reed.set_charge(&map_env, reed.max_charge(&map_env));
    map.set_unit(Point::new(1, 1), Some(UnitType::MARINE.instance(&map_env).set_owner_id(0).set_hero(reed).set_hp(1).build()));
    map.set_unit(Point::new(2, 1), Some(UnitType::MARINE.instance(&map_env).set_owner_id(0).set_hp(50).set_flag(FLAG_EXHAUSTED).build()));
    map.set_unit(Point::new(3, 1), Some(UnitType::MARINE.instance(&map_env).set_owner_id(0).set_hp(50).set_flag(FLAG_EXHAUSTED).build()));

    map.set_unit(Point::new(4, 4), Some(UnitType::MARINE.instance(&map_env).set_owner_id(1).set_hp(50).set_flag(FLAG_EXHAUSTED).build()));

    let settings = map.settings().unwrap();
    let (mut server, _) = Game::new_server(map.clone(), &settings, settings.build_default(), Urc::new(|| 0.));

    // use power
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(Point::new(1, 1)),
        action: UnitAction::hero_power(1, Vec::new()),
    }), Urc::new(|| 0.)).unwrap();
    assert!(!server.get_unit(Point::new(2, 1)).unwrap().has_flag(FLAG_EXHAUSTED));
    assert!(server.get_unit(Point::new(3, 1)).unwrap().has_flag(FLAG_EXHAUSTED));
}
