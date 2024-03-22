

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use interfaces::game_interface::GameInterface;
    use interfaces::map_interface::MapInterface;

    use crate::config::config::Config;
    use crate::config::environment::Environment;
    use crate::game::commands::Command;
    use crate::game::fog::FogMode;
    use crate::game::settings::{GameSettings, PlayerSettings};
    use crate::map::direction::*;
    use crate::map::map::Map;
    use crate::map::map_view::MapView;
    use crate::map::point::*;
    use crate::map::point_map::PointMap;
    use crate::terrain::TerrainType;
    use crate::map::wrapping_map::*;
    use crate::units::commands::{UnitCommand, UnitAction};
    use crate::units::movement::{Path, PathStep};
    use crate::units::unit_types::UnitType;


    #[test]
    fn capture_city() {
        let map = PointMap::new(4, 4, false);
        let environment = Environment {
            config: Arc::new(Config::test_config()),
            map_size: map.size(),
            settings: None,
        };
        let wmap: WrappingMap<Direction4> = WMBuilder::new(map).build();
        let mut map = Map::new2(wmap, &environment);
        map.set_terrain(Point::new(1, 1), TerrainType::City.instance(&environment).set_owner_id(-1).build_with_defaults());
        map.set_unit(Point::new(0, 0), Some(UnitType::Sniper.instance(&environment).set_owner_id(0).build_with_defaults()));
        map.set_unit(Point::new(3, 3), Some(UnitType::Sniper.instance(&environment).set_owner_id(1).build_with_defaults()));
        let (mut game, _) = map.game_server(&GameSettings {
            name: "capture_city".to_string(),
            fog_mode: FogMode::Constant(crate::game::fog::FogSetting::None),
            players: vec![
                PlayerSettings::new(&environment.config, 0),
                PlayerSettings::new(&environment.config, 1),
            ],
        }, || 0.);
        game.handle_command(Command::UnitCommand(UnitCommand {
            unload_index: None,
            path: Path {
                start: Point::new(0, 0),
                steps: vec![
                    PathStep::Dir(Direction4::D0),
                    PathStep::Dir(Direction4::D270),
                ],
            },
            action: UnitAction::Capture,
        }), || 0.).unwrap();
        game.handle_command(Command::EndTurn, || 0.).unwrap();
        game.handle_command(Command::EndTurn, || 0.).unwrap();
        assert_eq!(0, game.get_map().get_terrain(Point::new(1, 1)).unwrap().get_owner_id());
    }

    #[test]
    fn build_unit() {
        let map = PointMap::new(4, 4, false);
        let environment = Environment {
            config: Arc::new(Config::test_config()),
            map_size: map.size(),
            settings: None,
        };
        let wmap: WrappingMap<Direction4> = WMBuilder::new(map).build();
        let mut map = Map::new2(wmap, &environment);
        map.set_terrain(Point::new(0, 0), TerrainType::Factory.instance(&environment).set_owner_id(0).build_with_defaults());
        map.set_terrain(Point::new(1, 1), TerrainType::City.instance(&environment).set_owner_id(0).build_with_defaults());
        map.set_unit(Point::new(3, 3), Some(UnitType::Sniper.instance(&environment).set_owner_id(1).build_with_defaults()));
        let mut player_setting = PlayerSettings::new(&environment.config, 0);
        player_setting.set_income(200);
        let (mut game, _) = map.game_server(&GameSettings {
            name: "build_unit".to_string(),
            fog_mode: FogMode::Constant(crate::game::fog::FogSetting::None),
            players: vec![
                player_setting,
                PlayerSettings::new(&environment.config, 1),
            ],
        }, || 0.);
        assert_eq!(200, *game.current_player().funds);
        game.handle_command(Command::BuyUnit(Point::new(0, 0), UnitType::Marine, Direction4::D0), || 0.).unwrap();
        assert!(*game.current_player().funds < 200);
        assert_eq!(0, game.get_map().get_unit(Point::new(0, 0)).unwrap().get_owner_id());
        assert!(game.get_map().get_unit(Point::new(0, 0)).unwrap().is_exhausted());
    }
}
