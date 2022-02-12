use crate::map::direction::*;
use crate::map::point_map::*;
use crate::map::point::*;
use std::collections::{HashSet, HashMap};

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct GlobalPoint {
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

type Distortion<D> = (bool, D);
type AreaPoint<D> = (Transformation<D>, Point);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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
    pub fn distortion(&self) -> &Distortion<D> {
        &self.distortion
    }
    pub fn translate_by(&self) -> &D::T {
        &self.translate_by
    }
    fn opposite(&self) -> Self {
        let mut translate_by = self.translate_by.rotate_by(&self.distortion.1.opposite_angle().opposite_direction());
        let angle = if self.distortion.0 {
            translate_by = translate_by.mirror_vertically();
            self.distortion.1
        } else {
            self.distortion.1.opposite_angle()
        };
        Transformation {
            distortion: (self.distortion.0, angle),
            translate_by: translate_by,
        }
    }
    fn add(&self, other: &Self) -> Self {
        let mut translate_by = other.translate_by;
        let mut angle = other.distortion.1;
        if self.distortion.0 {
            angle = angle.opposite_angle();
            translate_by = translate_by.mirror_vertically();
        }
        Transformation{
            distortion: (self.distortion.0 != other.distortion.0, self.distortion.1.rotate_by(&angle)),
            translate_by: self.translate_by.plus(&translate_by.rotate_by(&self.distortion.1)),
        }
    }
    fn subtract(&self, other: &Self) -> Self {
        let mut translate_by = self.translate_by.minus(&other.translate_by).rotate_by(&other.distortion.1.opposite_angle());
        let mut angle = self.distortion.1.rotate_by(&other.distortion.1.opposite_angle());
        if other.distortion.0 {
            angle = angle.opposite_angle();
            translate_by = translate_by.mirror_vertically();
        }
        Transformation{
            distortion: (self.distortion.0 != other.distortion.0, angle),
            translate_by: translate_by,
        }
    }
}

#[derive(Debug)]
pub enum TransformationError<D>
where D: Direction {
    Collision(AreaPoint<D>, Transformation<D>),
    Disconnected,
    TooMany,
    Mirroring,
}

pub struct WrappingMapBuilder<D>
where D: Direction
{
    map: PointMap,
    map_center: Point,
    seed_transformations: Vec<Transformation<D>>,
    adjacent_transformations: Vec<Transformation<D>>, // all transformations that neighbor the center, that can be constructed from the seed_transformations
    wrapped_neighbors: HashMap<(Point, D), OrientedPoint<D>>,
    screen_wrap_options: HashMap<Distortion<D>, Vec<D::T>>, // TODO: pack into a feature called smth like "screen_aware"?
    error: Option<TransformationError<D>>,
}
impl<D> WrappingMapBuilder<D>
where D: Direction
{
    pub fn new(map: PointMap, seed_transformations: Vec<Transformation<D>>) -> Self {
        // remove transformations that can't possibly be connected to the center
        let seed_transformations = seed_transformations.into_iter().filter(|trans| {
            trans.translate_by.len() < 256
        }).collect();
        let mut result = WrappingMapBuilder {
            map_center: Point::new(map.width() / 2, map.height() / 2),
            map,
            seed_transformations,
            adjacent_transformations: vec![],
            wrapped_neighbors: HashMap::new(),
            screen_wrap_options: HashMap::new(),
            error: None,
        };
        if result.seed_transformations.len() > 4 { // TODO: maybe 4 isn't even possible
            result.error = Some(TransformationError::TooMany);
        } else {
            // check if the given transformations are valid and calculate wrapped_neighbors
            let mut area: HashMap<GlobalPoint, AreaPoint<D>> = HashMap::new();
            result.error = result.check_seed_transformations(&mut area)
            .and_then(|_| result.check_transformations(&mut area))
            .and_then(|_| result.search_wrapped_neighbors(&area))
            .err();
        }

        result
    }
    pub fn map(&self) -> &PointMap {
        &self.map
    }
    pub fn seed_transformations(&self) -> &Vec<Transformation<D>> {
        &self.seed_transformations
    }
    pub fn adjacent_transformations(&self) -> &Vec<Transformation<D>> {
        &self.adjacent_transformations
    }
    pub fn screen_wrap_vectors(&self) -> Vec<(f32, f32)> {
        let mut wrap1: Option<D::T> = None;
        let mut wrap2: Option<D::T> = None;
        for list in self.screen_wrap_options.values() {
            for i in 0..list.len() {
                for j in (i + 1)..list.len() {
                    let difference = list[i].minus(&list[j]);
                    if let Some(w1) = wrap1 {
                        if difference.is_parallel(&w1) {
                            if difference.len() < w1.len() {
                                wrap1 = Some(difference)
                            }
                        } else if let Some(w2) = wrap2 {
                            if difference.len() < w2.len() {
                                wrap2 = Some(difference)
                            }
                        } else {
                            wrap2 = Some(difference)
                        }
                    } else {
                        wrap1 = Some(difference);
                    }
                }
            }
        }
        let mut result = vec![];
        if let Some(w1) = wrap1 {
            result.push(w1.screen_coordinates());
            if let Some(w2) = wrap2 {
                result.push(w2.screen_coordinates());
            }
        }
        result
    }
    pub fn err(&self) -> &Option<TransformationError<D>> {
        &self.error
    }
    pub fn build(self) -> Result<WrappingMap<D>, TransformationError<D>> {
        if let Some(err) = self.error {
            Err(err)
        } else {
            Ok(WrappingMap::new(self.map, self.seed_transformations, self.wrapped_neighbors))
        }
    }
    fn odd_if_hex(&self) -> bool {
        self.map.odd_if_hex() == (self.map_center.y() % 2 == 0)
    }
    fn check_seed_transformations(&mut self, area: &mut HashMap<GlobalPoint, AreaPoint<D>>) -> Result<(), TransformationError<D>> {
        self.add_points(area, &Transformation {
            distortion: (false, D::angle_0()),
            translate_by: D::angle_0().translation(0),
        })?;
        for tran in &self.seed_transformations {
            for p in self.map.get_valid_points().iter() {
                let gp = self.transform_point(p, tran);
                if let Some(ap) = area.get(&gp) {
                    // transformation overlaps center
                    return Err(TransformationError::Collision(ap.clone(), tran.clone()));
                }
            }
            if self.find_neighbors(area, tran).len() == 0 {
                // transformation not connected to center
                return Err(TransformationError::Disconnected);
            }
        }
        Ok(())
    }
    fn find_neighbors(&self, area: &mut HashMap<GlobalPoint, AreaPoint<D>>, transformation: &Transformation<D>) -> HashSet<Transformation<D>> {
        let mut result = HashSet::new();
        for p in self.map.get_valid_points().iter() {
            let gp = self.transform_point(p, transformation);
            for d in D::list() {
                let neighbor = d.translation(1).translate_point(&gp, self.odd_if_hex());
                if let Some(ap) = area.get(&neighbor) {
                    result.insert(ap.0.clone());
                }
            }
        }
        result
    }
    fn add_points(&self, area: &mut HashMap<GlobalPoint, AreaPoint<D>>, transformation: &Transformation<D>) -> Result<(), TransformationError<D>> {
        for p in self.map.get_valid_points().iter() {
            let gp = self.transform_point(p, transformation);
            if let Some(result) = area.get(&gp) {
                return Err(TransformationError::Collision(result.clone(), transformation.clone()));
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
    fn check_transformations(&mut self, area: &mut HashMap<GlobalPoint, AreaPoint<D>>) -> Result<Vec<(Transformation<D>, HashSet<(bool, D)>)>, TransformationError<D>> {
        let mut transformations = vec![
            (Transformation {
                distortion: (false, D::angle_0()),
                translate_by: D::angle_0().translation(0),
            }, HashSet::new())
        ];
        for tran in &self.seed_transformations {
            self.adjacent_transformations.push(tran.clone());
            self.adjacent_transformations.push(tran.opposite());
        }
        self.screen_wrap_options.insert(transformations[0].0.distortion, vec![transformations[0].0.translate_by]);
        let mut i = 0;
        while i < transformations.len() {
            let mut at = 0;
            let mut found_new_seed = false;
            while at < self.adjacent_transformations.len() {
                let transformation = &transformations[i];
                let history = &transformation.1;
                if history.contains(&transformation.0.distortion) {
                    break;
                }
                let new_tran = transformation.0.add(&self.adjacent_transformations[at]);
                //println!("{:?} + {:?} = {:?}", &transformed[next_index].0, &t, &new_tran);
                match self.add_points(area, &new_tran) {
                    Ok(_) => {
                        let mut history = history.clone();
                        history.insert(transformation.0.distortion);
                        // check if new transformation is connected to center, thus creating new entry for "transformations"
                        for neighbor in self.find_neighbors(area, &new_tran) {
                            let new_seed = new_tran.subtract(&neighbor);
                            if new_seed != transformations[0].0 && !self.adjacent_transformations.iter().any(|t| t == &new_seed) {
                                println!("found new implied neighbor: {:?}", new_seed);
                                println!("calculated as {:?} - {:?}", new_tran, neighbor);
                                //self.seed_transformations.push(new_seed.clone());
                                self.adjacent_transformations.push(new_seed.opposite());
                                self.adjacent_transformations.push(new_seed);
                                found_new_seed = true;
                            }
                        }
                        if !self.screen_wrap_options.contains_key(&new_tran.distortion) {
                            self.screen_wrap_options.insert(new_tran.distortion, vec![]);
                        }
                        self.screen_wrap_options.get_mut(&new_tran.distortion).unwrap().push(new_tran.translate_by);
                        transformations.push((new_tran, history));
                    },
                    Err(TransformationError::Collision(ap, t)) => {
                        if new_tran != ap.0 {
                            return Err(TransformationError::Collision(ap, t))
                        } else {
                            // found another way to get the same transformation. ignore the new one
                            // TODO: should histories be combined?
                        }
                    },
                    Err(e) => return Err(e),
                }
                at += 1;
            }
            i += 1;
            if found_new_seed {
                i = 0;
            }
        }
        Ok(transformations)
    }
    fn search_wrapped_neighbors(&mut self, area: &HashMap<GlobalPoint, AreaPoint<D>>) -> Result<(), TransformationError<D>> {
        for p in self.map.get_valid_points() {
            let gp = GlobalPoint::new(p.x() as i16, p.y() as i16);
            for d in D::list() {
                let neighbor = d.translation(1).translate_point(&gp, self.odd_if_hex());
                if let Some(ap) = area.get(&neighbor) {
                    self.wrapped_neighbors.insert((p, *d), OrientedPoint::new(ap.1, ap.0.distortion.0, ap.0.distortion.1));
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone)]
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
}

pub struct WrappingMap<D>
where D: Direction {
    point_map: PointMap,
    // only needed to save and load the wrapped_neighbors
    seed_transformations: Vec<Transformation<D>>,
    wrapped_neighbors: HashMap<(Point, D), OrientedPoint<D>>,
}

impl<D> WrappingMap<D>
where D: Direction {
    pub fn new(point_map: PointMap, seed_transformations: Vec<Transformation<D>>, wrapped_neighbors: HashMap<(Point, D), OrientedPoint<D>>) -> Self {
        WrappingMap {
            point_map,
            seed_transformations,
            wrapped_neighbors,
        }
    }
    /*pub fn get_neighbor(&self, point: &Point, direction: &D) -> Option<OrientedPoint<D>> {
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
    }*/
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::point_map::PointMap;

    #[test]
    fn transformations1() {
        let transformations = vec![
            Transformation::new((false, Direction4::D0), Direction4::D0.translation(0)),
            Transformation::new((false, Direction4::D0), Direction4::D0.translation(5)),
            Transformation::new((true, Direction4::D90), Direction4::D180.translation(6)),
            Transformation::new((false, Direction4::D90), Direction4::D90.translation(7)),
            Transformation::new((true, Direction4::D180), Direction4::D0.translation(8)),
            Transformation::new((true, Direction4::D0), Direction4::D0.translation(9).plus(&Direction4::D90.translation(7))),
        ];
        for t in transformations.iter() {
            println!("{:?}", &t);
            println!("{:?}", &t.opposite());
            assert_eq!(t, &transformations[0].add(&t));
            assert_eq!(t, &t.opposite().opposite());
            assert_eq!(transformations[0], t.subtract(&t));
        }
        for t in transformations.iter() {
            for t2 in transformations.iter() {
                println!("{:?}", &t);
                println!("{:?}", &t2);
                println!("{:?}", &t.subtract(&t2));
                assert_eq!(t, &t2.add(&t.subtract(&t2)));
            }
        }
    }

    #[test]
    fn transformations2() {
        assert_eq!(Transformation::new((true, Direction4::D0), Direction4::D0.translation(8).plus(&Direction4::D90.translation(7))).opposite(),
                Transformation::new((true, Direction4::D0), Direction4::D0.translation(8).plus(&Direction4::D90.translation(-7))));
        assert_eq!(Transformation::new((true, Direction4::D90), Direction4::D0.translation(5).plus(&Direction4::D90.translation(-6))).opposite(),
                Transformation::new((true, Direction4::D90), Direction4::D0.translation(-6).plus(&Direction4::D90.translation(5))));
        println!("{:?}", Transformation::new((true, Direction4::D0), Direction4::D0.translation(-994).plus(&Direction4::D90.translation(-20))).opposite());
        assert_eq!(Transformation::new((false, Direction4::D0), Direction4::D0.translation(-990))
                    .subtract(&Transformation::new((true, Direction4::D0), Direction4::D0.translation(-994).plus(&Direction4::D90.translation(-20)))),
                Transformation::new((true, Direction4::D0), Direction4::D0.translation(-4).plus(&Direction4::D90.translation(20))));
    }

    #[test]
    fn no_wrapping() -> Result<(), TransformationError<Direction4>> {
        let builder = WrappingMapBuilder::<Direction4>::new(PointMap::new(4, 8, false), vec![]);
        builder.build()?;
        Ok(())
    }

    #[test]
    fn no_wrapping_hex() -> Result<(), TransformationError<Direction6>> {
        let builder = WrappingMapBuilder::<Direction6>::new(PointMap::new(4, 8, false), vec![]);
        builder.build()?;
        Ok(())
    }

    #[test]
    fn simple_wrapping() -> Result<(), TransformationError<Direction4>> {
        let mut builder = WrappingMapBuilder::<Direction4>::new(PointMap::new(5, 4, false), vec![
            Transformation::new((false, Direction4::D0), Direction4::D0.translation(-5))
        ]);
        let mut area: HashMap<GlobalPoint, AreaPoint<Direction4>> = HashMap::new();
        builder.check_seed_transformations(&mut area)?;
        let transformations = builder.check_transformations(&mut area)?;
        assert_eq!(transformations.len(), 3);

        Ok(())
    }

    #[test]
    fn rotated_wrapping() -> Result<(), TransformationError<Direction4>> {
        let mut builder = WrappingMapBuilder::<Direction4>::new(PointMap::new(5, 4, false), vec![
            Transformation::new((false, Direction4::D90), Direction4::D0.translation(5))
        ]);
        let mut area: HashMap<GlobalPoint, AreaPoint<Direction4>> = HashMap::new();
        builder.check_seed_transformations(&mut area)?;
        let transformations = builder.check_transformations(&mut area)?;
        println!("{:?}", transformations);
        assert_eq!(transformations.len(), 4);
        
        Ok(())
    }

    #[test]
    fn double_wrapping() -> Result<(), TransformationError<Direction4>> {
        let mut builder = WrappingMapBuilder::<Direction4>::new(PointMap::new(5, 4, false), vec![
            Transformation::new((false, Direction4::D0), Direction4::D0.translation(-5).plus(&Direction4::D90.translation(2))),
            Transformation::new((false, Direction4::D0), Direction4::D0.translation(-5).plus(&Direction4::D90.translation(-2))),
        ]);
        let mut area: HashMap<GlobalPoint, AreaPoint<Direction4>> = HashMap::new();
        builder.check_seed_transformations(&mut area)?;
        let transformations = builder.check_transformations(&mut area)?;
        assert_eq!(transformations.len(), 7);

        Ok(())
    }
}
