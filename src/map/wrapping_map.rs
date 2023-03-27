use crate::map::direction::*;
use crate::map::point_map::*;
use crate::map::point::*;
use std::collections::{HashSet, HashMap};

use zipper::*;
use zipper::zipper_derive::*;

pub type Distortion<D> = (bool, D);
type AreaPoint<D> = (Transformation<D>, Point);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Zippable)]
pub struct Transformation<D>
where D: Direction {
    distortion: Distortion::<D>,
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
    pub fn opposite(&self) -> Self {
        let mut translate_by = self.translate_by.rotate_by(self.distortion.1.mirror_vertically().opposite_direction());
        let angle = if self.distortion.0 {
            translate_by = translate_by.mirror_horizontally();
            self.distortion.1
        } else {
            self.distortion.1.mirror_vertically()
        };
        Transformation {
            distortion: (self.distortion.0, angle),
            translate_by: translate_by,
        }
    }
    pub fn add(&self, other: &Self) -> Self {
        let mut translate_by = other.translate_by;
        let mut angle = other.distortion.1;
        if self.distortion.0 {
            angle = angle.mirror_vertically();
            translate_by = translate_by.mirror_horizontally();
        }
        Transformation{
            distortion: (self.distortion.0 != other.distortion.0, self.distortion.1.rotate_by(angle)),
            translate_by: self.translate_by.plus(&translate_by.rotate_by(self.distortion.1)),
        }
    }
    fn subtract(&self, other: &Self) -> Self {
        let mut translate_by = self.translate_by.minus(&other.translate_by).rotate_by(other.distortion.1.mirror_vertically());
        let mut angle = self.distortion.1.rotate_by(other.distortion.1.mirror_vertically());
        if other.distortion.0 {
            angle = angle.mirror_vertically();
            translate_by = translate_by.mirror_horizontally();
        }
        Transformation{
            distortion: (self.distortion.0 != other.distortion.0, angle),
            translate_by: translate_by,
        }
    }
    pub fn transform_point(&self, p: &GlobalPoint, map_center: &GlobalPoint, odd: bool) -> GlobalPoint {
        let mut x = p.x() - map_center.x();
        let y = p.y() - map_center.y();
        if self.distortion.0 {
            // mirrored
            x = -x;
            if D::is_hex() && y % 2 != 0 {
                if odd {
                    x += 1;
                } else {
                    x -= 1;
                }
            }
        }
        let p = GlobalPoint::new(x, y);
        let p = self.distortion.1.rotate_around_center(&p, &GlobalPoint::new(0, 0), odd);
        let p = self.translate_by.translate_point(&p, odd);
        GlobalPoint::new(p.x() + map_center.x(), p.y() + map_center.y())
    }
}

#[derive(Debug, Clone)]
pub enum TransformationError<D>
where D: Direction {
    Collision(AreaPoint<D>, Transformation<D>),
    Disconnected,
    TooMany,
    Mirroring,
    DuplicateSeed,
}

#[derive(Clone)]
pub struct WrappingMapBuilder<D>
where D: Direction
{
    map: PointMap,
    map_center: Point,
    missing_map_neighbor_points: Vec<GlobalPoint>,
    seed_transformations: Vec<Transformation<D>>,
    adjacent_transformations: Vec<Transformation<D>>, // all transformations that neighbor the center, that can be constructed from the seed_transformations
    wrapped_neighbors: HashMap<(Point, D), OrientedPoint<D>>,
    #[cfg(feature = "rendering")]
    screen_wrap_options: HashMap<Distortion<D>, Vec<D::T>>,
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
            missing_map_neighbor_points: vec![],
            seed_transformations,
            adjacent_transformations: vec![],
            wrapped_neighbors: HashMap::new(),
            #[cfg(feature = "rendering")]
            screen_wrap_options: HashMap::new(),
            error: None,
        };
        let mut area: HashMap<GlobalPoint, AreaPoint<D>> = HashMap::new();
        result.add_points(&mut area, &Transformation::new((false, D::angle_0()), D::angle_0().translation(0))).unwrap();
        result.generate_map_neighbor_points(&area);
        result.check_validity();
        result
    }
    fn check_validity(&mut self) {
        self.adjacent_transformations = vec![];
        self.wrapped_neighbors = HashMap::new();
        #[cfg(feature = "rendering")]
        {
            self.screen_wrap_options = HashMap::new();
        }
        if self.seed_transformations.len() > 4 { // TODO: maybe 4 isn't even possible
            self.error = Some(TransformationError::TooMany);
        } else {
            // check if the given transformations are valid and calculate wrapped_neighbors
            let mut area: HashMap<GlobalPoint, AreaPoint<D>> = HashMap::new();
            self.add_points(&mut area, &Transformation::new((false, D::angle_0()), D::angle_0().translation(0))).unwrap();
            self.error = self.check_seed_transformations(&mut area)
            .and_then(|_| self.check_transformations(&mut area))
            .and_then(|_| self.search_wrapped_neighbors(&area))
            .err();
        }
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

    #[cfg(feature = "rendering")]
    pub fn screen_wrap_vectors(&self) -> Vec<D::T> {
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
            result.push(w1);
            if let Some(w2) = wrap2 {
                result.push(w2);
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
            Ok(WrappingMap::new(self))
        }
    }

    pub fn odd_if_hex(&self) -> bool {
        self.map.odd_if_hex() == (self.map_center.y() % 2 == 0)
    }
    pub fn update_transformation(&mut self, index: usize, transformation: Transformation<D>) {
        if self.seed_transformations.len() > index {
            self.seed_transformations[index] = transformation;
            self.check_validity();
        }
    }
    pub fn remove_transformation(&mut self, index: usize) {
        if self.seed_transformations.len() > index {
            self.seed_transformations.remove(index);
            self.check_validity();
        }
    }
    pub fn add_transformation(&mut self, transformation: Transformation<D>) {
        if self.seed_transformations.len() < 4 {
            self.seed_transformations.push(transformation);
            self.check_validity();
        }
    }
    fn generate_map_neighbor_points(&mut self, area: &HashMap<GlobalPoint, AreaPoint<D>>) {
        let transformation = Transformation::new((false, D::angle_0()), D::angle_0().translation(0));
        for p in self.map.get_valid_points().iter() {
            let gp = self.transform_point(p.x() as i16, p.y() as i16, &transformation);
            for d in D::list() {
                let neighbor = d.translation(1).translate_point(&gp, self.odd_if_hex());
                if let None = area.get(&neighbor) {
                    self.missing_map_neighbor_points.push(neighbor);
                }
            }
        }
    }
    fn check_seed_transformations(&mut self, area: &mut HashMap<GlobalPoint, AreaPoint<D>>) -> Result<(), TransformationError<D>> {
        for i in 0..self.seed_transformations.len() {
            for j in i+1..self.seed_transformations.len() {
                if self.seed_transformations[i] == self.seed_transformations[j] || self.seed_transformations[i] == self.seed_transformations[j].opposite() {
                    return Err(TransformationError::DuplicateSeed)
                }
            }
        }
        for tran in &self.seed_transformations {
            for p in self.map.get_valid_points().iter() {
                let gp = self.transform_point(p.x() as i16, p.y() as i16, tran);
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
        for p in &self.missing_map_neighbor_points {
            let gp = self.transform_point(p.x() + self.map_center.x() as i16, p.y() + self.map_center.y() as i16, transformation);
            if let Some(ap) = area.get(&gp) {
                result.insert(ap.0.clone());
            }
        }
        result
    }
    pub fn is_point_in_transformation(&self, point: &GlobalPoint, transformation: &Transformation<D>) -> bool {
        for p in self.map.get_valid_points().iter() {
            if point == &self.transform_point(p.x() as i16, p.y() as i16, transformation) {
                return true;
            }
        }
        false
    }
    fn add_points(&self, area: &mut HashMap<GlobalPoint, AreaPoint<D>>, transformation: &Transformation<D>) -> Result<(), TransformationError<D>> {
        for p in self.map.get_valid_points().iter() {
            let gp = self.transform_point(p.x() as i16, p.y() as i16, transformation);
            if let Some(result) = area.get(&gp) {
                return Err(TransformationError::Collision(result.clone(), transformation.clone()));
            }
            area.insert(gp, (transformation.clone(), p.clone()));
        }
        Ok(())
    }
    fn transform_point(&self, x: i16, y: i16, transformation: &Transformation<D>) -> GlobalPoint {
        let mut x = x as i16 - self.map_center.x() as i16;
        let y = y as i16 - self.map_center.y() as i16;
        if transformation.distortion.0 {
            // mirrored
            x = -x;
            if D::is_hex() && y % 2 != 0 {
                if self.odd_if_hex() {
                    x += 1;
                } else {
                    x -= 1;
                }
            }
        }
        let p = GlobalPoint::new(x, y);
        let p = transformation.distortion.1.rotate_around_center(&p, &GlobalPoint::new(0, 0), self.odd_if_hex());
        let p = transformation.translate_by.translate_point(&p, self.odd_if_hex());
        p
    }
    #[cfg(feature = "rendering")]
    pub fn rendered_transformations(&self) -> Vec<Transformation<D>> {
        let mut transformations = vec![
            Transformation {
                distortion: (false, D::angle_0()),
                translate_by: D::angle_0().translation(0),
            }
        ];
        let adjacent_transformations = if self.error.is_none() {
            self.adjacent_transformations.clone()
        } else {
            let mut adjacent_transformations = vec![];
            for tran in &self.seed_transformations {
                adjacent_transformations.push(tran.clone());
            }
            for tran in &self.seed_transformations {
                adjacent_transformations.push(tran.opposite());
            }
            adjacent_transformations
        };
        let loop_limit = if self.seed_transformations.len() > 3 {
            2 // if there are 4 seed transformations, no way do we need 3 loops to see a conflict
        } else {
            // with this, there can be at most 186 transformations:
            // 6 in the first round
            // 30 = 6 * 5 in the second (one of the transformations is the inverse, thus ending up where we were before)
            // 150 = 30 * f in the third and last round
            3
        };
        for i in 0..loop_limit {
            for glob in transformations.clone() {
                for relative in &adjacent_transformations {
                    let new_tran = glob.add(relative);
                    if i == 0 || transformations.iter().find(|t| *t == &new_tran).is_none() {
                        transformations.push(new_tran);
                    }
                }
            }
        }
        let wrapping: Vec<(f32, f32)> = self.screen_wrap_vectors().into_iter()
            .map(|t| t.screen_coordinates())
            .collect();
        transformations.into_iter().enumerate().filter(|(i, tran)| {
            if *i == 0 {
                return false;
            } else if *i <= self.seed_transformations.len() {
                return true;
            }
            let center = tran.translate_by.screen_coordinates();
            if wrapping.len() == 1 {
                //wraps in only 1 direction
                let factor = (center.0 * wrapping[0].0 + center.1 * wrapping[0].1) / (wrapping[0].0 * wrapping[0].0 + wrapping[0].1 * wrapping[0].1);
                factor.abs() < 1.5
            } else if wrapping.len() == 2 {
                // wrap in 2 directions
                let distance = (center.0 * center.0 + center.1 * center.1).sqrt();
                let dir = (center.0 / distance, center.1 / distance);
                let normal = (dir.1, -dir.0);
                let factor1 = normal.0 * wrapping[1].0 + normal.1 * wrapping[1].1;
                let factor2 = -normal.0 * wrapping[0].0 - normal.1 * wrapping[0].1;
                let total_factor = distance / (dir.0 * (factor1 * wrapping[0].0 + factor2 * wrapping[1].0) + dir.1 * (factor1 * wrapping[0].1 + factor2 * wrapping[1].1));
                (factor1 * total_factor).abs() < 1.5 && (factor2 * total_factor).abs() < 1.5
            } else {
                // no wrapping
                true
            }
        }).map(|(_, tran)| {
            tran
        }).collect()
    }
    fn check_transformations(&mut self, area: &mut HashMap<GlobalPoint, AreaPoint<D>>) -> Result<Vec<(Transformation<D>, HashSet<Distortion<D>>)>, TransformationError<D>> {
        //let mut neigbor_search = Duration::new(0, 0);
        let mut transformations = vec![
            (Transformation {
                distortion: (false, D::angle_0()),
                translate_by: D::angle_0().translation(0),
            }, HashSet::with_capacity(12))
        ];
        for tran in &self.seed_transformations {
            self.adjacent_transformations.push(tran.clone());
        }
        for tran in &self.seed_transformations {
            self.adjacent_transformations.push(tran.opposite());
        }
        #[cfg(feature = "rendering")]
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
                        let  neighbors = self.find_neighbors(area, &new_tran);
                        for neighbor in neighbors {
                            let new_seed = new_tran.subtract(&neighbor);
                            if new_seed != transformations[0].0 && !self.adjacent_transformations.iter().any(|t| t == &new_seed) {
                                //println!("found new implied neighbor: {:?}", new_seed);
                                //println!("calculated as {:?} - {:?}", new_tran, neighbor);
                                let opp = new_seed.opposite();
                                self.adjacent_transformations.push(new_seed);
                                if !self.adjacent_transformations.iter().any(|t| t == &opp) {
                                    self.adjacent_transformations.push(opp);
                                }
                                found_new_seed = true;
                            }
                        }
                        #[cfg(feature = "rendering")]
                        {
                            if !self.screen_wrap_options.contains_key(&new_tran.distortion) {
                                self.screen_wrap_options.insert(new_tran.distortion, vec![]);
                            }
                            self.screen_wrap_options.get_mut(&new_tran.distortion).unwrap().push(new_tran.translate_by);
                        }
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
            let gp = GlobalPoint::new(p.x() as i16 - self.map_center.x() as i16, p.y() as i16 - self.map_center.y() as i16);
            for d in D::list() {
                let neighbor = d.translation(1).translate_point(&gp, self.odd_if_hex());
                if let Some(ap) = area.get(&neighbor) {
                    if ap.0.translate_by().len() > 0 {
                        let mut direction = d.rotate_by(ap.0.distortion.1.mirror_vertically());
                        if ap.0.distortion.0 {
                            direction = direction.mirror_horizontally();
                        }
                        self.wrapped_neighbors.insert((p, d), OrientedPoint::new(ap.1, ap.0.distortion.0, direction));
                    }
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OrientedPoint<D>
where D: Direction {
    pub point: Point,
    pub mirrored: bool,
    pub direction: D,
}
impl<D>OrientedPoint<D>
where D: Direction {
    pub fn new(point: Point, mirrored: bool, direction: D) -> Self {
        OrientedPoint{point, mirrored, direction}
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct WrappingMap<D>
where D: Direction {
    pointmap: PointMap,
    // only needed to save and load the wrapped_neighbors
    seed_transformations: Vec<Transformation<D>>,
    wrapped_neighbors: HashMap<(Point, D), OrientedPoint<D>>,
#[cfg(feature = "rendering")]
    screen_wrap_vectors: Vec<D::T>,
}

impl<D> WrappingMap<D>
where D: Direction {
    fn new(builder: WrappingMapBuilder<D>) -> Self {
        WrappingMap {
#[cfg(feature = "rendering")]
            screen_wrap_vectors: builder.screen_wrap_vectors(),
            pointmap: builder.map,
            seed_transformations: builder.seed_transformations,
            wrapped_neighbors: builder.wrapped_neighbors,
        }
    }

#[cfg(feature = "rendering")]
    pub fn screen_wrap_vectors(&self) -> &Vec<D::T> {
        &self.screen_wrap_vectors
    }

    pub fn pointmap(&self) -> &PointMap {
        &self.pointmap
    }

    /*pub fn odd_if_hex(&self) -> bool {
        self.pointmap.odd_if_hex() == ((self.pointmap.height() / 2) % 2 == 0)
    }*/

    pub fn seed_transformations(&self) -> &Vec<Transformation<D>> {
        &self.seed_transformations
    }

    pub fn get_neighbor(&self, point: Point, direction: D) -> Option<OrientedPoint<D>> {
        if !self.pointmap.is_point_valid(point) {
            None
        } else {
            direction.get_neighbor(point, self.pointmap.odd_if_hex())
            .filter(|point| {
                self.pointmap.is_point_valid(*point)
            })
            .map_or_else(|| {
                self.wrapped_neighbors.get(&(point.clone(), direction))
                .map(|op| {
                    op.clone()
                })
            }, |point| {
                Some(OrientedPoint::new(point, false, direction))
            })
        }
    }
}

impl<D: Direction> Zippable for WrappingMap<D> {
    fn export(&self, zipper: &mut Zipper) {
        self.pointmap.export(zipper);
        zipper.write_u8(self.seed_transformations.len() as u8, 3);
        for trans in &self.seed_transformations {
            trans.export(zipper);
        }
    }
    fn import(unzipper: &mut Unzipper) -> Result<Self, ZipperError> {
        let pointmap = PointMap::import(unzipper)?;
        let len = unzipper.read_u8(3)?;
        let mut seed_transformations = vec![];
        for _ in 0..len {
            seed_transformations.push(Transformation::import(unzipper)?);
        }
        let builder = WrappingMapBuilder::new(pointmap, seed_transformations);
        if let Ok(result) = builder.build() {
            Ok(result)
        } else {
            Err(ZipperError::InconsistentData)
        }
    }
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
        builder.add_points(&mut area, &Transformation::new((false, Direction4::D0), Direction4::D0.translation(0))).unwrap();
        builder.check_seed_transformations(&mut area)?;
        let transformations = builder.check_transformations(&mut area)?;
        assert_eq!(transformations.len(), 3);

        Ok(())
    }

    #[test]
    fn mirrored_wrapping() -> Result<(), TransformationError<Direction4>> {
        let mut builder = WrappingMapBuilder::<Direction4>::new(PointMap::new(5, 4, false), vec![
            Transformation::new((true, Direction4::D0), Direction4::D0.translation(-5))
        ]);
        let mut area: HashMap<GlobalPoint, AreaPoint<Direction4>> = HashMap::new();
        builder.add_points(&mut area, &Transformation::new((false, Direction4::D0), Direction4::D0.translation(0))).unwrap();
        builder.check_seed_transformations(&mut area)?;
        let transformations = builder.check_transformations(&mut area)?;
        assert_eq!(transformations.len(), 2);

        Ok(())
    }

    #[test]
    fn rotated_wrapping() -> Result<(), TransformationError<Direction4>> {
        let mut builder = WrappingMapBuilder::<Direction4>::new(PointMap::new(5, 4, false), vec![
            Transformation::new((false, Direction4::D90), Direction4::D0.translation(5))
        ]);
        let mut area: HashMap<GlobalPoint, AreaPoint<Direction4>> = HashMap::new();
        builder.add_points(&mut area, &Transformation::new((false, Direction4::D0), Direction4::D0.translation(0))).unwrap();
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
        builder.add_points(&mut area, &Transformation::new((false, Direction4::D0), Direction4::D0.translation(0))).unwrap();
        builder.check_seed_transformations(&mut area)?;
        let transformations = builder.check_transformations(&mut area)?;
        assert_eq!(transformations.len(), 7);

        Ok(())
    }
}
