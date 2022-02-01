use crate::map::direction::*;
use crate::map::point_map::*;
use crate::map::point::*;
use std::collections::{HashSet, HashMap};

#[derive(Clone, PartialEq, Eq, Hash)]
struct GlobalPoint {
    x: i16,
    y: i16,
}

impl Position<i16> for GlobalPoint {
    fn new(x: i16, y: i16) -> Self {
        GlobalPoint {x, y}
    }
    fn x(&self) -> i16 {
        self.x
    }
    fn y(&self) -> i16 {
        self.y
    }
}

type Distortion<D: Direction> = (bool, D);
type AreaPoint<D: Direction> = (Transformation<D>, Point);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transformation<D>
where D: Direction {
    distortion: Distortion<D>,
    translate_by: D::T,
}

impl<D> Transformation<D>
where D: Direction {
    pub fn new(distortion: Distortion<D>, translate_by: D::T) -> Self {
        Transformation {distortion, translate_by}
    }
    fn opposite(&self) -> Self {
        let angle = self.distortion.1.opposite_angle();
        Transformation {
            distortion: (self.distortion.0, angle),
            translate_by: self.translate_by.rotate_by(&angle.opposite_direction()),
        }
    }
    fn add(&self, other: &Self) -> Self {
        Transformation{
            distortion: (self.distortion.0 != other.distortion.0, self.distortion.1.rotate_by(other.distortion.1)),
            translate_by: self.translate_by.plus(&other.translate_by.rotate_by(&self.distortion.1)),
        }
    }
}

/*#[derive(Clone)]
struct MultiTransformation<D, T>
where D: Direction,
    T: Translation<D>
{
    distortion: Distortion<D, T>,
    position: GlobalPoint,
    history: HashSet<Distortion<D, T>>,
}*/
#[derive(Debug)]
pub enum TransformationError<D>
where D: Direction {
    Collision(AreaPoint<D>),
    Disconnected
}

pub struct WrappingMapBuilder<D>
where D: Direction
{
    map: PointMap,
    map_center: Point,
    seed_transformations: Vec<Transformation<D>>,
    /*adjacent_transformations: Vec<Transformation<D>>,
    transformations: HashMap<(Distortion<D>, GlobalPoint), HashSet<Distortion<D>>>,
    area: HashMap<GlobalPoint, AreaPoint<D>>,*/
}
impl<D> WrappingMapBuilder<D>
where D: Direction
{
    pub fn new(map: PointMap, seed_transformations: Vec<Transformation<D>>) -> Self {
        let seed_transformations = seed_transformations.into_iter().filter(|trans| {
            trans.translate_by.len() < 256
        }).collect();
        let map_center = Point::new(map.width() / 2, map.height() / 2);
        WrappingMapBuilder {
            map,
            map_center,
            seed_transformations,
        }
    }
    pub fn odd_if_hex(&self) -> bool {
        self.map.odd_if_hex() == (self.map_center.y() % 2 == 0)
    }
    // returns false if any seed transformation either overlaps the center or isn't neighboring any of the center's points
    pub fn are_seed_transformations_adjacent_to_center(&self) -> Result<(), TransformationError<D>> {
        let mut area: HashMap<GlobalPoint, AreaPoint<D>> = HashMap::new();
        self.add_points(&mut area, &Transformation {
            distortion: (false, D::angle_0()),
            translate_by: D::angle_0().translation(0),
        });
        for tran in &self.seed_transformations {
            let mut is_neigbor = false;
            for p in self.map.get_valid_points().iter() {
                let gp = self.transform_point(p, tran);
                if let Some(ap) = area.get(&gp) {
                    // transformation overlaps center
                    return Err(TransformationError::Collision(ap.clone()));
                }
                if !is_neigbor {
                    for d in D::list() {
                        let neighbor = d.translation(1).translate_point(&gp, self.odd_if_hex());
                        if area.contains_key(&neighbor) {
                            is_neigbor = true;
                            break;
                        }
                    }
                }
            }
            if !is_neigbor {
                // transformation not connected to center
                return Err(TransformationError::Disconnected);
            }
        }
        Ok(())
    }
    fn add_points(&self, area: &mut HashMap<GlobalPoint, AreaPoint<D>>, transformation: &Transformation<D>) -> Result<(), TransformationError<D>> {
        for p in self.map.get_valid_points().iter() {
            let gp = self.transform_point(p, transformation);
            if let Some(result) = area.get(&gp) {
                return Err(TransformationError::Collision(result.clone()));
            }
            area.insert(gp, (transformation.clone(), p.clone()));
        }
        Ok(())
    }
    fn transform_point(&self, p: &Point, transformation: &Transformation<D>) -> GlobalPoint {
        let mut x = p.x() as i16 - self.map_center.x() as i16;
        let y = p.y() as i16 - self.map_center.y() as i16;
        if transformation.distortion.0 {
            // mirrored
            x = -x;
            if self.map.width() % 2 == 0 {
                x += 1;
            }
        }
        let p = GlobalPoint::new(x, y);
        let p = transformation.distortion.1.rotate_around_center(&p, &GlobalPoint::new(0, 0), self.odd_if_hex());
        let p = transformation.translate_by.translate_point(&p, self.odd_if_hex());
        p
    }
    pub fn check_transformations(&self) -> Result<Vec<Transformation<D>>, TransformationError<D>> {
        let mut area: HashMap<GlobalPoint, AreaPoint<D>> = HashMap::new();
        let mut transformed = vec![
            (Transformation {
                distortion: (false, D::angle_0()),
                translate_by: D::angle_0().translation(0),
            }, 0)
        ];
        self.add_points(&mut area, &transformed[0].0)?;
        let mut transformations = vec![];
        for tran in &self.seed_transformations {
            transformations.push(tran.clone());
            transformations.push(tran.opposite());
        }
        let mut next_index = 0;
        while next_index < transformed.len() {
            let step = transformed[next_index].1;
            if step >= 12 {
                break;
            }
            for t in &transformations {
                let new_tran = transformed[next_index].0.add(t);
                //println!("{:?} + {:?} = {:?}", &transformed[next_index].0, &t, &new_tran);
                match self.add_points(&mut area, &new_tran) {
                    Ok(_) => {
                        transformed.push((new_tran, step + 1));
                        // TODO: check if new transformation is connected to center, thus creating new entry for "transformations"
                    },
                    Err(TransformationError::Collision(ap)) => {
                        if new_tran != ap.0 {
                            return Err(TransformationError::Collision(ap))
                        } else {
                            // found another way to get the same transformation. ignore the new one
                            // TODO: should keep the one with lower step
                        }
                    },
                    Err(e) => return Err(e),
                }
            }
            next_index += 1;
        }
        Ok(transformed.into_iter().map(|(tran, _)| {
            tran
        }).collect())
    }
    /*fn update(&mut self) {
        self.transformations = HashMap::new();
        self.adjacent_transformations = self.seed_transformations.clone();
        self.area = HashMap::new();

        if self.seed_transformations.len() == 0 {
            return;
        }
        self.add_transformation(
            MultiTransformation{
                distortion: (false, D::axis_x()),
                position: GlobalPoint::new(0, 0),
                history: HashSet::new(),
            }
        );
    }
    fn add_transformation(&mut self, t: MultiTransformation<D>) {
        let key = (t.distortion, t.position);
        if self.transformations.contains_key(&key) {
            return;
        }
        self.transformations.insert(key, t.history);
    }
    fn add_area_point(&mut self, x: i16, y: i16, data: AreaPoint<D>) {

    }
    fn apply_distortion(&self, original: MultiTransformation<D>, by: &Transformation<D, T>) -> MultiTransformation<D> {
        let distortion:Distortion<D> = (
            original.distortion.0 == by.distortion.0,
            original.distortion.1.rotate_by(by.distortion.1),
        );

        let mut position = by.translate_by.translate_point(&original.position, self.offset_y % 2 != 0);

        let mut history = original.history.clone();
        history.insert(distortion);
        MultiTransformation{
            distortion,
            position,
            history,
        }
    }*/
}

/*#[derive(Clone)]
pub struct OrientedPoint<D>
where D: Direction {
    point: Point,
    mirrored: bool,
    direction: D,
}
impl<D>OrientedPoint<D>
where D: Direction {
    pub fn new(point: Point, mirrored: bool, direction: D) -> Self {
        OrientedPoint{point, mirrored, direction}
    }
}*/

/*pub struct WrappingMap<D>
where D: Direction {
    point_map: PointMap,
    // only needed to save and load the wrapped_neighbors
    seed_transformations: Vec<Transformation<D>>,
    wrapped_neighbors: HashMap<(Point, D), OrientedPoint<D>>,
}

impl<D> WrappingMap<D>
where D: Direction {
    pub fn get_neighbor(&self, point: &Point, direction: &D) -> Option<OrientedPoint<D>> {
        if !self.point_map.is_point_valid(point) {
            None
        } else {
            direction.get_neighbor(point)
            .filter(|point| {
                self.point_map.is_point_valid(&point)
            })
            .map_or_else(|| {
                self.wrapped_neighbors.get(&(point.clone(), *direction))
                .map(|op| {
                    op.clone()
                })
            }, |point| {
                Some(OrientedPoint::new(point, false, *direction))
            })
        }
    }
}*/

/*impl WMap<Direction4> for WrappingMap<Direction4> {
    fn follow_rotated_point(&self, point: &Point, rotate_right_to: &Direction4) -> Point {
        match rotate_right_to {
            Direction4::RIGHT => Point::new(point.x(), point.y()),
            Direction4::UP => Point::new(point.y(), self.point_map.width() - point.x()),
            Direction4::LEFT => Point::new(self.point_map.width() - point.x(), self.point_map.height() - point.y()),
            Direction4::DOWN => Point::new(self.point_map.height() - point.y(), point.x()),
        }
    }
    fn rotated_point_map(&self, rotate_right_to: &Direction4) -> PointMap {
        let mut result = match rotate_right_to {
            Direction4::RIGHT => PointMap::new(self.point_map.width(), self.point_map.height()),
            Direction4::LEFT => PointMap::new(self.point_map.width(), self.point_map.height()),
            Direction4::UP => PointMap::new(self.point_map.height(), self.point_map.width()),
            Direction4::DOWN => PointMap::new(self.point_map.height(), self.point_map.width()),
        };
        for x in 0..self.point_map.width() {
            for y in 0..self.point_map.height() {
                let origin = Point::new(x, y);
                let destination = self.follow_rotated_point(&origin, &rotate_right_to);
                result.set_valid(&destination, self.point_map.is_point_valid(&origin));
            }
        }
        result
    }
}
impl WMap<Direction6> for WrappingMap<Direction6> {
    fn follow_rotated_point(&self, point: &Point, rotate_right_to: &Direction6) -> Point {
        match rotate_right_to {
            Direction6::RIGHT => Point::new(point.x(), point.y()),
            Direction6::UP => Point::new(point.y(), self.point_map.width() - point.x()),
            Direction6::LEFT => Point::new(self.point_map.width() - point.x(), self.point_map.height() - point.y()),
            Direction6::DOWN => Point::new(self.point_map.height() - point.y(), point.x()),
        }
        Point::new(point.x(), point.y())
    }
    fn rotated_point_map(&self, rotate_right_to: &Direction6) -> PointMap {
        let mut result = match rotate_right_to {
            Direction6::RIGHT => PointMap::new(self.point_map.width(), self.point_map.height()),
            Direction6::LEFT => PointMap::new(self.point_map.width(), self.point_map.height()),
            Direction6::UP => PointMap::new(self.point_map.height(), self.point_map.width()),
            Direction6::DOWN => PointMap::new(self.point_map.height(), self.point_map.width()),
        };
        for x in 0..self.point_map.width() {
            for y in 0..self.point_map.height() {
                let origin = Point::new(x, y);
                let destination = self.follow_rotated_point(&origin, &rotate_right_to);
                result.set_valid(&destination, self.point_map.is_point_valid(&origin));
            }
        }
        result
        PointMap::new(self.point_map.width(), self.point_map.height())
    }
}*/

