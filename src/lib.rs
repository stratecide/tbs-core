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

    use interfaces::game_interface::*;
    use interfaces::map_interface::*;
    use crate::game::game::*;
    use crate::game::fog::*;
    use crate::map::direction::*;
    use crate::map::map::Map;
    use crate::map::wrapping_map::WrappingMapBuilder;
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
        
        map.set_unit(Point::new(0, 2), Some(UnitType::normal(NormalUnits::Hovercraft(true), 0.into())));
        map.set_unit(Point::new(6, 2), Some(UnitType::normal(NormalUnits::Hovercraft(true), 1.into())));

        let mut settings = map.settings().unwrap();
        settings.fog_mode = FogMode::Constant(FogSetting::Sharp(0));
        let (server, events) = crate::game::game::Game::new_server(map.clone(), &settings, || 0.0);
        let exported_server = server.export();
        
        println!("exported server: {:?}", exported_server);
        println!("events: {:?}", events.get(&Perspective::Server));
        
        let imported_server = *Game::<Direction4>::import_server(exported_server.clone()).unwrap();
        assert_eq!(imported_server, server);
        
        for team in [Perspective::Neutral, Perspective::Team(0), Perspective::Team(1)] {
            println!("testing client import for perspective {:?}", team);
            println!("events: {:?}", events.get(&team));
            let client = crate::game::game::Game::new_client(map.clone(), &settings, events.get(&team).unwrap());
            let hidden = match team {
                Perspective::Team(team) => Some((team, exported_server.hidden.as_ref().unwrap().teams.get(&team).unwrap().clone())),
                _ => None,
            };
            let client_imported = *Game::<Direction4>::import_client(exported_server.public.clone(), hidden).unwrap();
            assert_eq!(client.get_map(), client_imported.get_map());
            assert_eq!(client.get_fog(), client_imported.get_fog());
            assert_eq!(client, client_imported);
        }
    }
}
