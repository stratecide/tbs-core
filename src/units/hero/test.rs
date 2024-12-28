use std::sync::Arc;

use crate::config::config::Config;
use crate::game::commands::*;
use crate::game::fog::*;
use crate::game::game::Game;
use crate::game::game_view::GameView;
use crate::map::direction::*;
use crate::map::map::Map;
use crate::map::point::Point;
use crate::map::point::Position;
use crate::map::point_map::PointMap;
use crate::map::wrapping_map::WMBuilder;
use crate::script::custom_action::test::CA_UNIT_BUY_HERO;
use crate::script::custom_action::CustomActionInput;
use crate::tags::TagValue;
use crate::terrain::TerrainType;
use crate::units::combat::AttackVector;
use crate::units::commands::*;
use crate::units::hero::*;
use crate::units::movement::*;
use crate::tags::tests::*;
use crate::units::unit_types::UnitType;

#[allow(non_upper_case_globals)]
impl HeroType {
    pub const Crystal: Self = Self(0);
    pub const CrystalObelisk: Self = Self(1);
    pub const EarlGrey: Self = Self(2);
    pub const BlueBerry: Self = Self(3);
    pub const Tess: Self = Self(4);
    pub const Edwin: Self = Self(5);
}

#[test]
fn buy_hero() {
    let config = Arc::new(Config::test_config());
    let map = PointMap::new(5, 5, false);
    let map = WMBuilder::<Direction4>::new(map);
    let mut map = Map::new(map.build(), &config);
    let map_env = map.environment().clone();
    map.set_terrain(Point::new(1, 1), TerrainType::StatueLand.instance(&map_env).set_owner_id(0).build());
    map.set_unit(Point::new(1, 1), Some(UnitType::small_tank().instance(&map_env).set_owner_id(0).build()));
    map.set_unit(Point::new(4, 4), Some(UnitType::small_tank().instance(&map_env).set_owner_id(1).set_hero(Hero::new(HeroType::CrystalObelisk)).build()));

    let settings = map.settings().unwrap();
    let mut settings = settings.clone();
    settings.fog_mode = FogMode::Constant(FogSetting::None);
    settings.players[0].set_funds(999999);

    let (mut server, _) = Game::new_server(map.clone(), settings.build_default(), Arc::new(|| 0.));
    assert_eq!(Hero::aura_range(&*server, &server.get_unit(Point::new(1, 1)).unwrap(), Point::new(1, 1), None), None);
    let path = Path::new(Point::new(1, 1));
    let options = server.get_unit(Point::new(1, 1)).unwrap().options_after_path(&*server, &path, None, &[]);
    println!("options: {:?}", options);
    assert!(options.contains(&UnitAction::custom(CA_UNIT_BUY_HERO, Vec::new())));
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path,
        action: UnitAction::custom(CA_UNIT_BUY_HERO, vec![CustomActionInput::ShopItem(0.into())]),
    }), Arc::new(|| 0.)).unwrap();
    assert_eq!(server.get_unit(Point::new(1, 1)).unwrap().get_hero(), Some(&Hero::new(HeroType::Crystal)));
    assert_eq!(server.get_unit(Point::new(1, 1)).unwrap().get_tag(TAG_HERO_ORIGIN), Some(TagValue::Point(Point::new(1, 1))));
    assert!(server.get_unit(Point::new(1, 1)).unwrap().has_flag(FLAG_EXHAUSTED));
    assert_eq!(Hero::aura_range(&*server, &server.get_unit(Point::new(1, 1)).unwrap(), Point::new(1, 1), None), Some(2));
}


#[test]
fn crystal() {
    let config = Arc::new(Config::test_config());
    let map = PointMap::new(5, 5, false);
    let map = WMBuilder::<Direction4>::new(map);
    let mut map = Map::new(map.build(), &config);
    let map_env = map.environment().clone();
    let mut crystal = Hero::new(HeroType::Crystal);
    crystal.set_charge(&map_env, crystal.max_charge(&map_env));
    map.set_unit(Point::new(1, 1), Some(UnitType::small_tank().instance(&map_env).set_owner_id(0).set_hero(crystal).set_hp(1).build()));
    map.set_unit(Point::new(2, 1), Some(UnitType::small_tank().instance(&map_env).set_owner_id(0).set_hp(100).build()));
    map.set_unit(Point::new(3, 1), Some(UnitType::small_tank().instance(&map_env).set_owner_id(1).set_hp(100).build()));

    map.set_unit(Point::new(4, 4), Some(UnitType::small_tank().instance(&map_env).set_owner_id(0).set_hp(100).set_hero(Hero::new(HeroType::CrystalObelisk)).build()));

    let settings = map.settings().unwrap();
    let mut settings = settings.clone();
    settings.fog_mode = FogMode::Constant(FogSetting::None);

    let (mut server, _) = Game::new_server(map.clone(), settings.build_default(), Arc::new(|| 0.));
    let unchanged = server.clone();
    let environment: crate::config::environment::Environment = server.environment().clone();
    assert_eq!(Hero::aura_range(&*server, &server.get_unit(Point::new(1, 1)).unwrap(), Point::new(1, 1), None), Some(2));
    assert_eq!(Hero::aura_range(&*server, &server.get_unit(Point::new(4, 4)).unwrap(), Point::new(4, 4), None), Some(2));
    // use power
    let path = Path::new(Point::new(1, 1));
    let options = server.get_unit(Point::new(1, 1)).unwrap().options_after_path(&*server, &path, None, &[]);
    println!("options: {:?}", options);
    assert!(options.contains(&UnitAction::hero_power(1, Vec::new())));
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path,
        action: UnitAction::hero_power(1, vec![CustomActionInput::Point(Point::new(0, 1))]),
    }), Arc::new(|| 0.)).unwrap();
    assert_eq!(server.get_unit(Point::new(0, 1)), Some(UnitType::hero_crystal().instance(&environment).set_owner_id(0).set_hp(100).set_hero(Hero::new(HeroType::CrystalObelisk)).build()));
    assert_eq!(Hero::aura_range(&*server, &server.get_unit(Point::new(1, 1)).unwrap(), Point::new(1, 1), None), Some(3));
    assert_eq!(Hero::aura_range(&*server, &server.get_unit(Point::new(4, 4)).unwrap(), Point::new(4, 4), None), Some(3));
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(Point::new(2, 1)),
        action: UnitAction::Attack(AttackVector::Direction(Direction4::D0)),
    }), Arc::new(|| 0.)).unwrap();
    let power_aura_damage = 100 - server.get_unit(Point::new(3, 1)).unwrap().get_hp();
    server.handle_command(Command::EndTurn, Arc::new(|| 0.)).unwrap();
    assert_eq!(Hero::aura_range(&*server, &server.get_unit(Point::new(1, 1)).unwrap(), Point::new(1, 1), None), Some(3));
    assert_eq!(Hero::aura_range(&*server, &server.get_unit(Point::new(4, 4)).unwrap(), Point::new(4, 4), None), Some(3));
    server.handle_command(Command::EndTurn, Arc::new(|| 0.)).unwrap();
    assert_eq!(Hero::aura_range(&*server, &server.get_unit(Point::new(1, 1)).unwrap(), Point::new(1, 1), None), Some(2));
    assert_eq!(Hero::aura_range(&*server, &server.get_unit(Point::new(4, 4)).unwrap(), Point::new(4, 4), None), Some(2));

    // don't use power
    let mut server = unchanged.clone();
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(Point::new(2, 1)),
        action: UnitAction::Attack(AttackVector::Direction(Direction4::D0)),
    }), Arc::new(|| 0.)).unwrap();
    let aura_damage = 100 - server.get_unit(Point::new(3, 1)).unwrap().get_hp();
    server.handle_command(Command::EndTurn, Arc::new(|| 0.)).unwrap();
    server.handle_command(Command::EndTurn, Arc::new(|| 0.)).unwrap();
    assert_eq!(server.get_unit(Point::new(4, 4)).unwrap().get_hp(), 100);
    assert_eq!(Hero::hero_influence_at(&*server, Point::new(0, 0), 0).len(), 1);
    assert_eq!(Hero::hero_influence_at(&*server, Point::new(0, 0), 1).len(), 0);
    assert_eq!(Hero::hero_influence_at(&*server, Point::new(0, 0), -1).len(), 1);

    assert!(aura_damage < power_aura_damage, "{aura_damage} < {power_aura_damage}");

    // test crystal obelisk behavior when hero is missing
    map.set_unit(Point::new(1, 1), None);
    let (mut server, _) = Game::new_server(map.clone(), settings.build_default(), Arc::new(|| 0.));
    let crystal_damage = 100 - server.get_unit(Point::new(4, 4)).unwrap().get_hp();
    assert!(crystal_damage > 0);
    server.handle_command(Command::EndTurn, Arc::new(|| 0.)).unwrap();
    server.handle_command(Command::EndTurn, Arc::new(|| 0.)).unwrap();
    assert_eq!(server.get_unit(Point::new(4, 4)).unwrap().get_hp(), 100 - 2 * crystal_damage);
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(Point::new(2, 1)),
        action: UnitAction::Attack(AttackVector::Direction(Direction4::D0)),
    }), Arc::new(|| 0.)).unwrap();
    let normal_damage = 100 - server.get_unit(Point::new(3, 1)).unwrap().get_hp();

    assert!(normal_damage < aura_damage);
}

#[test]
fn earl_grey() {
    let config = Arc::new(Config::test_config());
    let map = PointMap::new(5, 5, false);
    let map = WMBuilder::<Direction4>::new(map);
    let mut map = Map::new(map.build(), &config);
    let map_env = map.environment().clone();
    let mut earl_grey = Hero::new(HeroType::EarlGrey);
    earl_grey.set_charge(&map_env, earl_grey.max_charge(&map_env));
    map.set_unit(Point::new(1, 1), Some(UnitType::marine().instance(&map_env).set_owner_id(0).set_hero(earl_grey).set_hp(1).build()));
    map.set_unit(Point::new(2, 1), Some(UnitType::marine().instance(&map_env).set_owner_id(0).build()));
    map.set_unit(Point::new(3, 1), Some(UnitType::marine().instance(&map_env).set_owner_id(1).build()));

    map.set_unit(Point::new(4, 4), Some(UnitType::marine().instance(&map_env).set_owner_id(0).build()));

    let settings = map.settings().unwrap();
    let mut settings = settings.clone();
    settings.fog_mode = FogMode::Constant(FogSetting::None);

    let (mut server, _) = Game::new_server(map.clone(), settings.build_default(), Arc::new(|| 0.));
    let influence1 = Hero::hero_influence_at(&*server, Point::new(2, 1), 0);
    let influence2 = Hero::hero_influence_at(&*server, Point::new(4, 4), 0);
    assert_eq!(
        server.get_unit(Point::new(2, 1)).unwrap().movement_points(&*server, Point::new(2, 1), None, &influence1),
        server.get_unit(Point::new(4, 4)).unwrap().movement_points(&*server, Point::new(4, 4), None, &influence2),
    );
    // hero power shouldn't be available if the hero moves
    let mut path = Path::new(Point::new(1, 1));
    path.steps.push(PathStep::Dir(Direction4::D90));
    let error = server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path,
        action: UnitAction::hero_power(1, Vec::new()),
    }), Arc::new(|| 0.)).unwrap_err();
    assert_eq!(error, CommandError::InvalidAction);
    // use power
    let path = Path::new(Point::new(1, 1));
    let options = server.get_unit(Point::new(1, 1)).unwrap().options_after_path(&*server, &path, None, &[]);
    println!("options: {:?}", options);
    assert!(options.contains(&UnitAction::hero_power(1, Vec::new())));
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path,
        action: UnitAction::hero_power(1, Vec::new()),
    }), Arc::new(|| 0.)).unwrap();
    assert!(!server.get_unit(Point::new(1, 1)).unwrap().has_flag(FLAG_EXHAUSTED));
    let influence1 = Hero::hero_influence_at(&*server, Point::new(2, 1), 0);
    let influence2 = Hero::hero_influence_at(&*server, Point::new(4, 4), 0);
    assert!(
        server.get_unit(Point::new(2, 1)).unwrap().movement_points(&*server, Point::new(2, 1), None, &influence1)
        >
        server.get_unit(Point::new(4, 4)).unwrap().movement_points(&*server, Point::new(4, 4), None, &influence2)
    );
}

#[test]
fn blue_berry() {
    let config = Arc::new(Config::test_config());
    let map = PointMap::new(5, 5, false);
    let map = WMBuilder::<Direction4>::new(map);
    let mut map = Map::new(map.build(), &config);
    let map_env = map.environment().clone();
    let mut blue_berry = Hero::new(HeroType::BlueBerry);
    blue_berry.set_charge(&map_env, blue_berry.max_charge(&map_env));
    map.set_unit(Point::new(1, 1), Some(UnitType::marine().instance(&map_env).set_owner_id(0).set_hero(blue_berry).set_hp(1).build()));
    map.set_unit(Point::new(2, 1), Some(UnitType::marine().instance(&map_env).set_owner_id(0).set_hp(50).set_flag(FLAG_EXHAUSTED).build()));
    map.set_unit(Point::new(3, 1), Some(UnitType::marine().instance(&map_env).set_owner_id(1).build()));

    map.set_unit(Point::new(4, 4), Some(UnitType::marine().instance(&map_env).set_owner_id(0).set_hp(50).set_flag(FLAG_EXHAUSTED).build()));

    let settings = map.settings().unwrap();
    let mut settings = settings.clone();
    settings.fog_mode = FogMode::Constant(FogSetting::None);

    let (mut server, _) = Game::new_server(map.clone(), settings.build_default(), Arc::new(|| 0.));
    assert!(server.get_unit(Point::new(2, 1)).unwrap().get_hp() > 50);
    assert_eq!(server.get_unit(Point::new(4, 4)).unwrap().get_hp(), 50);
    assert!(server.get_unit(Point::new(4, 4)).unwrap().has_flag(FLAG_EXHAUSTED));
    // use power
    let path = Path::new(Point::new(1, 1));
    let options = server.get_unit(Point::new(1, 1)).unwrap().options_after_path(&*server, &path, None, &[]);
    println!("options: {:?}", options);
    assert!(options.contains(&UnitAction::hero_power(1, Vec::new())));
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path,
        action: UnitAction::hero_power(1, Vec::new()),
    }), Arc::new(|| 0.)).unwrap();
    assert!(!server.get_unit(Point::new(2, 1)).unwrap().has_flag(FLAG_EXHAUSTED));
    assert!(server.get_unit(Point::new(4, 4)).unwrap().has_flag(FLAG_EXHAUSTED));
}

#[test]
fn tess() {
    let config = Arc::new(Config::test_config());
    let map = PointMap::new(5, 5, false);
    let map = WMBuilder::<Direction4>::new(map);
    let mut map = Map::new(map.build(), &config);
    let map_env = map.environment().clone();
    let mut tess = Hero::new(HeroType::Tess);
    tess.set_charge(&map_env, tess.max_charge(&map_env));
    map.set_unit(Point::new(1, 1), Some(UnitType::marine().instance(&map_env).set_owner_id(0).set_hero(tess).set_hp(1).build()));
    map.set_unit(Point::new(3, 1), Some(UnitType::marine().instance(&map_env).set_owner_id(1).build()));

    let settings = map.settings().unwrap();
    let mut settings = settings.clone();
    settings.fog_mode = FogMode::Constant(FogSetting::None);
    settings.players[0].set_funds(9999);

    let (mut server, _) = Game::new_server(map.clone(), settings.build_default(), Arc::new(|| 0.));
    // use power
    let path = Path::new(Point::new(1, 1));
    let options = server.get_unit(Point::new(1, 1)).unwrap().options_after_path(&*server, &path, None, &[]);
    println!("options: {:?}", options);
    assert!(options.contains(&UnitAction::hero_power(1, Vec::new())));
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path,
        action: UnitAction::hero_power(1, vec![CustomActionInput::ShopItem(3.into()), CustomActionInput::Direction(Direction4::D0)]),
    }), Arc::new(|| 0.)).unwrap();
    assert!(!server.get_unit(Point::new(2, 1)).unwrap().has_flag(FLAG_EXHAUSTED));
    assert!(*server.get_owning_player(0).unwrap().funds < 9999);
}

#[test]
fn edwin() {
    let config = Arc::new(Config::test_config());
    let map = PointMap::new(5, 5, false);
    let map = WMBuilder::<Direction4>::new(map);
    let mut map = Map::new(map.build(), &config);
    let map_env = map.environment().clone();
    let mut edwin = Hero::new(HeroType::Edwin);
    edwin.set_charge(&map_env, edwin.max_charge(&map_env));
    map.set_unit(Point::new(1, 1), Some(UnitType::marine().instance(&map_env).set_owner_id(0).set_hero(edwin).build()));
    let enemy = UnitType::marine().instance(&map_env).set_owner_id(1).build();
    map.set_unit(Point::new(2, 1), Some(enemy.clone()));
    let friend = UnitType::small_tank().instance(&map_env).set_owner_id(0).build();
    map.set_unit(Point::new(0, 4), Some(friend.clone()));
    map.set_unit(Point::new(2, 2), Some(enemy.clone()));

    let settings = map.settings().unwrap();
    let mut settings = settings.clone();
    settings.fog_mode = FogMode::Constant(FogSetting::None);

    let (mut server, _) = Game::new_server(map.clone(), settings.build_default(), Arc::new(|| 0.));
    let unchanged = server.clone();
    // use power
    let path = Path::new(Point::new(1, 1));
    let options = server.get_unit(Point::new(1, 1)).unwrap().options_after_path(&*server, &path, None, &[]);
    println!("options: {:?}", options);
    assert!(options.contains(&UnitAction::hero_power(1, Vec::new())));
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path,
        action: UnitAction::hero_power(1, vec![CustomActionInput::Point(Point::new(2, 1)), CustomActionInput::Point(Point::new(0, 4))]),
    }), Arc::new(|| 0.)).unwrap();
    assert_eq!(server.get_unit(Point::new(2, 1)), Some(friend));
    assert_eq!(server.get_unit(Point::new(0, 4)), Some(enemy));

    // knockback with power
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(Point::new(2, 1)),
        action: UnitAction::Attack(AttackVector::Direction(Direction4::D270)),
    }), Arc::new(|| 0.)).unwrap();
    assert_eq!(server.get_unit(Point::new(2, 2)), None);
    assert!(server.get_unit(Point::new(2, 3)).is_some());

    // no knockback without power
    let mut server = unchanged.clone();
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(Point::new(1, 1)),
        action: UnitAction::Attack(AttackVector::Direction(Direction4::D0)),
    }), Arc::new(|| 0.)).unwrap();
    assert_eq!(server.get_unit(Point::new(3, 1)), None);
    assert!(server.get_unit(Point::new(2, 1)).is_some());
}
