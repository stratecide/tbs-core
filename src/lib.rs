pub mod map;
pub mod player;
pub mod terrain;
pub mod units;
pub mod details;
pub mod game;
pub mod commanders;

pub use zipper;
pub use interfaces;

#[cfg(test)]
mod tests {

    use interfaces::Game;
    use crate::game::game::*;
    use crate::map::direction::*;
    use crate::map::map::Map;
    use crate::map::wrapping_map::WrappingMapBuilder;
    use crate::player::*;
    use crate::units::UnitType;
    use crate::units::normal_units::NormalUnits;

    use super::map::point_map::PointMap;
    use super::map::point::*;
    

    #[test]
    fn filled_point_map() {
        let map = PointMap::new(5, 6, false);
        assert_eq!(map.width(), 5);
        assert_eq!(map.height(), 6);
        for x in 0..5 {
            for y in 0..6 {
                assert!(map.is_point_valid(Point::new(x, y)));
            }
            assert!(!map.is_point_valid(Point::new(x, 6)));
        }
        for y in 0..6 {
            assert!(!map.is_point_valid(Point::new(5, y)));
        }
    }

    #[test]
    fn export_game() {
        let pointmap = PointMap::new(7, 5, true);
        let wrapping = WrappingMapBuilder::new(pointmap, vec![]).build().unwrap();
        let mut map = Map::<Direction4>::new(wrapping);
        
        map.set_unit(Point::new(0, 2), Some(UnitType::normal(NormalUnits::Hovercraft(true), OWNER_0)));
        map.set_unit(Point::new(6, 2), Some(UnitType::normal(NormalUnits::Hovercraft(true), OWNER_1)));

        let mut settings = map.settings().unwrap();
        settings.fog_mode = FogMode::Always;
        let (server, events) = crate::game::game::Game::new_server(map.clone(), &settings, || 0.0);
        let exported_server = server.export();
        
        println!("exported server: {:?}", exported_server);
        
        let imported_server = import_server::<Direction4>(exported_server.clone()).unwrap();
        assert_eq!(imported_server, server);
        
        for team in [None, Some(OWNER_0), Some(OWNER_1)] {
            println!("testing client import for perspective {:?}", team);
            let client = crate::game::game::Game::new_client(map.clone(), &settings, events.get(&Some(team)).unwrap());
            let client_imported = import_client::<Direction4>(exported_server.public.clone(), team.as_ref().and_then(|team| Some((team.clone(), exported_server.clone().hidden.unwrap().teams.get(&**team).unwrap().clone())))).unwrap();
            assert_eq!(client, client_imported);
        }
    }
}
