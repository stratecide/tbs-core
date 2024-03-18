use std::collections::HashSet;

use crate::config::environment::Environment;
use crate::details::Detail;
use crate::terrain::terrain::Terrain;
use crate::units::unit::Unit;

use super::direction::Direction;
use super::map::NeighborMode;
use super::point::Point;
use super::wrapping_map::*;


pub trait MapView<D: Direction> {
    /*fn game(&self) -> Option<&dyn GameView<D>> {
        None
    }*/
    fn environment(&self) -> &Environment;
    fn wrapping_logic(&self) -> &WrappingMap<D>;
    fn all_points(&self) -> Vec<Point>;
    fn get_terrain(&self, p: Point) -> Option<&Terrain>;
    fn get_details(&self, p: Point) -> &[Detail<D>];
    fn get_unit(&self, p: Point) -> Option<&Unit<D>>;

    /**
     * checks the pipe at dp.point for whether it can be entered by dp.direction and if true, returns the position of the next pipe tile
     * returns None if no pipe is at the given location, for example because the previous pipe tile was an exit
     */
    fn next_pipe_tile(&self, point: Point, direction: D) -> Option<(Point, Distortion<D>)> {
        for det in self.get_details(point) {
            match det {
                Detail::Pipe(pipe_state) => {
                    if let Some(disto) = pipe_state.distortion(direction) {
                        return self.wrapping_logic().get_neighbor(point, disto.update_direction(direction))
                        .and_then(|(p, d)| Some((p, disto + d)))
                    }
                }
                _ => (),
            }
        }
        None
    }

    /**
     * the returned Distortion has to be applied to 'd' in order to
     * keep moving in the same direction
     */
    fn get_neighbor(&self, p: Point, d: D) -> Option<(Point, Distortion<D>)> {
        if let Some((point, mut distortion)) = self.wrapping_logic().get_neighbor(p, d) {
            for det in self.get_details(point) {
                match det {
                    Detail::Pipe(pipe_state) => {
                        if !pipe_state.is_open(distortion.update_direction(d).opposite_direction()) {
                            continue;
                        }
                        if let Some(_disto) = pipe_state.distortion(distortion.update_direction(d)) {
                        //if pipe_state.transform_direction(n.1.update_direction(d)).is_some() {
                            //distortion += disto;
                            let mut current = point;
                            while let Some((next, disto)) = self.next_pipe_tile(current, distortion.update_direction(d)) {
                                current = next;
                                distortion += disto;
                                if current == point {
                                    // infinite loop, abort
                                    return None;
                                }
                            }
                            return Some((current, distortion));
                        }
                    }
                    _ => (),
                }
            }
            Some((point, distortion))
        } else {
            None
        }
    }
    
    fn get_neighbors(&self, p: Point, mode: NeighborMode) -> Vec<OrientedPoint<D>> {
        let mut result = vec![];
        for d in D::list() {
            match mode {
                NeighborMode::Direct => {
                    if let Some((p, distortion)) = self.wrapping_logic().get_neighbor(p, d) {
                        result.push(OrientedPoint::new(p, distortion.is_mirrored(), distortion.update_direction(d)));
                    }
                }
                NeighborMode::FollowPipes => {
                    if let Some((p, distortion)) = self.get_neighbor(p, d) {
                        result.push(OrientedPoint::new(p, distortion.is_mirrored(), distortion.update_direction(d)));
                    }
                }
            }
        }
        result
    }

    // the result includes start, the OrientedPoints point towards the next point
    // the result may be shorter than the requested length if not enough points could be found
    fn get_line(&self, start: Point, d: D, length: usize, mode: NeighborMode) -> Vec<OrientedPoint<D>> {
        let mut result = vec![OrientedPoint::new(start, false, d)];
        let mut distortion = Distortion::neutral();
        while result.len() < length {
            let current = result.get(result.len() - 1).unwrap();
            let next = match mode {
                NeighborMode::Direct => self.wrapping_logic().get_neighbor(current.point, distortion.update_direction(d)),
                NeighborMode::FollowPipes => self.get_neighbor(current.point, distortion.update_direction(d)),
            };
            if let Some((p, disto)) = next {
                distortion += disto;
                result.push(OrientedPoint::new(p, distortion.is_mirrored(), distortion.update_direction(d)));
            } else {
                break;
            }
        }
        result
    }

    fn width_search(&self, start: Point, mut f: Box<&mut dyn FnMut(Point) -> bool>) -> HashSet<Point> {
        let mut result = HashSet::new();
        let mut to_check = HashSet::new();
        to_check.insert(start);
        while to_check.len() > 0 {
            let mut next = HashSet::new();
            for p in to_check {
                if f(p) {
                    result.insert(p);
                    for p in self.get_neighbors(p, NeighborMode::Direct) {
                        if !result.contains(&p.point) {
                            next.insert(p.point);
                        }
                    }
                }
            }
            to_check = next;
        }
        result
    }

    fn range_in_layers(&self, center: Point, range: usize) -> Vec<HashSet<Point>> {
        if range == 0 {
            return Vec::new();
        }
        let mut result = Vec::new();
        let mut layer: HashSet<(Point, D, Option<D>)> = HashSet::new();
        for dp in self.get_neighbors(center, NeighborMode::FollowPipes) {
            layer.insert((dp.point, dp.direction, None));
        }
        for _ in 1..range {
            let previous_layer = layer;
            layer = HashSet::new();
            let mut result_layer = HashSet::new();
            for (p, dir, dir_change) in previous_layer {
                result_layer.insert(p);
                if let Some((point, distortion)) = self.get_neighbor(p, dir) {
                    let dir_change = match (distortion.is_mirrored(), dir_change) {
                        (_, None) => None,
                        (true, Some(angle)) => Some(angle.mirror_vertically()),
                        (false, Some(angle)) => Some(angle),
                    };
                    layer.insert((point, distortion.update_direction(dir), dir_change));
                }
                let mut dir_changes = vec![];
                if let Some(dir_change) = dir_change {
                    // if we already have 2 directions, only those 2 directions can find new points
                    dir_changes.push(dir_change);
                } else {
                    // since only one direction has been used so far, try both directions that are directly neighboring
                    let d = *D::list().last().unwrap();
                    dir_changes.push(d.mirror_vertically());
                    dir_changes.push(d);
                }
                for mut dir_change in dir_changes {
                    if let Some((point, distortion)) = self.get_neighbor(p, dir.rotate_by(dir_change)) {
                        if distortion.is_mirrored() {
                            dir_change = dir_change.mirror_vertically();
                        }
                        layer.insert((point, distortion.update_direction(dir), Some(dir_change)));
                    }
                }
            }
            result.push(result_layer);
        }
        let mut result_layer = HashSet::new();
        for (p, _, _) in layer {
            result_layer.insert(p);
        }
        result.push(result_layer);
        result
    }
}

impl<'a, D: Direction, M: MapView<D>> MapView<D> for Box<&'a M> {
    fn environment(&self) -> &Environment {
        (**self).environment()
    }

    fn wrapping_logic(&self) -> &WrappingMap<D> {
        (**self).wrapping_logic()
    }

    fn all_points(&self) -> Vec<Point> {
        (**self).all_points()
    }

    fn get_terrain(&self, p: Point) -> Option<&Terrain> {
        (**self).get_terrain(p)
    }

    fn get_details(&self, p: Point) -> &[Detail<D>] {
        (**self).get_details(p)
    }

    fn get_unit(&self, p: Point) -> Option<&Unit<D>> {
        (**self).get_unit(p)
    }
}
