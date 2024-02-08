pub mod point;
pub mod point_map;
pub mod direction;
pub mod wrapping_map;
pub mod map;

#[cfg(test)]
mod tests {
    use super::{direction::{Direction, Direction4, Direction6}, wrapping_map::Distortion};


    #[test]
    fn simple_distortions() {
        let distortion = Distortion::neutral();
        for d in Direction4::list() {
            assert_eq!(d, distortion.update_direction(d));
            assert_eq!(d, distortion.update_diagonal_direction(d));
        }

        let distortion = Distortion::new(true, Direction6::D60);
        assert_eq!(distortion.update_direction(Direction6::D0), Direction6::D240);
        assert_eq!(distortion.update_direction(Direction6::D60), Direction6::D180);
        assert_eq!(distortion.update_direction(Direction6::D120), Direction6::D120);
        assert_eq!(distortion.update_direction(Direction6::D180), Direction6::D60);
        assert_eq!(distortion.update_direction(Direction6::D240), Direction6::D0);
        assert_eq!(distortion.update_direction(Direction6::D300), Direction6::D300);
        assert_eq!(distortion.update_diagonal_direction(Direction6::D0), Direction6::D180);
        assert_eq!(distortion.update_diagonal_direction(Direction6::D60), Direction6::D120);
        assert_eq!(distortion.update_diagonal_direction(Direction6::D120), Direction6::D60);
        assert_eq!(distortion.update_diagonal_direction(Direction6::D180), Direction6::D0);
        assert_eq!(distortion.update_diagonal_direction(Direction6::D240), Direction6::D300);
        assert_eq!(distortion.update_diagonal_direction(Direction6::D300), Direction6::D240);
    }
}
