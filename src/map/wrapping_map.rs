use crate::map::direction::*;
use crate::map::point_map::*;
use crate::map::point::*;
use std::collections::{HashSet, HashMap};
use std::ops::{Neg, Add, AddAssign, Sub, SubAssign};

use zipper::*;
use zipper::zipper_derive::*;

pub const MAX_TRANSFORMATIONS: usize = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Zippable)]
pub struct Distortion<D: Direction> {
    // mirrored horizontally, i.e. X becomes -X while Y is unaffected
    mirrored: bool,
    // the map gets mirrored before it gets rotated
    rotation: D,
}

impl<D: Direction> Distortion<D> {
    pub fn new(mirrored: bool, rotation: D) -> Self {
        Self {
            mirrored,
            rotation,
        }
    }
    pub fn neutral() -> Self {
        Self {
            mirrored: false,
            rotation: D::angle_0(),
        }
    }

    pub fn is_mirrored(&self) -> bool {
        self.mirrored
    }
    pub fn get_mirrored_mut(&mut self) -> &mut bool {
        &mut self.mirrored
    }

    pub fn get_rotation(&self) -> D {
        self.rotation
    }
    pub fn get_rotation_mut(&mut self) -> &mut D {
        &mut self.rotation
    }

    pub fn update_direction(&self, direction: D) -> D {
        let mut direction = direction.rotate_by(self.rotation.mirror_vertically());
        if self.mirrored {
            direction = direction.mirror_horizontally();
        }
        direction
    }

    pub fn update_diagonal_direction(&self, direction: D) -> D {
        let mut direction = direction.rotate_by(self.rotation.mirror_vertically());
        if self.mirrored {
            direction = direction.mirror_horizontally().rotate(true);
        }
        direction
    }
}

impl<D: Direction> Neg for Distortion<D> {
    type Output = Self;
    fn neg(mut self) -> Self::Output {
        if !self.mirrored {
            self.rotation = self.rotation.mirror_vertically()
        };
        self
    }
}

impl<D: Direction> Add for Distortion<D> {
    type Output = Self;
    fn add(self, mut rhs: Self) -> Self::Output {
        if self.mirrored {
            rhs.rotation = rhs.rotation.mirror_vertically();
        }
        Distortion::new(self.mirrored != rhs.mirrored, self.rotation.rotate_by(rhs.rotation))
    }
}

impl<D: Direction> AddAssign for Distortion<D> {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl<D: Direction> Sub for Distortion<D> {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self::Output {
        self + rhs.neg()
    }
}

impl<D: Direction> SubAssign for Distortion<D> {
    fn sub_assign(&mut self, rhs: Self) {
        *self = *self - rhs;
    }
}

type AreaPoint<D> = (Transformation<D>, Point);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Zippable)]
#[zippable(support = u16)]
pub struct Transformation<D>
where D: Direction {
    // translation isn't affected by self.distortion
    pub translate_by: D::T,
    pub distortion: Distortion<D>,
}

impl<D> Transformation<D>
where D: Direction {
    pub fn new(distortion: Distortion<D>, translate_by: D::T) -> Self {
        Transformation {distortion, translate_by}
    }
    pub fn distortion(&self) -> Distortion<D> {
        self.distortion
    }
    pub fn translate_by(&self) -> &D::T {
        &self.translate_by
    }

    /*pub fn opposite(&self) -> Self {
        let mut translate_by = self.translate_by.rotate_by(self.distortion.rotation.mirror_vertically().opposite_direction());
        let angle = if self.distortion.mirrored {
            translate_by = translate_by.mirror_horizontally();
            self.distortion.rotation
        } else {
            self.distortion.rotation.mirror_vertically()
        };
        Transformation {
            distortion: Distortion::new(self.distortion.mirrored, angle),
            translate_by,
        }
    }
    pub fn add(&self, other: &Self) -> Self {
        let mut translate_by = other.translate_by;
        let mut angle = other.distortion.rotation;
        if self.distortion.mirrored {
            angle = angle.mirror_vertically();
            translate_by = translate_by.mirror_horizontally();
        }
        Transformation{
            distortion: Distortion::new(self.distortion.mirrored != other.distortion.mirrored, self.distortion.rotation.rotate_by(angle)),
            translate_by: self.translate_by.plus(&translate_by.rotate_by(self.distortion.rotation)),
        }
    }
    fn subtract(&self, other: &Self) -> Self {
        let mut translate_by = self.translate_by.minus(&other.translate_by).rotate_by(other.distortion.rotation.mirror_vertically());
        let mut angle = self.distortion.rotation.rotate_by(other.distortion.rotation.mirror_vertically());
        if other.distortion.mirrored {
            angle = angle.mirror_vertically();
            translate_by = translate_by.mirror_horizontally();
        }
        Transformation{
            distortion: Distortion::new(self.distortion.mirrored != other.distortion.mirrored, angle),
            translate_by,
        }
    }*/

    pub fn transform_point(&self, p: &GlobalPoint, map_center: &GlobalPoint, odd: bool) -> GlobalPoint {
        let mut x = p.x() - map_center.x();
        let y = p.y() - map_center.y();
        if self.distortion.mirrored {
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
        let p = self.distortion.rotation.rotate_around_center(&p, &GlobalPoint::new(0, 0), odd);
        let p = self.translate_by.translate_point(&p, odd);
        GlobalPoint::new(p.x() + map_center.x(), p.y() + map_center.y())
    }
}

impl<D: Direction> Neg for Transformation<D> {
    type Output = Self;
    fn neg(mut self) -> Self::Output {
        // mirror_vertically() rotates the map back to a neutral rotation
        // opposite_direction() inverses the translation
        self.translate_by = self.translate_by.rotate_by(self.distortion.rotation.mirror_vertically().opposite_direction());
        if self.distortion.mirrored {
            self.translate_by = self.translate_by.mirror_horizontally();
        }
        self.distortion = -self.distortion;
        self
    }
}

impl<D: Direction> Add for Transformation<D> {
    type Output = Self;
    fn add(mut self, mut rhs: Self) -> Self::Output {
        if self.distortion.mirrored {
            rhs.translate_by = rhs.translate_by.mirror_horizontally();
        }
        self.translate_by += rhs.translate_by.rotate_by(self.distortion.rotation);
        self.distortion += rhs.distortion;
        self
    }
}

impl<D: Direction> Sub for Transformation<D> {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self::Output {
        self + rhs.neg()
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
    wrapped_neighbors: HashMap<(Point, D), (Point, Distortion<D>)>,
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
        result.add_points(&mut area, &Transformation::new(Distortion::new(false, D::angle_0()), D::angle_0().translation(0))).unwrap();
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
        if self.seed_transformations.len() > MAX_TRANSFORMATIONS {
            self.error = Some(TransformationError::TooMany);
        } else {
            // check if the given transformations are valid and calculate wrapped_neighbors
            let mut area: HashMap<GlobalPoint, AreaPoint<D>> = HashMap::new();
            self.add_points(&mut area, &Transformation::new(Distortion::new(false, D::angle_0()), D::angle_0().translation(0))).unwrap();
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
                    let difference = list[i]- list[j];
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
        let transformation = Transformation::new(Distortion::new(false, D::angle_0()), D::angle_0().translation(0));
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
                if self.seed_transformations[i] == self.seed_transformations[j] || self.seed_transformations[i] == -self.seed_transformations[j] {
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
        if transformation.distortion.mirrored {
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
        let p = transformation.distortion.rotation.rotate_around_center(&p, &GlobalPoint::new(0, 0), self.odd_if_hex());
        let p = transformation.translate_by.translate_point(&p, self.odd_if_hex());
        p
    }
    #[cfg(feature = "rendering")]
    pub fn rendered_transformations(&self) -> Vec<Transformation<D>> {
        let mut transformations = vec![
            Transformation {
                distortion: Distortion::new(false, D::angle_0()),
                translate_by: D::angle_0().translation(0),
            }
        ];
        let adjacent_transformations = if self.error.is_none() {
            self.adjacent_transformations.clone()
        } else {
            let mut adjacent_transformations = vec![];
            for tran in &self.seed_transformations {
                adjacent_transformations.push(*tran);
            }
            for tran in &self.seed_transformations {
                adjacent_transformations.push(-*tran);
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
                    let new_tran = glob.add(*relative);
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
                distortion: Distortion::new(false, D::angle_0()),
                translate_by: D::angle_0().translation(0),
            }, HashSet::with_capacity(12))
        ];
        for tran in &self.seed_transformations {
            self.adjacent_transformations.push(*tran);
        }
        for tran in &self.seed_transformations {
            self.adjacent_transformations.push(-*tran);
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
                let new_tran = transformation.0 + self.adjacent_transformations[at];
                //println!("{:?} + {:?} = {:?}", &transformed[next_index].0, &t, &new_tran);
                match self.add_points(area, &new_tran) {
                    Ok(_) => {
                        let mut history = history.clone();
                        history.insert(transformation.0.distortion);
                        // check if new transformation is connected to center, thus creating new entry for "transformations"
                        let  neighbors = self.find_neighbors(area, &new_tran);
                        for neighbor in neighbors {
                            let new_seed = new_tran - neighbor;
                            if new_seed != transformations[0].0 && !self.adjacent_transformations.iter().any(|t| t == &new_seed) {
                                //println!("found new implied neighbor: {:?}", new_seed);
                                //println!("calculated as {:?} - {:?}", new_tran, neighbor);
                                let opp = -new_seed;
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
                        let mut direction = d.rotate_by(ap.0.distortion.rotation.mirror_vertically());
                        if ap.0.distortion.mirrored {
                            direction = direction.mirror_horizontally();
                        }
                        self.wrapped_neighbors.insert((p, d), (ap.1, ap.0.distortion));
                    }
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
    pub fn simple(point: Point, direction: D) -> Self {
        Self::new(point, false, direction)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct WrappingMap<D>
where D: Direction {
    pointmap: PointMap,
    // only needed to save and load the wrapped_neighbors
    seed_transformations: Vec<Transformation<D>>,
    wrapped_neighbors: HashMap<(Point, D), (Point, Distortion<D>)>,
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
    fn max_translation(map_size: MapSize) -> u16 {
        if D::is_hex() {
            map_size.width().max(map_size.height()) as u16 * 2
        } else {
            map_size.width().max(map_size.height()) as u16
        }
    }

    pub fn seed_transformations(&self) -> &Vec<Transformation<D>> {
        &self.seed_transformations
    }

    pub fn get_neighbor(&self, point: Point, direction: D) -> Option<(Point, Distortion<D>)> {
        if !self.pointmap.is_point_valid(point) {
            None
        } else if let Some(point) = direction.get_neighbor(point, self.pointmap.odd_if_hex())
        .filter(|point| {
            self.pointmap.is_point_valid(*point)
        }) {
            Some((point, Distortion::neutral()))
        } else if let Some((point, distortion)) = self.wrapped_neighbors.get(&(point, direction)) {
            Some((*point, *distortion))
        } else {
            None
        }
    }
}

impl<D: Direction> Zippable for WrappingMap<D> {
    fn zip(&self, zipper: &mut Zipper) {
        self.pointmap.zip(zipper);
        zipper.write_u8(self.seed_transformations.len() as u8, 3);
        let max_translation = Self::max_translation(self.pointmap.size());
        for trans in &self.seed_transformations {
            trans.export(zipper, max_translation);
        }
    }
    fn unzip(unzipper: &mut Unzipper) -> Result<Self, ZipperError> {
        let pointmap = PointMap::unzip(unzipper)?;
        let len = unzipper.read_u8(3)?;
        let mut seed_transformations = vec![];
        let max_translation = Self::max_translation(pointmap.size());
        for _ in 0..len {
            seed_transformations.push(Transformation::import(unzipper, max_translation)?);
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
    fn distortions() {
        for mirrored in vec![true, false] {
            for rotation in Direction4::list() {
                let distortion = Distortion::new(mirrored, rotation);
                assert_eq!(Distortion::neutral(), distortion - distortion);
            }
        }
        for mirrored in vec![true, false] {
            for rotation in Direction6::list() {
                let distortion = Distortion::new(mirrored, rotation);
                assert_eq!(Distortion::neutral(), distortion - distortion);
            }
        }
    }

    #[test]
    fn transformations1() {
        let transformations = vec![
            Transformation::new(Distortion::new(false, Direction4::D0), Direction4::D0.translation(0)),
            Transformation::new(Distortion::new(false, Direction4::D0), Direction4::D0.translation(5)),
            Transformation::new(Distortion::new(true, Direction4::D90), Direction4::D180.translation(6)),
            Transformation::new(Distortion::new(false, Direction4::D90), Direction4::D90.translation(7)),
            Transformation::new(Distortion::new(true, Direction4::D180), Direction4::D0.translation(8)),
            Transformation::new(Distortion::new(true, Direction4::D0), Direction4::D0.translation(9) + Direction4::D90.translation(7)),
        ];
        for t in transformations.iter().cloned() {
            assert_eq!(t, transformations[0] + t, "adding {t:?}");
            assert_eq!(t, -(-t), "double negative");
            assert_eq!(transformations[0], t - t, "subtracting {t:?} from itself");
        }
        for t in &transformations {
            for t2 in &transformations {
                //assert_eq!(*t, *t2 + (*t - *t2), "{t2:?} + {:?}", *t - *t2);
                for t3 in &transformations {
                    assert_eq!((*t + *t2) + *t3, *t + (*t2 + *t3), "order of addition shouldn't matter {t:?} + {t2:?} + {t3:?}");
                }
            }
        }
    }

    #[test]
    fn transformations2() {
        assert_eq!(-Transformation::new(Distortion::new(true, Direction4::D0), Direction4::D0.translation(8) + Direction4::D90.translation(7)),
                Transformation::new(Distortion::new(true, Direction4::D0), Direction4::D0.translation(8) + Direction4::D90.translation(-7)));
        assert_eq!(-Transformation::new(Distortion::new(true, Direction4::D90), Direction4::D0.translation(5) + Direction4::D90.translation(-6)),
                Transformation::new(Distortion::new(true, Direction4::D90), Direction4::D0.translation(-6) + Direction4::D90.translation(5)));
        /*println!("{:?}", -Transformation::new(Distortion::new(true, Direction4::D0), Direction4::D0.translation(-994) + Direction4::D90.translation(-20)));
        assert_eq!(Transformation::new(Distortion::new(false, Direction4::D0), Direction4::D0.translation(-990))
                    - Transformation::new(Distortion::new(true, Direction4::D0), Direction4::D0.translation(-994) + Direction4::D90.translation(-20)),
                Transformation::new(Distortion::new(true, Direction4::D0), Direction4::D0.translation(-4) + Direction4::D90.translation(20)));*/
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
            Transformation::new(Distortion::new(false, Direction4::D0), Direction4::D0.translation(-5))
        ]);
        let mut area: HashMap<GlobalPoint, AreaPoint<Direction4>> = HashMap::new();
        builder.add_points(&mut area, &Transformation::new(Distortion::new(false, Direction4::D0), Direction4::D0.translation(0))).unwrap();
        builder.check_seed_transformations(&mut area)?;
        let transformations = builder.check_transformations(&mut area)?;
        assert_eq!(transformations.len(), 3);

        Ok(())
    }

    #[test]
    fn mirrored_wrapping() -> Result<(), TransformationError<Direction4>> {
        let mut builder = WrappingMapBuilder::<Direction4>::new(PointMap::new(5, 4, false), vec![
            Transformation::new(Distortion::new(true, Direction4::D0), Direction4::D0.translation(-5))
        ]);
        let mut area: HashMap<GlobalPoint, AreaPoint<Direction4>> = HashMap::new();
        builder.add_points(&mut area, &Transformation::new(Distortion::new(false, Direction4::D0), Direction4::D0.translation(0))).unwrap();
        builder.check_seed_transformations(&mut area)?;
        let transformations = builder.check_transformations(&mut area)?;
        assert_eq!(transformations.len(), 2);

        Ok(())
    }

    #[test]
    fn rotated_wrapping() -> Result<(), TransformationError<Direction4>> {
        let mut builder = WrappingMapBuilder::<Direction4>::new(PointMap::new(5, 4, false), vec![
            Transformation::new(Distortion::new(false, Direction4::D90), Direction4::D0.translation(5))
        ]);
        let mut area: HashMap<GlobalPoint, AreaPoint<Direction4>> = HashMap::new();
        builder.add_points(&mut area, &Transformation::new(Distortion::new(false, Direction4::D0), Direction4::D0.translation(0))).unwrap();
        builder.check_seed_transformations(&mut area)?;
        let transformations = builder.check_transformations(&mut area)?;
        println!("{:?}", transformations);
        assert_eq!(transformations.len(), 4);
        
        Ok(())
    }

    #[test]
    fn double_wrapping() -> Result<(), TransformationError<Direction4>> {
        let mut builder = WrappingMapBuilder::<Direction4>::new(PointMap::new(5, 4, false), vec![
            Transformation::new(Distortion::new(false, Direction4::D0), Direction4::D0.translation(-5) + Direction4::D90.translation(2)),
            Transformation::new(Distortion::new(false, Direction4::D0), Direction4::D0.translation(-5) + Direction4::D90.translation(-2)),
        ]);
        let mut area: HashMap<GlobalPoint, AreaPoint<Direction4>> = HashMap::new();
        builder.add_points(&mut area, &Transformation::new(Distortion::new(false, Direction4::D0), Direction4::D0.translation(0))).unwrap();
        builder.check_seed_transformations(&mut area)?;
        let transformations = builder.check_transformations(&mut area)?;
        assert_eq!(transformations.len(), 7);

        Ok(())
    }
}
