#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use interfaces::game_interface::{GameInterface, Perspective};
    use interfaces::map_interface::MapInterface;

    use crate::config::config::Config;
    use crate::game::commands::Command;
    use crate::game::fog::*;
    use crate::game::game_view::GameView;
    use crate::map::direction::*;
    use crate::map::map::Map;
    use crate::map::map_view::MapView;
    use crate::map::point::*;
    use crate::map::point_map::PointMap;
    use crate::map::wrapping_map::*;
    use crate::script::custom_action::CustomActionData;
    use crate::terrain::TerrainType;
    use crate::units::attributes::ActionStatus;
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
        let unit: Unit<Direction4> = UnitType::TransportHeli.instance(&map_env).set_owner_id(0).set_transported(vec![
            UnitType::Marine.instance(&map_env).set_hp(34).build_with_defaults(),
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
        let unit = UnitType::Sniper.instance(&map_env).set_owner_id(0).set_status(ActionStatus::Capturing).build_with_defaults();
        assert_eq!(
            unit.fog_replacement(&map, Point::new(1, 1), FogIntensity::Light),
            Some(UnitType::Unknown.instance(&map_env).build_with_defaults())
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
        map.set_unit(Point::new(3, 4), Some(UnitType::DroneBoat.instance(&map_env).set_owner_id(0).build_with_defaults()));
        map.set_unit(Point::new(1, 6), Some(UnitType::WarShip.instance(&map_env).set_owner_id(1).build_with_defaults()));
        let mut settings = map.settings().unwrap();
        settings.fog_mode = FogMode::Constant(FogSetting::None);
        settings.players[0].set_funds(1000);
        let (mut server, events) = map.clone().game_server(&settings, || 0.);
        let mut client = map.game_client(&settings, events.get(&Perspective::Team(0)).unwrap().0);
        let events = server.handle_command(Command::UnitCommand(UnitCommand {
            unload_index: None,
            path: Path::new(Point::new(3, 4)),
            action: UnitAction::Custom(0, vec![CustomActionData::UnitType(UnitType::LightDrone)]),
        }), || 0.).unwrap();
        for ev in events.get(&Perspective::Team(0)).unwrap().0 {
            client.handle_event(ev);
        }
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
        map.set_unit(Point::new(3, 4), Some(UnitType::SmallTank.instance(&map_env).set_owner_id(0).set_hp(1).build_with_defaults()));
        map.set_unit(Point::new(1, 6), Some(UnitType::WarShip.instance(&map_env).set_owner_id(1).build_with_defaults()));
        let mut settings = map.settings().unwrap();
        settings.fog_mode = FogMode::Constant(FogSetting::None);
        settings.players[0].set_funds(1000);
        let (mut server, _) = map.clone().game_server(&settings, || 0.);
        server.handle_command(Command::UnitCommand(UnitCommand {
            unload_index: None,
            path: Path::new(Point::new(3, 4)),
            action: UnitAction::Repair,
        }), || 0.).unwrap();
        assert!(*server.get_owning_player(0).unwrap().funds < 1000);
        assert!(server.get_unit(Point::new(3, 4)).unwrap().get_hp() > 1);
        assert_eq!(server.get_unit(Point::new(3, 4)).unwrap().get_status(), ActionStatus::Repairing);
        server.handle_command(Command::EndTurn, || 0.).unwrap();
        server.handle_command(Command::EndTurn, || 0.).unwrap();
        assert_eq!(server.get_unit(Point::new(3, 4)).unwrap().get_status(), ActionStatus::Ready);
    }
}
