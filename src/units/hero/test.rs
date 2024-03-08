
#[cfg(test)]
mod tests {

    use std::sync::Arc;

    use interfaces::game_interface::*;
    use interfaces::map_interface::*;
    use crate::config::config::Config;
    use crate::game::commands::Command;
    use crate::game::fog::*;
    use crate::map::direction::*;
    use crate::map::map::Map;
    use crate::map::point::Point;
    use crate::map::point::Position;
    use crate::map::point_map::PointMap;
    use crate::map::wrapping_map::WMBuilder;
    use crate::script::custom_action::CustomActionData;
    use crate::terrain::TerrainType;
    use crate::units::combat::AttackVector;
    use crate::units::commands::UnitAction;
    use crate::units::commands::UnitCommand;
    use crate::units::hero::ActionStatus;
    use crate::units::hero::Hero;
    use crate::units::hero::HeroType;
    use crate::units::movement::Path;
    use crate::units::movement::PathStep;
    use crate::units::unit_types::UnitType;

    #[test]
    fn buy_hero() {
        let config = Arc::new(Config::test_config());
        let map = PointMap::new(5, 5, false);
        let map = WMBuilder::<Direction4>::new(map);
        let mut map = Map::new(map.build(), &config);
        let map_env = map.environment().clone();
        map.set_terrain(Point::new(1, 1), TerrainType::Memorial.instance(&map_env).set_owner_id(0).build_with_defaults());
        map.set_unit(Point::new(1, 1), Some(UnitType::SmallTank.instance(&map_env).set_owner_id(0).build_with_defaults()));
        map.set_unit(Point::new(4, 4), Some(UnitType::SmallTank.instance(&map_env).set_owner_id(1).set_hero(Hero::new(HeroType::CrystalObelisk, None)).build_with_defaults()));

        let settings = map.settings().unwrap();
        let mut settings = settings.clone();
        settings.fog_mode = FogMode::Constant(FogSetting::None);
        settings.players[0].set_funds(999999);

        let (mut server, _) = map.clone().game_server(&settings, || 0.);
        let environment: crate::config::environment::Environment = server.environment().clone();
        assert_eq!(Hero::aura_range(Some(&server), server.get_map(), server.get_map().get_unit(Point::new(1, 1)).unwrap(), Point::new(1, 1), None), None);
        let path = Path::new(Point::new(1, 1));
        let options = server.get_map().get_unit(Point::new(1, 1)).unwrap().options_after_path(&server, &path, None, &[]);
        println!("options: {:?}", options);
        assert!(options.contains(&UnitAction::BuyHero(HeroType::Crystal)));
        server.handle_command(Command::UnitCommand(UnitCommand {
            unload_index: None,
            path,
            action: UnitAction::BuyHero(HeroType::Crystal),
        }), || 0.).unwrap();
        assert_eq!(server.get_map().get_unit(Point::new(1, 1)), Some(&UnitType::SmallTank.instance(&environment).set_owner_id(0).set_hero(Hero::new(HeroType::Crystal, Some(Point::new(1, 1)))).set_status(ActionStatus::Exhausted).build_with_defaults()));
        assert_eq!(Hero::aura_range(Some(&server), server.get_map(), server.get_map().get_unit(Point::new(1, 1)).unwrap(), Point::new(1, 1), None), Some(2));
    }


    #[test]
    fn crystal() {
        let config = Arc::new(Config::test_config());
        let map = PointMap::new(5, 5, false);
        let map = WMBuilder::<Direction4>::new(map);
        let mut map = Map::new(map.build(), &config);
        let map_env = map.environment().clone();
        let mut crystal = Hero::new(HeroType::Crystal, None);
        crystal.set_charge(&map_env, crystal.max_charge(&map_env));
        //map.set_unit(Point::new(0, 0), Some(UnitType::SmallTank.instance(&map_env).set_owner_id(0).set_hero(Hero::new(HeroType::CrystalObelisk, None)).build_with_defaults()));
        map.set_unit(Point::new(1, 1), Some(UnitType::SmallTank.instance(&map_env).set_owner_id(0).set_hero(crystal).set_hp(1).build_with_defaults()));
        map.set_unit(Point::new(2, 1), Some(UnitType::SmallTank.instance(&map_env).set_owner_id(0).build_with_defaults()));
        map.set_unit(Point::new(3, 1), Some(UnitType::SmallTank.instance(&map_env).set_owner_id(1).build_with_defaults()));

        map.set_unit(Point::new(4, 4), Some(UnitType::SmallTank.instance(&map_env).set_owner_id(0).set_hero(Hero::new(HeroType::CrystalObelisk, None)).build_with_defaults()));

        let settings = map.settings().unwrap();
        let mut settings = settings.clone();
        settings.fog_mode = FogMode::Constant(FogSetting::None);

        let (mut server, _) = map.clone().game_server(&settings, || 0.);
        let unchanged = server.clone();
        let environment: crate::config::environment::Environment = server.environment().clone();
        assert_eq!(Hero::aura_range(Some(&server), server.get_map(), server.get_map().get_unit(Point::new(1, 1)).unwrap(), Point::new(1, 1), None), Some(2));
        assert_eq!(Hero::aura_range(Some(&server), server.get_map(), server.get_map().get_unit(Point::new(4, 4)).unwrap(), Point::new(4, 4), None), Some(2));
        // use power
        let path = Path::new(Point::new(1, 1));
        let options = server.get_map().get_unit(Point::new(1, 1)).unwrap().options_after_path(&server, &path, None, &[]);
        println!("options: {:?}", options);
        assert!(options.contains(&UnitAction::HeroPower(1, Vec::new())));
        server.handle_command(Command::UnitCommand(UnitCommand {
            unload_index: None,
            path,
            action: UnitAction::HeroPower(1, vec![CustomActionData::Point(Point::new(0, 1))]),
        }), || 0.).unwrap();
        assert_eq!(server.get_map().get_unit(Point::new(0, 1)), Some(&UnitType::HeroCrystal.instance(&environment).set_owner_id(0).set_hero(Hero::new(HeroType::CrystalObelisk, None)).build_with_defaults()));
        assert_eq!(Hero::aura_range(Some(&server), server.get_map(), server.get_map().get_unit(Point::new(1, 1)).unwrap(), Point::new(1, 1), None), Some(3));
        assert_eq!(Hero::aura_range(Some(&server), server.get_map(), server.get_map().get_unit(Point::new(4, 4)).unwrap(), Point::new(4, 4), None), Some(3));
        server.handle_command(Command::UnitCommand(UnitCommand {
            unload_index: None,
            path: Path::new(Point::new(2, 1)),
            action: UnitAction::Attack(AttackVector::Direction(Direction4::D0)),
        }), || 0.).unwrap();
        let power_aura_damage = 100 - server.get_map().get_unit(Point::new(3, 1)).unwrap().get_hp();

        // don't use power
        let mut server = unchanged.clone();
        server.handle_command(Command::UnitCommand(UnitCommand {
            unload_index: None,
            path: Path::new(Point::new(2, 1)),
            action: UnitAction::Attack(AttackVector::Direction(Direction4::D0)),
        }), || 0.).unwrap();
        let aura_damage = 100 - server.get_map().get_unit(Point::new(3, 1)).unwrap().get_hp();
        server.handle_command(Command::EndTurn, || 0.).unwrap();
        server.handle_command(Command::EndTurn, || 0.).unwrap();
        assert_eq!(server.get_map().get_unit(Point::new(4, 4)).unwrap().get_hp(), 100);
        assert_eq!(Hero::hero_influence_at(Some(&server), server.get_map(), Point::new(0, 0), 0).len(), 1);
        assert_eq!(Hero::hero_influence_at(Some(&server), server.get_map(), Point::new(0, 0), 1).len(), 0);
        assert_eq!(Hero::hero_influence_at(Some(&server), server.get_map(), Point::new(0, 0), -1).len(), 1);

        assert!(aura_damage < power_aura_damage);

        // test crystal obelisk behavior when hero is missing
        map.set_unit(Point::new(1, 1), None);
        let (mut server, _) = map.clone().game_server(&settings, || 0.);
        assert_eq!(server.get_map().get_unit(Point::new(4, 4)).unwrap().get_hp(), 80);
        server.handle_command(Command::EndTurn, || 0.).unwrap();
        server.handle_command(Command::EndTurn, || 0.).unwrap();
        assert_eq!(server.get_map().get_unit(Point::new(4, 4)).unwrap().get_hp(), 60);
        server.handle_command(Command::UnitCommand(UnitCommand {
            unload_index: None,
            path: Path::new(Point::new(2, 1)),
            action: UnitAction::Attack(AttackVector::Direction(Direction4::D0)),
        }), || 0.).unwrap();
        let normal_damage = 100 - server.get_map().get_unit(Point::new(3, 1)).unwrap().get_hp();

        assert!(normal_damage < aura_damage);
    }

    #[test]
    fn earl_grey() {
        let config = Arc::new(Config::test_config());
        let map = PointMap::new(5, 5, false);
        let map = WMBuilder::<Direction4>::new(map);
        let mut map = Map::new(map.build(), &config);
        let map_env = map.environment().clone();
        let mut earl_grey = Hero::new(HeroType::EarlGrey, None);
        earl_grey.set_charge(&map_env, earl_grey.max_charge(&map_env));
        //map.set_unit(Point::new(0, 0), Some(UnitType::SmallTank.instance(&map_env).set_owner_id(0).set_hero(Hero::new(HeroType::CrystalObelisk, None)).build_with_defaults()));
        map.set_unit(Point::new(1, 1), Some(UnitType::HoverBike.instance(&map_env).set_owner_id(0).set_hero(earl_grey).set_hp(1).build_with_defaults()));
        map.set_unit(Point::new(2, 1), Some(UnitType::HoverBike.instance(&map_env).set_owner_id(0).build_with_defaults()));
        map.set_unit(Point::new(3, 1), Some(UnitType::HoverBike.instance(&map_env).set_owner_id(1).build_with_defaults()));

        map.set_unit(Point::new(4, 4), Some(UnitType::HoverBike.instance(&map_env).set_owner_id(0).build_with_defaults()));

        let settings = map.settings().unwrap();
        let mut settings = settings.clone();
        settings.fog_mode = FogMode::Constant(FogSetting::None);

        let (mut server, _) = map.clone().game_server(&settings, || 0.);
        let influence1 = Hero::hero_influence_at(Some(&server), server.get_map(), Point::new(2, 1), 0);
        let influence2 = Hero::hero_influence_at(Some(&server), server.get_map(), Point::new(4, 4), 0);
        assert_eq!(
            server.get_map().get_unit(Point::new(2, 1)).unwrap().movement_points(Some(&server), server.get_map(), Point::new(2, 1), None, &influence1),
            server.get_map().get_unit(Point::new(4, 4)).unwrap().movement_points(Some(&server), server.get_map(), Point::new(4, 4), None, &influence2),
        );
        // hero power shouldn't be available if the hero moves
        let mut path = Path::new(Point::new(1, 1));
        path.steps.push(PathStep::Dir(Direction4::D90));
        let options = server.get_map().get_unit(Point::new(1, 1)).unwrap().options_after_path(&server, &path, None, &[]);
        println!("options: {:?}", options);
        assert!(!options.contains(&UnitAction::HeroPower(1, Vec::new())));
        // use power
        let path = Path::new(Point::new(1, 1));
        let options = server.get_map().get_unit(Point::new(1, 1)).unwrap().options_after_path(&server, &path, None, &[]);
        println!("options: {:?}", options);
        assert!(options.contains(&UnitAction::HeroPower(1, Vec::new())));
        server.handle_command(Command::UnitCommand(UnitCommand {
            unload_index: None,
            path,
            action: UnitAction::HeroPower(1, Vec::new()),
        }), || 0.).unwrap();
        assert_eq!(server.get_map().get_unit(Point::new(1, 1)).unwrap().get_status(), ActionStatus::Ready);
        let influence1 = Hero::hero_influence_at(Some(&server), server.get_map(), Point::new(2, 1), 0);
        let influence2 = Hero::hero_influence_at(Some(&server), server.get_map(), Point::new(4, 4), 0);
        assert!(
            server.get_map().get_unit(Point::new(2, 1)).unwrap().movement_points(Some(&server), server.get_map(), Point::new(2, 1), None, &influence1)
            >
            server.get_map().get_unit(Point::new(4, 4)).unwrap().movement_points(Some(&server), server.get_map(), Point::new(4, 4), None, &influence2)
        );
    }

    #[test]
    fn blue_berry() {
        let config = Arc::new(Config::test_config());
        let map = PointMap::new(5, 5, false);
        let map = WMBuilder::<Direction4>::new(map);
        let mut map = Map::new(map.build(), &config);
        let map_env = map.environment().clone();
        let mut blue_berry = Hero::new(HeroType::BlueBerry, None);
        blue_berry.set_charge(&map_env, blue_berry.max_charge(&map_env));
        //map.set_unit(Point::new(0, 0), Some(UnitType::SmallTank.instance(&map_env).set_owner_id(0).set_hero(Hero::new(HeroType::CrystalObelisk, None)).build_with_defaults()));
        map.set_unit(Point::new(1, 1), Some(UnitType::HoverBike.instance(&map_env).set_owner_id(0).set_hero(blue_berry).set_hp(1).build_with_defaults()));
        map.set_unit(Point::new(2, 1), Some(UnitType::HoverBike.instance(&map_env).set_owner_id(0).set_hp(50).set_status(ActionStatus::Exhausted).build_with_defaults()));
        map.set_unit(Point::new(3, 1), Some(UnitType::HoverBike.instance(&map_env).set_owner_id(1).build_with_defaults()));

        map.set_unit(Point::new(4, 4), Some(UnitType::HoverBike.instance(&map_env).set_owner_id(0).set_hp(50).set_status(ActionStatus::Exhausted).build_with_defaults()));

        let settings = map.settings().unwrap();
        let mut settings = settings.clone();
        settings.fog_mode = FogMode::Constant(FogSetting::None);

        let (mut server, _) = map.clone().game_server(&settings, || 0.);
        assert!(server.get_map().get_unit(Point::new(2, 1)).unwrap().get_hp() > 50);
        assert_eq!(server.get_map().get_unit(Point::new(4, 4)).unwrap().get_hp(), 50);
        // use power
        let path = Path::new(Point::new(1, 1));
        let options = server.get_map().get_unit(Point::new(1, 1)).unwrap().options_after_path(&server, &path, None, &[]);
        println!("options: {:?}", options);
        assert!(options.contains(&UnitAction::HeroPower(1, Vec::new())));
        server.handle_command(Command::UnitCommand(UnitCommand {
            unload_index: None,
            path,
            action: UnitAction::HeroPower(1, Vec::new()),
        }), || 0.).unwrap();
        assert_eq!(server.get_map().get_unit(Point::new(2, 1)).unwrap().get_status(), ActionStatus::Ready);
        assert_eq!(server.get_map().get_unit(Point::new(4, 4)).unwrap().get_status(), ActionStatus::Exhausted);
    }

    #[test]
    fn tess() {
        let config = Arc::new(Config::test_config());
        let map = PointMap::new(5, 5, false);
        let map = WMBuilder::<Direction4>::new(map);
        let mut map = Map::new(map.build(), &config);
        let map_env = map.environment().clone();
        let mut tess = Hero::new(HeroType::Tess, None);
        tess.set_charge(&map_env, tess.max_charge(&map_env));
        //map.set_unit(Point::new(0, 0), Some(UnitType::SmallTank.instance(&map_env).set_owner_id(0).set_hero(Hero::new(HeroType::CrystalObelisk, None)).build_with_defaults()));
        map.set_unit(Point::new(1, 1), Some(UnitType::HoverBike.instance(&map_env).set_owner_id(0).set_hero(tess).set_hp(1).build_with_defaults()));
        map.set_unit(Point::new(3, 1), Some(UnitType::HoverBike.instance(&map_env).set_owner_id(1).build_with_defaults()));

        let settings = map.settings().unwrap();
        let mut settings = settings.clone();
        settings.fog_mode = FogMode::Constant(FogSetting::None);
        settings.players[0].set_funds(9999);

        let (mut server, _) = map.clone().game_server(&settings, || 0.);
        // use power
        let path = Path::new(Point::new(1, 1));
        let options = server.get_map().get_unit(Point::new(1, 1)).unwrap().options_after_path(&server, &path, None, &[]);
        println!("options: {:?}", options);
        assert!(options.contains(&UnitAction::HeroPower(1, Vec::new())));
        server.handle_command(Command::UnitCommand(UnitCommand {
            unload_index: None,
            path,
            action: UnitAction::HeroPower(1, vec![CustomActionData::UnitType(UnitType::SmallTank), CustomActionData::Direction(Direction4::D0)]),
        }), || 0.).unwrap();
        assert_eq!(server.get_map().get_unit(Point::new(2, 1)).unwrap().get_status(), ActionStatus::Ready);
        assert!(*server.get_owning_player(0).unwrap().funds < 9999);
    }

    #[test]
    fn edwin() {
        let config = Arc::new(Config::test_config());
        let map = PointMap::new(5, 5, false);
        let map = WMBuilder::<Direction4>::new(map);
        let mut map = Map::new(map.build(), &config);
        let map_env = map.environment().clone();
        let mut edwin = Hero::new(HeroType::Edwin, None);
        edwin.set_charge(&map_env, edwin.max_charge(&map_env));
        map.set_unit(Point::new(1, 1), Some(UnitType::HoverBike.instance(&map_env).set_owner_id(0).set_hero(edwin).build_with_defaults()));
        let enemy = UnitType::HoverBike.instance(&map_env).set_owner_id(1).build_with_defaults();
        map.set_unit(Point::new(2, 1), Some(enemy.clone()));
        let friend = UnitType::SmallTank.instance(&map_env).set_owner_id(0).build_with_defaults();
        map.set_unit(Point::new(0, 4), Some(friend.clone()));

        let settings = map.settings().unwrap();
        let mut settings = settings.clone();
        settings.fog_mode = FogMode::Constant(FogSetting::None);

        let (mut server, _) = map.clone().game_server(&settings, || 0.);
        let unchanged = server.clone();
        // use power
        let path = Path::new(Point::new(1, 1));
        let options = server.get_map().get_unit(Point::new(1, 1)).unwrap().options_after_path(&server, &path, None, &[]);
        println!("options: {:?}", options);
        assert!(options.contains(&UnitAction::HeroPower(1, Vec::new())));
        server.handle_command(Command::UnitCommand(UnitCommand {
            unload_index: None,
            path,
            action: UnitAction::HeroPower(1, vec![CustomActionData::Point(Point::new(2, 1)), CustomActionData::Point(Point::new(0, 4))]),
        }), || 0.).unwrap();
        assert_eq!(server.get_map().get_unit(Point::new(2, 1)), Some(&friend));
        assert_eq!(server.get_map().get_unit(Point::new(0, 4)), Some(&enemy));

        // knockback
        let mut server = unchanged.clone();
        server.handle_command(Command::UnitCommand(UnitCommand {
            unload_index: None,
            path: Path::new(Point::new(1, 1)),
            action: UnitAction::Attack(AttackVector::Direction(Direction4::D0)),
        }), || 0.).unwrap();
        assert_eq!(server.get_map().get_unit(Point::new(2, 1)), None);
        assert!(server.get_map().get_unit(Point::new(3, 1)).is_some());
    }
}
