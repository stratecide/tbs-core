use std::sync::Arc;

use interfaces::*;
use semver::Version;
use zipper::Unzipper;
use zipper::Zipper;
use crate::commander::commander_type::CommanderType;
use crate::config::config::Config;
use crate::details::Detail;
use crate::details::SkullData;
use crate::details::SludgeToken;
use crate::game::commands::Command;
use crate::game::commands::CommandError;
use crate::game::fog::*;
use crate::game::game::Game;
use crate::game::game_view::GameView;
use crate::map::direction::*;
use crate::map::map::Map;
use crate::map::map_view::MapView;
use crate::map::point::Point;
use crate::map::point::Position;
use crate::map::point_map::PointMap;
use crate::map::wrapping_map::WMBuilder;
use crate::script::custom_action::CustomActionData;
use crate::terrain::TerrainType;
use crate::units::attributes::ActionStatus;
use crate::units::attributes::Amphibious;
use crate::units::attributes::AttributeKey;
use crate::units::combat::AttackVector;
use crate::units::commands::UnitAction;
use crate::units::commands::UnitCommand;
use crate::units::movement::Path;
use crate::units::unit::Unit;
use crate::units::unit_types::UnitType;
use crate::VERSION;

#[test]
fn zombie() {
    let config = Arc::new(Config::test_config());
    let map = PointMap::new(5, 5, false);
    let map = WMBuilder::<Direction6>::new(map);
    let mut map = Map::new(map.build(), &config);
    let map_env = map.environment().clone();
    map.set_unit(Point::new(1, 1), Some(UnitType::SmallTank.instance(&map_env).set_owner_id(0).build_with_defaults()));
    map.set_unit(Point::new(2, 1), Some(UnitType::SmallTank.instance(&map_env).set_owner_id(1).set_hp(1).build_with_defaults()));

    map.set_unit(Point::new(4, 4), Some(UnitType::SmallTank.instance(&map_env).set_owner_id(1).set_hp(1).build_with_defaults()));

    let skull = SkullData::new(&UnitType::Marine.instance(&map_env).set_owner_id(1).set_amphibious(Amphibious::InWater).build_with_defaults(), 0);
    map.set_details(Point::new(0, 4), vec![Detail::Skull(skull.clone())]);
    let skull2 = SkullData::new(&UnitType::Marine.instance(&map_env).set_owner_id(1).set_amphibious(Amphibious::OnLand).build_with_defaults(), 0);
    map.set_details(Point::new(1, 4), vec![Detail::Skull(skull2.clone())]);

    let settings = map.settings().unwrap();

    let mut settings = settings.clone();
    for player in &settings.players {
        assert!(player.get_commander_options().contains(&CommanderType::Zombie));
    }
    settings.fog_mode = FogMode::Constant(FogSetting::None);
    settings.players[0].set_commander(CommanderType::Zombie);
    let (mut server, _) = Game::new_server(map.clone(), &settings, Box::new(|| 0.));
    server.players.get_mut(0).unwrap().commander.charge = server.players.get_mut(0).unwrap().commander.get_max_charge();
    let unchanged = server.clone();
    let environment = server.environment().clone();
    // small power
    server.handle_command(Command::commander_power(1, Vec::new()), Box::new(|| 0.)).unwrap();
    assert_eq!(server.get_map().get_details(Point::new(0, 4)), Vec::new());
    assert_eq!(server.get_map().get_unit(Point::new(0, 4)), Some(&UnitType::Marine.instance(&environment).set_owner_id(0).set_hp(50).set_zombified(true).set_amphibious(Amphibious::InWater).build_with_defaults()));
    assert_eq!(server.get_map().get_unit(Point::new(1, 4)), Some(&UnitType::Marine.instance(&environment).set_owner_id(0).set_hp(50).set_zombified(true).set_amphibious(Amphibious::OnLand).build_with_defaults()));
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(Point::new(1, 1)),
        action: UnitAction::Attack(AttackVector::Direction(Direction6::D0)),
    }), Box::new(|| 0.)).unwrap();
    assert_eq!(server.get_map().get_details(Point::new(2, 1)), vec![Detail::Skull(SkullData::new(&UnitType::SmallTank.instance(&map_env).set_owner_id(1).build_with_defaults(), 0))]);
    assert_eq!(server.get_map().get_unit(Point::new(2, 1)), None);
    // big power
    let mut server = unchanged.clone();
    server.handle_command(Command::commander_power(2, Vec::new()), Box::new(|| 0.)).unwrap();
    assert_eq!(server.get_map().get_details(Point::new(0, 4)), Vec::new());
    assert_eq!(server.get_map().get_unit(Point::new(0, 4)), Some(&UnitType::Marine.instance(&environment).set_owner_id(0).set_hp(50).set_zombified(true).set_amphibious(Amphibious::InWater).build_with_defaults()));
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(Point::new(1, 1)),
        action: UnitAction::Attack(AttackVector::Direction(Direction6::D0)),
    }), Box::new(|| 0.)).unwrap();
    assert_eq!(server.get_map().get_details(Point::new(2, 1)), Vec::new());
    assert_eq!(server.get_map().get_unit(Point::new(2, 1)), Some(&UnitType::SmallTank.instance(&environment).set_owner_id(0).set_hp(50).set_zombified(true).build_with_defaults()));
}

#[test]
fn simo() {
    let config = Arc::new(Config::test_config());
    let map = PointMap::new(6, 5, false);
    let map = WMBuilder::<Direction4>::new(map);
    let mut map = Map::new(map.build(), &config);
    let map_env = map.environment().clone();
    let arty_pos = Point::new(0, 1);
    map.set_unit(arty_pos, Some(UnitType::Artillery.instance(&map_env).set_owner_id(0).set_hp(50).build_with_defaults()));

    let target_close = Point::new(3, 1);
    map.set_unit(target_close, Some(UnitType::SmallTank.instance(&map_env).set_owner_id(1).build_with_defaults()));
    let target_far = Point::new(4, 1);
    map.set_unit(target_far, Some(UnitType::SmallTank.instance(&map_env).set_owner_id(1).build_with_defaults()));
    let target_farthest = Point::new(5, 1);
    map.set_unit(target_farthest, Some(UnitType::SmallTank.instance(&map_env).set_owner_id(1).build_with_defaults()));

    let settings = map.settings().unwrap();

    let mut settings = settings.clone();
    for player in &settings.players {
        assert!(player.get_commander_options().contains(&CommanderType::Simo));
    }
    settings.fog_mode = FogMode::Constant(FogSetting::None);
    settings.players[0].set_commander(CommanderType::Simo);
    let (mut server, _) = Game::new_server(map.clone(), &settings, Box::new(|| 0.));
    server.players.get_mut(0).unwrap().commander.charge = server.players.get_mut(0).unwrap().commander.get_max_charge();
    let unchanged = server.clone();

    // before chaos/order
    let arty = server.get_unit(arty_pos).unwrap();
    assert!(arty.shortest_path_to_attack(&server, &Path::new(arty_pos), None, target_close).is_some());
    assert!(arty.shortest_path_to_attack(&server, &Path::new(arty_pos), None, target_far).is_none());
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(arty_pos),
        action: UnitAction::Attack(AttackVector::Point(target_close)),
    }), Box::new(|| 0.)).unwrap();
    assert!(server.get_unit(target_close).unwrap().get_hp() < 100);
    assert_eq!(server.get_unit(target_far).unwrap().get_hp(), 100);

    // embrace chaos
    let mut server = unchanged.clone();
    server.handle_command(Command::commander_power(1, Vec::new()), Box::new(|| 0.)).unwrap();
    server.handle_command(Command::commander_power(4, Vec::new()), Box::new(|| 0.)).err().unwrap();
    server.handle_command(Command::commander_power(5, Vec::new()), Box::new(|| 0.)).err().unwrap();
    let arty = server.get_unit(arty_pos).unwrap();
    assert!(arty.shortest_path_to_attack(&server, &Path::new(arty_pos), None, target_close).is_some());
    assert!(arty.shortest_path_to_attack(&server, &Path::new(arty_pos), None, target_far).is_none());
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(arty_pos),
        action: UnitAction::Attack(AttackVector::Point(target_close)),
    }), Box::new(|| 0.)).unwrap();
    let hp_close = server.get_unit(target_close).unwrap().get_hp();
    let hp_far = server.get_unit(target_far).unwrap().get_hp();
    assert!(hp_far < 100);
    assert!(hp_close < hp_far);

    // chaos power
    let mut server = unchanged.clone();
    server.handle_command(Command::commander_power(1, Vec::new()), Box::new(|| 0.)).unwrap();
    server.handle_command(Command::commander_power(3, Vec::new()), Box::new(|| 0.)).unwrap();
    //let arty = server.get_unit(arty_pos).unwrap();
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(arty_pos),
        action: UnitAction::Attack(AttackVector::Point(target_far)),
    }), Box::new(|| 0.)).err().expect("range shouldn't be increased");
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(arty_pos),
        action: UnitAction::Attack(AttackVector::Point(target_close)),
    }), Box::new(|| 0.)).unwrap();
    assert!(server.get_unit(target_close).unwrap().get_hp() < hp_close);
    assert!(server.get_unit(target_far).unwrap().get_hp() < hp_far);
    assert_eq!(server.get_unit(target_farthest).unwrap().get_hp(), 100);

    // order power (small)
    let mut server = unchanged.clone();
    server.handle_command(Command::commander_power(2, Vec::new()), Box::new(|| 0.)).unwrap();
    server.handle_command(Command::commander_power(3, Vec::new()), Box::new(|| 0.)).err().unwrap();
    server.handle_command(Command::commander_power(4, Vec::new()), Box::new(|| 0.)).unwrap();
    let arty = server.get_unit(arty_pos).unwrap();
    assert!(arty.shortest_path_to_attack(&server, &Path::new(arty_pos), None, target_far).is_some());
    assert!(arty.shortest_path_to_attack(&server, &Path::new(arty_pos), None, target_farthest).is_none());
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(arty_pos),
        action: UnitAction::Attack(AttackVector::Point(target_far)),
    }), Box::new(|| 0.)).unwrap();
    assert_eq!(server.get_unit(target_close).unwrap().get_hp(), 100);
    assert!(server.get_unit(target_far).unwrap().get_hp() < 100);

    // order power (big)
    let mut server = unchanged.clone();
    server.handle_command(Command::commander_power(2, Vec::new()), Box::new(|| 0.)).unwrap();
    server.handle_command(Command::commander_power(5, Vec::new()), Box::new(|| 0.)).unwrap();
    let arty = server.get_unit(arty_pos).unwrap();
    assert!(arty.shortest_path_to_attack(&server, &Path::new(arty_pos), None, target_close).is_some());
    assert!(arty.shortest_path_to_attack(&server, &Path::new(arty_pos), None, target_far).is_some());
    assert!(arty.shortest_path_to_attack(&server, &Path::new(arty_pos), None, target_farthest).is_some());
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(arty_pos),
        action: UnitAction::Attack(AttackVector::Point(target_farthest)),
    }), Box::new(|| 0.)).unwrap();
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
    map.set_unit(arty_pos, Some(UnitType::Artillery.instance(&map_env).set_owner_id(0).set_hp(50).build_with_defaults()));

    let target_close = Point::new(3, 1);
    map.set_unit(target_close, Some(UnitType::SmallTank.instance(&map_env).set_owner_id(1).set_hp(50).build_with_defaults()));
    map.set_terrain(target_close, TerrainType::Flame.instance(&map_env).build_with_defaults());
    let target_far = Point::new(5, 4);
    map.set_unit(target_far, Some(UnitType::SmallTank.instance(&map_env).set_owner_id(1).set_hp(50).build_with_defaults()));

    let mut settings = map.settings().unwrap();
    for player in &settings.players {
        assert!(player.get_commander_options().contains(&CommanderType::Vlad));
    }
    settings.fog_mode = FogMode::Constant(FogSetting::None);
    settings.players[0].set_commander(CommanderType::Vlad);
    let (mut server, _) = Game::new_server(map.clone(), &settings, Box::new(|| 0.));

    // d2d daylight
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(arty_pos),
        action: UnitAction::Attack(AttackVector::Point(target_close)),
    }), Box::new(|| 0.)).unwrap();
    assert_eq!(server.get_unit(arty_pos).unwrap().get_hp(), 50);

    // d2d night
    settings.fog_mode = FogMode::Constant(FogSetting::Sharp(0));
    let (mut server, _) = Game::new_server(map.clone(), &settings, Box::new(|| 0.));
    server.players.get_mut(0).unwrap().commander.charge = server.players.get_mut(0).unwrap().commander.get_max_charge();
    let unchanged = server.clone();
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(arty_pos),
        action: UnitAction::Attack(AttackVector::Point(target_close)),
    }), Box::new(|| 0.)).unwrap();
    assert!(server.get_unit(arty_pos).unwrap().get_hp() > 50);

    // small power
    let mut server = unchanged.clone();
    server.handle_command(Command::commander_power(1, Vec::new()), Box::new(|| 0.)).unwrap();
    assert_eq!(server.get_unit(arty_pos).unwrap().get_hp(), 50);
    assert!(server.get_unit(target_close).unwrap().get_hp() < 50);
    assert_eq!(server.get_unit(target_far).unwrap().get_hp(), 50);

    // big power
    let mut server = unchanged.clone();
    server.handle_command(Command::commander_power(2, Vec::new()), Box::new(|| 0.)).unwrap();
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
        map.set_terrain(p, TerrainType::Street.instance(&map_env).build_with_defaults());
    }
    map.set_terrain(Point::new(0, 1), TerrainType::Grass.instance(&map_env).build_with_defaults());
    map.set_terrain(Point::new(0, 2), TerrainType::Forest.instance(&map_env).build_with_defaults());
    map.set_terrain(Point::new(0, 3), TerrainType::FairyForest.instance(&map_env).set_owner_id(0).build_with_defaults());
    for i in 0..4 {
        map.set_unit(Point::new(0, i), Some(UnitType::SmallTank.instance(&map_env).set_owner_id(0).build_with_defaults()));
        map.set_unit(Point::new(1, i), Some(UnitType::SmallTank.instance(&map_env).set_owner_id(1).build_with_defaults()));
        map.set_terrain(Point::new(2, i), TerrainType::Grass.instance(&map_env).build_with_defaults());
    }

    map.set_terrain(Point::new(5, 3), TerrainType::FairyForest.instance(&map_env).set_owner_id(0).build_with_defaults());
    map.set_terrain(Point::new(5, 4), TerrainType::FairyForest.instance(&map_env).set_owner_id(0).build_with_defaults());
    map.set_unit(Point::new(5, 4), Some(UnitType::SmallTank.instance(&map_env).set_owner_id(1).set_hp(1).build_with_defaults()));

    map.set_terrain(Point::new(3, 0), TerrainType::FairyForest.instance(&map_env).set_owner_id(1).build_with_defaults());
    map.set_unit(Point::new(3, 0), Some(UnitType::SmallTank.instance(&map_env).set_owner_id(1).build_with_defaults()));

    let mut settings = map.settings().unwrap();
    for player in &settings.players {
        assert!(player.get_commander_options().contains(&CommanderType::Tapio));
    }
    settings.fog_mode = FogMode::Constant(FogSetting::Sharp(0));
    settings.players[0].set_commander(CommanderType::Tapio);
    let (mut server, _) = Game::new_server(map.clone(), &settings, Box::new(|| 0.));
    server.players.get_mut(0).unwrap().commander.charge = server.players.get_mut(0).unwrap().commander.get_max_charge();
    let unchanged = server.clone();

    // passive: deal more damage when attacking from forest / grass
    for i in 0..4 {
        server.handle_command(Command::UnitCommand(UnitCommand {
            unload_index: None,
            path: Path::new(Point::new(0, i)),
            action: UnitAction::Attack(AttackVector::Direction(Direction4::D0)),
        }), Box::new(|| 0.)).unwrap();
    }
    assert!(server.get_unit(Point::new(1, 0)).unwrap().get_hp() > server.get_unit(Point::new(1, 1)).unwrap().get_hp(), "stronger attack from grass than street");
    assert!(server.get_unit(Point::new(1, 1)).unwrap().get_hp() > server.get_unit(Point::new(1, 2)).unwrap().get_hp(), "stronger attack from forest than grass");
    assert_eq!(server.get_unit(Point::new(1, 2)).unwrap().get_hp(), server.get_unit(Point::new(1, 3)).unwrap().get_hp(), "fairy forest == normal forest");
    let fairy_forest_hp = server.get_unit(Point::new(0, 3)).unwrap().get_hp();

    // fairy forest heals
    server.handle_command(Command::EndTurn, Box::new(|| 0.)).unwrap();
    assert!(server.get_unit(Point::new(5, 4)).unwrap().get_hp() > 1, "heals even enemy units");
    assert_eq!(server.get_unit(Point::new(0, 3)).unwrap().get_hp(), fairy_forest_hp, "only heal on your own start turn event");
    server.handle_command(Command::EndTurn, Box::new(|| 0.)).unwrap();
    assert!(server.get_unit(Point::new(0, 3)).unwrap().get_hp() > fairy_forest_hp, "heal on your own start turn event");

    // ACTIVE: turn grass into fairy forests
    let mut server = unchanged.clone();
    server.handle_command(Command::commander_power(1, vec![CustomActionData::Point(Point::new(3, 0))]), Box::new(|| 0.)).err().expect("can't turn street into fairy forest");
    for i in 0..2 {
        let charge_before = server.players[0].commander.charge;
        server.handle_command(Command::commander_power(1, vec![CustomActionData::Point(Point::new(2, i))]), Box::new(|| 0.)).expect(&format!("loop {i}"));
        assert!(server.players[0].commander.charge < charge_before);
        assert_eq!(server.get_terrain(Point::new(2, i)).unwrap().typ(), TerrainType::FairyForest);
    }

    // ACTIVE: destroy own fairy forests, dealing damage to enemies nearby
    let mut server = unchanged.clone();
    server.handle_command(Command::commander_power(2, Vec::new()), Box::new(|| 0.)).unwrap();
    assert_eq!(server.get_terrain(Point::new(5, 3)).unwrap().typ(), TerrainType::Grass);
    assert_eq!(server.get_terrain(Point::new(3, 0)).unwrap().typ(), TerrainType::FairyForest, "power doesn't affect others players' fairy forests");
    assert_eq!(server.get_unit(Point::new(0, 2)).unwrap().get_hp(), 100, "don't hurt your own units");
    assert!(server.get_unit(Point::new(1, 2)).unwrap().get_hp() < 100);

    // ACTIVE: see into fairy forests, build units from fairy forests
    let mut server = unchanged.clone();
    server.players.get_mut(0).unwrap().funds = 10000.into();
    server.handle_command(Command::EndTurn, Box::new(|| 0.)).unwrap();
    server.handle_command(Command::EndTurn, Box::new(|| 0.)).unwrap();
    assert_ne!(server.get_fog_at(ClientPerspective::Team(0), Point::new(5, 3)), FogIntensity::TrueSight);
    server.handle_command(Command::BuyUnit(Point::new(5, 3), UnitType::SmallTank, Direction4::D0), Box::new(|| 0.)).err().unwrap();
    server.handle_command(Command::commander_power(3, Vec::new()), Box::new(|| 0.)).unwrap();
    assert_eq!(server.get_fog_at(ClientPerspective::Team(0), Point::new(5, 3)), FogIntensity::TrueSight);
    server.handle_command(Command::BuyUnit(Point::new(5, 3), UnitType::SmallTank, Direction4::D0), Box::new(|| 0.)).unwrap();
    assert_eq!(server.get_unit(Point::new(5, 3)).unwrap().get_status(), ActionStatus::Exhausted);
    assert_eq!(server.get_terrain(Point::new(5, 3)).unwrap().typ(), TerrainType::Grass);
}

#[test]
fn sludge_monster() {
    let config = Arc::new(Config::test_config());
    let map = PointMap::new(5, 5, false);
    let map = WMBuilder::<Direction4>::new(map);
    let mut map = Map::new(map.build(), &config);
    let map_env = map.environment().clone();
    map.set_terrain(Point::new(1, 0), TerrainType::City.instance(&map_env).set_owner_id(0).build_with_defaults());
    map.set_unit(Point::new(1, 0), Some(UnitType::SmallTank.instance(&map_env).set_owner_id(0).set_hp(1).build_with_defaults()));
    map.set_unit(Point::new(0, 1), Some(UnitType::SmallTank.instance(&map_env).set_owner_id(0).set_hp(50).build_with_defaults()));
    map.set_details(Point::new(2, 1), vec![Detail::SludgeToken(SludgeToken::new(&config, 1, 0))]);
    map.set_unit(Point::new(2, 1), Some(UnitType::SmallTank.instance(&map_env).set_owner_id(0).set_hp(60).build_with_defaults()));
    map.set_terrain(Point::new(1, 2), TerrainType::OilPlatform.instance(&map_env).build_with_defaults());
    map.set_unit(Point::new(1, 2), Some(UnitType::SmallTank.instance(&map_env).set_owner_id(0).set_hp(50).build_with_defaults()));
    map.set_unit(Point::new(1, 1), Some(UnitType::SmallTank.instance(&map_env).set_owner_id(1).build_with_defaults()));

    let settings = map.settings().unwrap();

    let mut settings = settings.clone();
    for player in &settings.players {
        assert!(player.get_commander_options().contains(&CommanderType::SludgeMonster));
    }
    settings.fog_mode = FogMode::Constant(FogSetting::None);
    settings.players[0].set_commander(CommanderType::SludgeMonster);
    let (mut server, _) = Game::new_server(map.clone(), &settings, Box::new(|| 0.));
    server.players.get_mut(0).unwrap().commander.charge = server.players.get_mut(0).unwrap().commander.get_max_charge();
    let unchanged = server.clone();

    // no power attack bonuses
    assert_eq!(server.get_unit(Point::new(0, 1)).unwrap().get_hp(), server.get_unit(Point::new(2, 1)).unwrap().get_hp(), "Sludge token should deal 10 damage");
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(Point::new(0, 1)),
        action: UnitAction::Attack(AttackVector::Direction(Direction4::D0)),
    }), Box::new(|| 0.)).unwrap();
    let default_attack = 100 - server.get_unit(Point::new(1, 1)).unwrap().get_hp();
    let mut server = unchanged.clone();
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(Point::new(2, 1)),
        action: UnitAction::Attack(AttackVector::Direction(Direction4::D180)),
    }), Box::new(|| 0.)).unwrap();
    let sludge_attack = 100 - server.get_unit(Point::new(1, 1)).unwrap().get_hp();
    let mut server = unchanged.clone();
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(Point::new(1, 2)),
        action: UnitAction::Attack(AttackVector::Direction(Direction4::D90)),
    }), Box::new(|| 0.)).unwrap();
    let oil_platform_attack = 100 - server.get_unit(Point::new(1, 1)).unwrap().get_hp();
    assert!(default_attack < oil_platform_attack);
    assert!(sludge_attack > oil_platform_attack, "sludge token should give +25% and oil platform only +20%");

    // leave token after unit death
    let mut server = unchanged.clone();
    assert_eq!(server.get_details(Point::new(0, 1)), &[]);
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(Point::new(1, 0)),
        action: UnitAction::Attack(AttackVector::Direction(Direction4::D270)),
    }), Box::new(|| 0.)).unwrap();
    assert_eq!(server.get_details(Point::new(1, 0)), &[Detail::SludgeToken(SludgeToken::new(&config, 0, 0))]);

    // small power
    let mut server = unchanged.clone();
    assert_eq!(server.get_details(Point::new(0, 1)), &[]);
    assert_eq!(server.get_details(Point::new(1, 1)), &[]);
    server.handle_command(Command::commander_power(1, Vec::new()), Box::new(|| 0.)).unwrap();
    assert_eq!(server.get_details(Point::new(0, 1)), &[Detail::SludgeToken(SludgeToken::new(&config, 0, 0))]);
    assert_eq!(server.get_details(Point::new(1, 1)), &[Detail::SludgeToken(SludgeToken::new(&config, 0, 0))]);

    // big power
    let mut server = unchanged.clone();
    assert_eq!(server.get_details(Point::new(0, 1)), &[]);
    assert_eq!(server.get_details(Point::new(1, 1)), &[]);
    assert_eq!(server.get_details(Point::new(2, 1)), &[Detail::SludgeToken(SludgeToken::new(&config, 1, 0))]);
    server.handle_command(Command::commander_power(2, Vec::new()), Box::new(|| 0.)).unwrap();
    assert_eq!(server.get_details(Point::new(0, 1)), &[Detail::SludgeToken(SludgeToken::new(&config, 0, 1))]);
    assert_eq!(server.get_details(Point::new(2, 1)), &[Detail::SludgeToken(SludgeToken::new(&config, 0, 1))]);
    // tokens vanish over time
    server.handle_command(Command::EndTurn, Box::new(|| 0.)).unwrap();
    assert_eq!(server.get_details(Point::new(0, 1)), &[Detail::SludgeToken(SludgeToken::new(&config, 0, 1))]);
    server.handle_command(Command::EndTurn, Box::new(|| 0.)).unwrap();
    assert_eq!(server.get_details(Point::new(0, 1)), &[Detail::SludgeToken(SludgeToken::new(&config, 0, 0))]);
    server.handle_command(Command::EndTurn, Box::new(|| 0.)).unwrap();
    assert_eq!(server.get_details(Point::new(0, 1)), &[Detail::SludgeToken(SludgeToken::new(&config, 0, 0))]);
    server.handle_command(Command::EndTurn, Box::new(|| 0.)).unwrap();
    assert_eq!(server.get_details(Point::new(0, 1)), &[]);
    let mut server = unchanged.clone();
    assert_eq!(server.get_details(Point::new(2, 1)), &[Detail::SludgeToken(SludgeToken::new(&config, 1, 0))]);
    server.handle_command(Command::EndTurn, Box::new(|| 0.)).unwrap();
    assert_eq!(server.get_details(Point::new(2, 1)), &[]);

    // can't repair
    let mut server = unchanged.clone();
    assert_eq!(server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(Point::new(1, 0)),
        action: UnitAction::custom(1, Vec::new()),
    }), Box::new(|| 0.)), Err(CommandError::InvalidAction));
}

#[test]
fn celerity() {
    let config = Arc::new(Config::test_config());
    let map = PointMap::new(5, 5, false);
    let map = WMBuilder::<Direction4>::new(map);
    let mut map = Map::new(map.build(), &config);
    let map_env = map.environment().clone();
    map.set_unit(Point::new(0, 0), Some(UnitType::SmallTank.instance(&map_env).set_owner_id(0).set_hp(1).build_with_defaults()));

    map.set_unit(Point::new(4, 0), Some(UnitType::Convoy.instance(&map_env).set_owner_id(0).set_transported(vec![
        UnitType::Marine.instance(&map_env).set_hp(34).build_with_defaults(),
        UnitType::Sniper.instance(&map_env).set_hp(69).build_with_defaults(),
    ]).set_hp(89).build_with_defaults()));

    map.set_unit(Point::new(2, 1), Some(UnitType::SmallTank.instance(&map_env).set_owner_id(1).set_hp(1).build_with_defaults()));
    map.set_unit(Point::new(1, 2), Some(UnitType::SmallTank.instance(&map_env).set_owner_id(1).set_hp(1).build_with_defaults()));
    map.set_unit(Point::new(2, 2), Some(UnitType::SmallTank.instance(&map_env).set_owner_id(0).build_with_defaults()));
    map.set_unit(Point::new(3, 2), Some(UnitType::SmallTank.instance(&map_env).set_owner_id(1).build_with_defaults()));
    map.set_unit(Point::new(2, 3), Some(UnitType::SmallTank.instance(&map_env).set_owner_id(1).set_hp(1).build_with_defaults()));

    map.set_terrain(Point::new(0, 4), TerrainType::Factory.instance(&map_env).set_owner_id(0).build_with_defaults());

    let settings = map.settings().unwrap();

    let mut settings = settings.clone();
    for player in &settings.players {
        assert!(player.get_commander_options().contains(&CommanderType::Celerity));
    }
    settings.fog_mode = FogMode::Constant(FogSetting::None);
    let funds = 10000;
    settings.players[0].set_funds(funds);

    // get some default values without using Celerity
    let (mut server, _) = Game::new_server(map.clone(), &settings, Box::new(|| 0.));
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(Point::new(2, 2)),
        action: UnitAction::Attack(AttackVector::Direction(Direction4::D0)),
    }), Box::new(|| 0.)).unwrap();
    let default_attack = 100 - server.get_unit(Point::new(3, 2)).unwrap().get_hp();
    server.handle_command(Command::BuyUnit(Point::new(0, 4), UnitType::SmallTank, Direction4::D0), Box::new(|| 0.)).unwrap();
    let default_cost = funds - *server.current_player().funds;
    assert!(!server.get_unit(Point::new(0, 4)).unwrap().has_attribute(AttributeKey::Level));

    settings.players[0].set_commander(CommanderType::Celerity);
    let (mut server, _) = Game::new_server(map.clone(), &settings, Box::new(|| 0.));
    let environment = server.environment().clone();
    server.players.get_mut(0).unwrap().commander.charge = server.players.get_mut(0).unwrap().commander.get_max_charge();
    let unchanged = server.clone();

    server.handle_command(Command::BuyUnit(Point::new(0, 4), UnitType::SmallTank, Direction4::D0), Box::new(|| 0.)).unwrap();
    assert!(funds - *server.current_player().funds < default_cost);
    assert_eq!(server.get_unit(Point::new(0, 4)).unwrap().get_level(), 0, "New units are Level 0");

    // level attack bonuses
    let mut attack_damage = Vec::new();
    for i in 0..=3 {
        let mut server = unchanged.clone();
        for d in Direction4::list().into_iter().take(i + 1).rev() {
            server.handle_command(Command::UnitCommand(UnitCommand {
                unload_index: None,
                path: Path::new(Point::new(2, 2)),
                action: UnitAction::Attack(AttackVector::Direction(d)),
            }), Box::new(|| 0.)).unwrap();
            server.handle_command(Command::EndTurn, Box::new(|| 0.)).unwrap();
            server.handle_command(Command::EndTurn, Box::new(|| 0.)).unwrap();
        }
        assert_eq!(server.get_unit(Point::new(2, 2)).unwrap().get_level(), i as u8);
        attack_damage.push(100 - server.get_unit(Point::new(3, 2)).unwrap().get_hp());
    }
    for i in 0..attack_damage.len() - 1 {
        assert!(attack_damage[i] < attack_damage[i + 1], "attack damage by level: {:?}", attack_damage);
    }
    assert_eq!(default_attack, attack_damage[1]);

    // small power
    let mut server = unchanged.clone();
    server.handle_command(Command::commander_power(1, Vec::new()), Box::new(|| 0.)).unwrap();
    assert!(server.get_unit(Point::new(0, 0)).unwrap().get_hp() > 1);
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(Point::new(2, 2)),
        action: UnitAction::Attack(AttackVector::Direction(Direction4::D0)),
    }), Box::new(|| 0.)).unwrap();
    assert_eq!(default_attack, 100 - server.get_unit(Point::new(3, 2)).unwrap().get_hp());

    // big power
    let mut server = unchanged.clone();
    server.handle_command(Command::commander_power(2, Vec::new()), Box::new(|| 0.)).unwrap();
    assert_eq!(server.get_unit(Point::new(0, 0)).unwrap().get_hp(), 1);
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::new(Point::new(2, 2)),
        action: UnitAction::Attack(AttackVector::Direction(Direction4::D0)),
    }), Box::new(|| 0.)).unwrap();
    assert_eq!(attack_damage[3], 100 - server.get_unit(Point::new(3, 2)).unwrap().get_hp());

    let convoy: Unit<Direction6> = UnitType::Convoy.instance(&environment).set_owner_id(0).set_transported(vec![
        UnitType::Marine.instance(&environment).set_hp(34).build_with_defaults(),
        UnitType::Sniper.instance(&environment).set_hp(69).build_with_defaults(),
    ]).set_hp(89).build_with_defaults();

    let mut zipper = Zipper::new();
    convoy.zip(&mut zipper, None);
    let mut unzipper = Unzipper::new(zipper.finish(), Version::parse(VERSION).unwrap());
    let convoy2 = Unit::unzip(&mut unzipper, &environment, None);
    assert_eq!(Ok(convoy), convoy2);

    let exported = unchanged.export();
    let imported = *Game::import_server(exported, &config, Version::parse(VERSION).unwrap()).unwrap();
    assert_eq!(imported, *unchanged);
    assert_eq!(server.get_unit(Point::new(4, 0)), Some(&UnitType::Convoy.instance(&environment).set_owner_id(0).set_transported(vec![
        UnitType::Marine.instance(&environment).set_hp(34).build_with_defaults(),
        UnitType::Sniper.instance(&environment).set_hp(69).build_with_defaults(),
    ]).set_hp(89).build_with_defaults()));
}
