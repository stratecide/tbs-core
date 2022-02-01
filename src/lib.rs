mod map;

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

    #[test]
    fn no_wrapping() -> Result<(), TransformationError<Direction4>> {
        let builder = WrappingMapBuilder::<Direction4>::new(PointMap::new(4, 8, false), vec![]);
        builder.check_transformations()?;
        Ok(())
    }

    #[test]
    fn simple_wrapping() -> Result<(), TransformationError<Direction4>> {
        let builder = WrappingMapBuilder::<Direction4>::new(PointMap::new(5, 4, false), vec![
            Transformation::new((false, Direction4::D0), Direction4::D0.translation(-5))
        ]);
        let transformations = builder.check_transformations()?;
        assert_eq!(transformations.len(), 25);

        Ok(())
    }

    #[test]
    fn rotated_wrapping() -> Result<(), TransformationError<Direction4>> {
        let builder = WrappingMapBuilder::<Direction4>::new(PointMap::new(5, 4, false), vec![
            Transformation::new((false, Direction4::D90), Direction4::D0.translation(5))
        ]);
        let transformations = builder.check_transformations()?;
        println!("{:?}", transformations);
        assert_eq!(transformations.len(), 4);
        
        Ok(())
    }
}
