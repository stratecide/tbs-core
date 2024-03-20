#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::config::config::Config;
    use crate::game::fog::FogIntensity;
    use crate::map::direction::*;
    use crate::map::map::Map;
    use crate::map::map_view::MapView;
    use crate::map::point::*;
    use crate::map::point_map::PointMap;
    use crate::map::wrapping_map::*;
    use crate::terrain::TerrainType;
    use crate::units::attributes::ActionStatus;
    use crate::units::unit_types::UnitType;


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
}