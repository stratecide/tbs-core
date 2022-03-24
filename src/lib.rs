pub mod map;
pub mod player;
pub mod terrain;
pub mod units;

#[cfg(test)]
mod tests {
    use super::map::wrapping_map::*;
    use super::map::point_map::PointMap;
    use super::map::point::*;
    use super::map::direction::*;

    #[test]
    fn filled_point_map() {
        let map = PointMap::new(5, 6, false);
        assert_eq!(map.width(), 5);
        assert_eq!(map.height(), 6);
        for x in 0..5 {
            for y in 0..6 {
                assert!(map.is_point_valid(&Point::new(x, y)));
            }
            assert!(!map.is_point_valid(&Point::new(x, 6)));
        }
        for y in 0..6 {
            assert!(!map.is_point_valid(&Point::new(5, y)));
        }
    }

}
