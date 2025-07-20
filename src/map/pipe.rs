use zipper::*;
use zipper_derive::Zippable;

use super::direction::*;
use super::wrapping_map::Distortion;


#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Zippable)]
pub struct PipeState<D: Direction> {
    directions: [D; 2],
}

impl<D: Direction> PipeState<D> {
    pub fn new(d1: D, d2: D) -> Option<Self> {
        if d1 == d2 {
            return None;
        }
        Some(Self {
            directions: [d1, d2],
        })
    }

    pub fn directions(&self) -> [D; 2] {
        self.directions
    }

    /**
     * @d: direction that leads into this PipeState
     * return: if d is a valid entry, returns Distortion to apply. None otherwise
     */
    pub fn distortion(&self, entry: D) -> Option<Distortion<D>> {
        let entry = entry.opposite_direction();
        for (i, dir) in self.directions.iter().enumerate() {
            if *dir == entry {
                return Some(Distortion::new(false, self.directions[1 - i].rotate_by(entry.opposite_direction().mirror_vertically())));
            }
        }
        None
    }

    pub fn distort(&mut self, distortion: Distortion<D>) {
        for d in &mut self.directions {
            *d = distortion.update_direction(*d);
        }
    }
}

impl<D: Direction> Default for PipeState<D> {
    fn default() -> Self {
        Self {
            directions: [D::angle_0(), D::angle_0().opposite_direction()],
        }
    }
}


#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::config::config::Config;
    use crate::map::direction::*;
    use crate::map::map::*;
    use crate::map::point::*;
    use crate::map::point_map::PointMap;
    use crate::map::wrapping_map::*;

    use super::PipeState;


    #[test]
    fn pipe_state() {
        let pipe = PipeState::new(Direction4::D180, Direction4::D90).unwrap();
        assert_eq!(pipe.distortion(Direction4::D0), Some(Distortion::new(false, Direction4::D90)));
        assert_eq!(pipe.distortion(Direction4::D0).unwrap().update_direction(Direction4::D0), Direction4::D90);
        assert_eq!(pipe.distortion(Direction4::D270).unwrap().update_direction(Direction4::D270), Direction4::D180);
    }

    #[test]
    fn straight_line() {
        let config = Arc::new(Config::default());
        let map = PointMap::new(8, 5, false);
        let map = WMBuilder::<Direction4>::with_transformations(map, vec![Transformation::new(Distortion::new(false, Direction4::D90), Direction4::D0.translation(6))]).unwrap();
        let mut map = Map::new(map.build(), &config);
        map.set_pipes(Point::new(7, 3), vec![PipeState::new(Direction4::D0, Direction4::D90).unwrap()]);
        map.set_pipes(Point::new(2, 4), vec![PipeState::new(Direction4::D180, Direction4::D90).unwrap()]);
        assert_eq!(
            map.get_neighbor(Point::new(3, 0), Direction4::D90),
            Some((Point::new(7, 2), Distortion::new(false, Direction4::D0)))
        );
        assert_eq!(
            map.get_line(Point::new(3, 1), Direction4::D90, 4, NeighborMode::FollowPipes),
            vec![
                OrientedPoint::new(Point::new(3, 1), false, Direction4::D90),
                OrientedPoint::new(Point::new(3, 0), false, Direction4::D90),
                OrientedPoint::new(Point::new(7, 2), false, Direction4::D90),
                OrientedPoint::new(Point::new(7, 1), false, Direction4::D90),
            ]
        );
        assert_eq!(
            map.get_neighbor(Point::new(1, 4), Direction4::D0),
            Some((Point::new(2, 3), Distortion::new(false, Direction4::D90)))
        );
        assert_eq!(
            map.get_line(Point::new(2, 2), Direction4::D270, 4, NeighborMode::FollowPipes),
            vec![
                OrientedPoint::new(Point::new(2, 2), false, Direction4::D270),
                OrientedPoint::new(Point::new(2, 3), false, Direction4::D270),
                OrientedPoint::new(Point::new(1, 4), false, Direction4::D180),
                OrientedPoint::new(Point::new(0, 4), false, Direction4::D180),
            ]
        );
    }
}
