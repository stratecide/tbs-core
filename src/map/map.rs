use crate::map::wrapping_map::{WrappingMapBuilder};
use crate::map::point_map::PointMap;
use crate::map::direction::*;
use crate::map::point::Point;


pub struct Map<D>
where D: Direction
{
    //tile_logic: WrappingMap<D>,
    temp: WrappingMapBuilder<D>
}

impl<D> Map<D>
where D: Direction
{
    fn get_neighbor(&self, point: &Point, direction: &D) {
        WrappingMapBuilder::<Direction4>::new(PointMap::new(5, 6, false), vec![]);
    }
}
