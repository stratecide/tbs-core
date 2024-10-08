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

    pub fn update_direction(&self, mut direction: D) -> D {
        if self.mirrored {
            direction = direction.mirror_horizontally();
        }
        direction.rotate_by(self.rotation)
    }

    pub fn update_diagonal_direction(&self, mut direction: D) -> D {
        if self.mirrored {
            direction = direction.mirror_horizontally().rotate(true);
        }
        direction.rotate_by(self.rotation)
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

    pub fn neutral() -> Self {
        Self::new(Distortion::neutral(), D::angle_0().translation(0))
    }

    pub fn distortion(&self) -> Distortion<D> {
        self.distortion
    }
    pub fn translate_by(&self) -> &D::T {
        &self.translate_by
    }

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransformationError<D>
where D: Direction {
    Collision(AreaPoint<D>, Transformation<D>),
    CollisionCenter,
    CollidingTransformation,
    Disconnected,
    TooMany,
    Mirroring,
    DuplicateSeed,
}

/**
 * Builder can keep some data when modifying a transformation,
 * so it can more efficient in the client where users will drag transformations around
 * instead of rebuilding a WrappingMap repeatedly
 * 
 * objectives
 *  - make sure that transformations are connected to the map
 *  - make sure transformations don't overlap each other or the main map
 *  - find wrapping vectors for rendering
 *  - find all point-neighbors that can be reached due to a transformation
 *  - (optional) be efficient when updating a transformation
 *  - (optional) be efficient when building a valid map
 */
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WMBuilder<D: Direction> {
    map: PointMap,
    map_center: GlobalPoint,
    missing_neighbors: HashMap<GlobalPoint, Vec<D>>,
    generating_transformations: Vec<Transformation<D>>,
    // for each transformation, the transformed missing_neighbors are cached
    connected_transformations: Vec<(Transformation<D>, HashSet<GlobalPoint>)>,
    wrapping_vectors: Vec<D::T>,
    distortion_map: HashMap<Distortion<D>, D::T>,
}

impl<D: Direction> WMBuilder<D> {
    pub fn new(map: PointMap) -> Self {
        let map_center = GlobalPoint::new(map.width() as i16 / 2, map.height() as i16 / 2);
        let mut missing_neighbors = HashMap::new();
        for p in map.get_valid_points() {
            let missing_dirs: Vec<D> = D::list().into_iter()
            .filter(|d| match d.get_neighbor(p, map.odd_if_hex()) {
                Some(p) => !map.is_point_valid(p),
                None => true,
            }).collect();
            if missing_dirs.len() > 0 {
                missing_neighbors.insert(GlobalPoint::new(p.x as i16, p.y as i16), missing_dirs);
            }
        }
        Self {
            map,
            map_center,
            missing_neighbors,
            generating_transformations: Vec::new(),
            connected_transformations: Vec::new(),
            wrapping_vectors: Vec::new(),
            distortion_map: HashMap::new(),
        }
    }

    pub fn with_transformations(map: PointMap, transformation: Vec<Transformation<D>>) -> Result<Self, TransformationError<D>> {
        let mut result = Self::new(map);
        for tr in transformation {
            result.add_transformation(tr)?;
        }
        Ok(result)
    }

    pub fn pointmap(&self) -> &PointMap {
        &self.map
    }

    pub fn odd(&self) -> bool {
        self.map.odd_if_hex() == (self.map_center.y() % 2 == 0)
    }

    pub fn get_center(&self) -> GlobalPoint {
        self.map_center
    }

    pub fn wrapping_vectors(&self) -> &[D::T] {
        &self.wrapping_vectors
    }

    pub fn get_generating_transformations(&self) -> &[Transformation<D>] {
        &self.generating_transformations
    }

    pub fn get_connected_transformations(&self) -> Vec<Transformation<D>> {
        self.connected_transformations.iter()
        .map(|(tr, _)| tr.clone())
        .collect()
    }

    fn point_map_equivalent(&self, p: GlobalPoint) -> Option<Point> {
        if p.x < 0 || p.y < 0 || p.x as u32 >= self.map.width() as u32 || p.y as u32 >= self.map.height() as u32 {
            return None;
        }
        let p = Point::new(p.x as u8, p.y as u8);
        if self.map.is_point_valid(p) {
            Some(p)
        } else {
            None
        }
    }

    pub fn is_inside_map(&self, p: GlobalPoint) -> bool {
        self.point_map_equivalent(p).is_some()
    }

    pub fn localize_point(&self, global_p: GlobalPoint) -> Option<(Point, Distortion<D>)> {
        let mut tr = D::T::between(&self.map_center, &global_p, self.map.odd_if_hex());
        wrap_point::<D>(&mut tr, &self.wrapping_vectors);
        let global_p = tr.translate_point(&self.map_center, self.map.odd_if_hex());
        let mut x_translation = vec![D::angle_0().translation(0)];
        if self.wrapping_vectors.len() > 0 {
            x_translation.push(self.wrapping_vectors[0]);
            x_translation.push(-self.wrapping_vectors[0]);
        }
        let mut y_translation = vec![D::angle_0().translation(0)];
        if self.wrapping_vectors.len() > 1 {
            y_translation.push(self.wrapping_vectors[1]);
            y_translation.push(-self.wrapping_vectors[1]);
        }
        for x in x_translation {
            for y in &y_translation {
                let translation = x + *y;
                let p = Transformation::new(Distortion::<D>::neutral(), translation).transform_point(&global_p, &self.map_center, self.odd());
                if let Some(p) = self.point_map_equivalent(p) {
                    return Some((p, Distortion::neutral()));
                }
                for (distortion, tr) in &self.distortion_map {
                    let p = Transformation::new(*distortion, translation + *tr).transform_point(&global_p, &self.map_center, self.odd());
                    if let Some(p) = self.point_map_equivalent(p) {
                        return Some((p, -*distortion));
                    }
                }
            }
        }
        None
    }

    fn check_connected_with_map(&self, transformation: &Transformation<D>) -> Result<(), TransformationError<D>> {
        let mut connected = false;
        for (p, dirs) in &self.missing_neighbors {
            let transformed = transformation.transform_point(p, &self.map_center, self.odd());
            if self.is_inside_map(transformed) {
                return Err(TransformationError::CollisionCenter);
            }
            if !connected {
                for d in dirs {
                    if self.is_inside_map(transformation.distortion.update_direction(*d).get_global_neighbor(transformed, self.odd())) {
                        connected = true;
                        break;
                    }
                }
            }
        }
        if connected {
            Ok(())
        } else {
            Err(TransformationError::Disconnected)
        }
    }

    fn prevent_colliding_transformation(&self, transformation: &Transformation<D>) -> Result<(), TransformationError<D>> {
        for p in self.map.get_valid_points() {
            let transformed: GlobalPoint = transformation.transform_point(&GlobalPoint::new(p.x as i16, p.y as i16), &self.map_center, self.odd());
            for (_, edge) in &self.connected_transformations {
                if edge.contains(&transformed) {
                    return Err(TransformationError::CollidingTransformation);
                }
            }
        }
        Ok(())
    }

    fn find_wrapping_vectors(generating_transformations: &[Transformation<D>]) -> Result<(Vec<D::T>, HashMap<Distortion<D>, D::T>), TransformationError<D>> {
        let mut connected_transformations = HashSet::new();
        for tr in generating_transformations {
            connected_transformations.insert(*tr);
            connected_transformations.insert(-*tr);
        }
        let mut transformations = vec![Transformation::new(Distortion::neutral(), D::angle_0().translation(0))];
        let mut result = Vec::new();
        let mut distortion_map = HashMap::new();
        distortion_map.insert(Distortion::neutral(), D::angle_0().translation(0));
        let mut i = 0;
        while i < transformations.len() {
            for gen in &connected_transformations {
                let mut tr = transformations[i] + *gen;
                wrap_point::<D>(&mut tr.translate_by, &result);
                if let Some(candidate) = distortion_map.get(&tr.distortion()) {
                    if *candidate == tr.translate_by {
                        continue;
                    }
                    let new_vector = tr.translate_by - *candidate;
                    let mut added = false;
                    for i in 0..result.len() {
                        if (new_vector % result[i]).len() == 0 {
                            // new vector is better than this one, replaces it
                            result[i] = new_vector;
                            added = true;
                            break;
                        }
                    }
                    if !added && result.len() == 2 {
                        for (i, mut vector) in result.iter().cloned().enumerate() {
                            wrap_point::<D>(&mut vector, &[new_vector, result[1 - i]]);
                            if vector.len() == 0 {
                                // combination of new vector and other old vector is better than this vector
                                result[i] = new_vector;
                                added = true;
                                break;
                            }
                        }
                    }
                    if !added {
                        if result.len() < 2 {
                            result.push(new_vector);
                        } else {
                            // found a wrapping_vector that can't be constructed from the 2 previously found ones
                            // which indicates impossible wrapping since smaller vectors should be found first (i hope)
                            return Err(TransformationError::CollidingTransformation);
                        }
                    }
                    // found a new (or better) wrapping_vector.
                    // now all previously found translations should be remapped
                    for candidate in distortion_map.values_mut() {
                        wrap_point::<D>(candidate, &result);
                    }
                    for tr in transformations.iter_mut().skip(i) {
                        wrap_point::<D>(&mut tr.translate_by, &result);
                    }
                } else {
                    distortion_map.insert(tr.distortion(), tr.translate_by);
                    transformations.push(tr);
                }
            }
            i += 1;
        }
        distortion_map.remove(&Distortion::neutral());
        Ok((result, distortion_map))
    }

    fn add_connected_transformation(&mut self, transformation: Transformation<D>) {
        if !self.connected_transformations.iter().any(|(tr, _)| {
            *tr == transformation
        }) {
            let edge = self.build_transformed_edge(&transformation);
            self.connected_transformations.push((transformation, edge));
        }
    }

    fn build_transformed_edge(&self, transformation: &Transformation<D>) -> HashSet<GlobalPoint> {
        let mut edge = HashSet::new();
        for (p, _) in &self.missing_neighbors {
            edge.insert(transformation.transform_point(p, &self.map_center, self.odd()));
        }
        edge
    }

    fn rebuild_connected_transformations(&mut self) {
        let mut connected = HashSet::new();
        let mut wrapping = self.wrapping_vectors.clone();
        if wrapping.len() == 2 {
            wrapping.push(wrapping[0] + wrapping[1]);
        }
        wrapping.push(D::angle_0().translation(0));
        for wrapping in wrapping {
            for (distortion, translation) in &self.distortion_map {
                let tr = Transformation::new(*distortion, *translation - wrapping);
                if Self::check_connected_with_map(&self, &tr).is_ok() {
                    connected.insert(tr);
                }
            }
        }
        let mut x_translation = vec![D::angle_0().translation(0)];
        if self.wrapping_vectors.len() > 0 {
            x_translation.push(self.wrapping_vectors[0]);
            x_translation.push(-self.wrapping_vectors[0]);
        }
        let mut y_translation = vec![D::angle_0().translation(0)];
        if self.wrapping_vectors.len() > 1 {
            y_translation.push(self.wrapping_vectors[1]);
            y_translation.push(-self.wrapping_vectors[1]);
        }
        for x in x_translation {
            for y in &y_translation {
                let translation = x + *y;
                if translation.len() == 0 {
                    continue;
                }
                let tr = Transformation::new(Distortion::neutral(), translation);
                if Self::check_connected_with_map(&self, &tr).is_ok() {
                    connected.insert(tr);
                }
            }
        }
        // don't re-create unchanged connected transformations
        self.connected_transformations.retain(|(tr, _)| {
            connected.contains(tr)
        });
        for connected in connected {
            self.add_connected_transformation(connected);
        }
    }

    pub fn add_transformation(&mut self, transformation: Transformation<D>) -> Result<(), TransformationError<D>> {
        if self.generating_transformations.len() >= MAX_TRANSFORMATIONS {
            return Err(TransformationError::TooMany);
        }
        for (tr, _) in &self.connected_transformations {
            if *tr == transformation || *tr == -transformation {
                return Err(TransformationError::DuplicateSeed);
            }
        }
        self.check_connected_with_map(&transformation)?;
        // no collision with map, but it could still be colliding with another transformation
        self.prevent_colliding_transformation(&transformation)?;
        let mut generating_transformations = self.generating_transformations.clone();
        generating_transformations.push(transformation);
        let (wrapping_vectors, distortion_map) = Self::find_wrapping_vectors(&generating_transformations)?;
        let mut x_translation = vec![D::angle_0().translation(0)];
        if wrapping_vectors.len() > 0 {
            x_translation.push(wrapping_vectors[0]);
            x_translation.push(-wrapping_vectors[0]);
        }
        let mut y_translation = vec![D::angle_0().translation(0)];
        if wrapping_vectors.len() > 1 {
            y_translation.push(wrapping_vectors[1]);
            y_translation.push(-wrapping_vectors[1]);
        }
        // make sure there's no overlapping transformations
        for x in x_translation {
            for y in &y_translation {
                let translation = x + *y;
                if translation.len() != 0 {
                    for (p, _) in &self.missing_neighbors {
                        let transformed = translation.translate_point(p, self.map.odd_if_hex());
                        if self.is_inside_map(transformed) {
                            return Err(TransformationError::CollidingTransformation);
                        }
                    }
                }
                for (distortion, tr) in &distortion_map {
                    let transformation = Transformation::new(*distortion, translation + *tr);
                    for (p, _) in &self.missing_neighbors {
                        let transformed = transformation.transform_point(p, &self.map_center, self.odd());
                        if self.is_inside_map(transformed) {
                            return Err(TransformationError::CollidingTransformation);
                        }
                    }
                }
            }
        }
        self.generating_transformations.push(transformation);
        self.wrapping_vectors = wrapping_vectors;
        self.distortion_map = distortion_map;
        self.rebuild_connected_transformations();
        Ok(())
    }

    /**
     * returns true if the transformation was among the generating transformations
     */
    pub fn remove_transformation(&mut self, transformation: Transformation<D>) -> bool {
        let mut removed = false;
        for i in 0..self.generating_transformations.len() {
            if self.generating_transformations[i] == transformation || self.generating_transformations[i] == -transformation {
                self.generating_transformations.remove(i);
                removed = true;
                break;
            }
        }
        if removed {
            let (wrapping_vectors, distortion_map) = Self::find_wrapping_vectors(&self.generating_transformations).expect("shouldn't break from removing a transformation");
            self.wrapping_vectors = wrapping_vectors;
            self.distortion_map = distortion_map;
            self.rebuild_connected_transformations();
        }
        removed
    }

    pub fn build(&self) -> WrappingMap<D> {
        let mut wrapped_neighbors = HashMap::new();
        for (source, dirs) in &self.missing_neighbors {
            let local_p = self.point_map_equivalent(*source).expect("only real points should have missing neighbors");
            for d in dirs {
                let p = d.get_global_neighbor(*source, self.map.odd_if_hex());
                if let Some((destination, distortion)) = self.localize_point(p) {
                    wrapped_neighbors.insert((local_p, *d), (destination, -distortion));
                }
            }
        }
        WrappingMap {
            pointmap: self.map.clone(),
            #[cfg(feature = "rendering")]
            screen_wrap_vectors: self.wrapping_vectors.clone(),
            seed_transformations: self.generating_transformations.clone(),
            wrapped_neighbors,
        }
    }

    pub fn distortions<'a>(&'a self) -> impl Iterator<Item = Distortion<D>> + 'a {
        [Distortion::neutral()].into_iter().chain(self.distortion_map.keys().cloned())
    }
}

fn wrap_point<D: Direction>(point: &mut D::T, wrapping_vectors: &[D::T]) {
    for vector in wrapping_vectors {
        *point = *point % *vector;
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
    /*fn new(builder: WrappingMapBuilder<D>) -> Self {
        WrappingMap {
            #[cfg(feature = "rendering")]
            screen_wrap_vectors: builder.screen_wrap_vectors(),
            pointmap: builder.map,
            seed_transformations: builder.seed_transformations,
            wrapped_neighbors: builder.wrapped_neighbors,
        }
    }*/

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
    pub fn max_translation(map_size: MapSize) -> u16 {
        if D::is_hex() {
            map_size.width().max(map_size.height()) as u16 * 2
        } else {
            map_size.width().max(map_size.height()) as u16
        }
    }

    pub fn seed_transformations(&self) -> &Vec<Transformation<D>> {
        &self.seed_transformations
    }

    /**
     * the returned Distortion has to be applied to direction in order to
     * keep moving in the same direction
     */
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
        let mut seed_transformations = Vec::new();
        let max_translation = Self::max_translation(pointmap.size());
        for _ in 0..len {
            seed_transformations.push(Transformation::import(unzipper, max_translation)?);
        }
        if let Ok(builder) = WMBuilder::with_transformations(pointmap, seed_transformations) {
            Ok(builder.build())
        } else {
            Err(ZipperError::InconsistentData)
        }
    }
}


#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::{config::config::Config, game::{game::Game, game_view::GameView}, map::{map::{Map, NeighborMode}, point_map::PointMap}, terrain::TerrainType, units::{movement::Path, unit_types::UnitType}};

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
    fn no_wrapping() {
        let builder = WMBuilder::<Direction4>::new(PointMap::new(4, 8, false));
        builder.build();
        let builder = WMBuilder::<Direction6>::new(PointMap::new(15, 11, false));
        let map = builder.build();
        let p = Point::new(0, 0);
        let neighbors: Vec<Option<Point>> = Direction6::list().into_iter()
        .map(|d|
            map.get_neighbor(p, d).map(|(p, _)| p)
        ).collect();
        assert_eq!(
            neighbors,
            vec![Some(Point::new(1, 0)), None, None, None, None, Some(Point::new(0, 1))],
        );
        let p = Point::new(14, 1);
        let neighbors: Vec<Option<Point>> = Direction6::list().into_iter()
        .map(|d|
            map.get_neighbor(p, d).map(|(p, _)| p)
        ).collect();
        assert_eq!(
            neighbors,
            vec![None, None, Some(Point::new(14, 0)), Some(Point::new(13, 1)), Some(Point::new(14, 2)), None],
        );
    }

    #[test]
    fn simple_wrapping() {
        let builder = WMBuilder::<Direction4>::with_transformations(PointMap::new(5, 4, false), vec![
            Transformation::new(Distortion::new(false, Direction4::D0), Direction4::D0.translation(-5))
        ]).unwrap();
        assert_eq!(builder.localize_point(GlobalPoint::new(1, 2)), Some((Point::new(1, 2), Distortion::neutral())));
        assert_eq!(builder.localize_point(GlobalPoint::new(6, 2)), Some((Point::new(1, 2), Distortion::neutral())));
        assert_eq!(builder.localize_point(GlobalPoint::new(1, 5)), None);

        let builder = WMBuilder::<Direction6>::with_transformations(PointMap::new(15, 11, false), vec![
            Transformation::new(Distortion::new(false, Direction6::D0), Direction6::D0.translation(13) + Direction6::D60.translation(5)),
        ]).unwrap();
        let map = builder.build();
        let p = Point::new(0, 0);
        let neighbors: Vec<Option<Point>> = Direction6::list().into_iter()
        .map(|d|
            map.get_neighbor(p, d).map(|(p, _)| p)
        ).collect();
        assert_eq!(
            neighbors,
            vec![Some(Point::new(1, 0)), None, None, None, None, Some(Point::new(0, 1))],
        );
        let p = Point::new(14, 1);
        let neighbors: Vec<Option<Point>> = Direction6::list().into_iter()
        .map(|d|
            map.get_neighbor(p, d).map(|(p, _)| p)
        ).collect();
        assert_eq!(
            neighbors,
            vec![Some(Point::new(0, 6)), None, Some(Point::new(14, 0)), Some(Point::new(13, 1)), Some(Point::new(14, 2)), None],
        );
    }

    #[test]
    fn mirrored_wrapping() -> Result<(), TransformationError<Direction4>> {
        let builder = WMBuilder::<Direction4>::with_transformations(PointMap::new(5, 4, false), vec![
            Transformation::new(Distortion::new(true, Direction4::D0), Direction4::D0.translation(-5))
        ])?;
        assert_eq!(builder.localize_point(GlobalPoint::new(1, 2)), Some((Point::new(1, 2), Distortion::neutral())));
        assert_eq!(builder.localize_point(GlobalPoint::new(-4, 2)), Some((Point::new(3, 2), Distortion::new(true, Direction4::D0))));
        assert_eq!(builder.localize_point(GlobalPoint::new(6, 2)), None);
        let map = builder.build();
        assert_eq!(map.get_neighbor(Point::new(0, 0), Direction4::D180), Some((Point::new(0, 0), Distortion::new(true, Direction4::D0))));
        Ok(())
    }

    #[test]
    fn rotated_wrapping() -> Result<(), TransformationError<Direction4>> {
        let builder = WMBuilder::<Direction4>::with_transformations(PointMap::new(5, 4, false), vec![
            Transformation::new(Distortion::new(false, Direction4::D90), Direction4::D0.translation(5))
        ])?;
        assert_eq!(builder.localize_point(GlobalPoint::new(1, 2)), Some((Point::new(1, 2), Distortion::neutral())));
        assert_eq!(builder.localize_point(GlobalPoint::new(6, 3)), Some((Point::new(1, 1), Distortion::new(false, Direction4::D90))));
        assert_eq!(builder.localize_point(GlobalPoint::new(11, 2)), None);
        let map = builder.build();
        assert_eq!(map.get_neighbor(Point::new(0, 0), Direction4::D90), None);
        assert_eq!(map.get_neighbor(Point::new(1, 0), Direction4::D90), Some((Point::new(4, 3), Distortion::new(false, Direction4::D90))));
        Ok(())
    }

    #[test]
    fn double_wrapping() -> Result<(), TransformationError<Direction4>> {
        let builder = WMBuilder::<Direction4>::with_transformations(PointMap::new(5, 4, false), vec![
            Transformation::new(Distortion::neutral(), Direction4::D0.translation(-5) + Direction4::D90.translation(2)),
            Transformation::new(Distortion::neutral(), Direction4::D0.translation(-5) + Direction4::D90.translation(-2)),
        ])?;
        assert_eq!(builder.localize_point(GlobalPoint::new(1, 2)), Some((Point::new(1, 2), Distortion::neutral())));
        assert_eq!(builder.localize_point(GlobalPoint::new(6, 2)), Some((Point::new(1, 0), Distortion::neutral())));
        assert_eq!(builder.localize_point(GlobalPoint::new(11, 2)), Some((Point::new(1, 2), Distortion::neutral())));
        let map = builder.build();
        assert_eq!(map.get_neighbor(Point::new(0, 0), Direction4::D90), Some((Point::new(0, 3), Distortion::neutral())));
        Ok(())
    }

    #[test]
    fn rotation_and_mirror() -> Result<(), TransformationError<Direction4>> {
        let builder = WMBuilder::<Direction4>::with_transformations(PointMap::new(5, 4, false), vec![
            Transformation::new(Distortion::new(false, Direction4::D90), Direction4::D0.translation(5)),
            Transformation::new(Distortion::new(true, Direction4::D0), Direction4::D0.translation(-5)),
        ])?;
        assert_eq!(builder.localize_point(GlobalPoint::new(1, 2)), Some((Point::new(1, 2), Distortion::neutral())));
        assert_eq!(builder.localize_point(GlobalPoint::new(6, 3)), Some((Point::new(1, 1), Distortion::new(false, Direction4::D90))));
        assert_eq!(builder.localize_point(GlobalPoint::new(-4, 2)), Some((Point::new(3, 2), Distortion::new(true, Direction4::D0))));
        let map: WrappingMap<Direction4> = builder.build();
        assert_eq!(map.get_neighbor(Point::new(0, 0), Direction4::D90), None);
        assert_eq!(map.get_neighbor(Point::new(1, 0), Direction4::D90), Some((Point::new(4, 3), Distortion::new(false, Direction4::D90))));
        let builder = WMBuilder::<Direction4>::with_transformations(PointMap::new(31, 25, false), vec![
            Transformation::new(Distortion::new(false, Direction4::D90), Direction4::D0.translation(28) + Direction4::D90.translation(3)),
            Transformation::new(Distortion::new(true, Direction4::D0), Direction4::D0.translation(-31)),
        ])?;
        let map: WrappingMap<Direction4> = builder.build();
        assert_eq!(map.get_neighbor(Point::new(0, map.pointmap.height() - 1), Direction4::D270), Some((Point::new(0, map.pointmap.height() - 1), Distortion::new(true, Direction4::D180))));
        assert_eq!(map.get_neighbor(Point::new(map.pointmap.width() - 1, map.pointmap.height() - 1), Direction4::D270), Some((Point::new(map.pointmap.width() - 1, map.pointmap.height() - 1), Distortion::new(true, Direction4::D180))));
        Ok(())
    }

    #[test]
    fn transformation_errors() -> Result<(), TransformationError<Direction4>> {
        let mut builder = WMBuilder::<Direction4>::new(PointMap::new(5, 4, false));
        let unchanged: WMBuilder<Direction4> = builder.clone();
        assert_eq!(builder.add_transformation(Transformation::new(Distortion::neutral(), Direction4::D0.translation(4))), Err(TransformationError::CollisionCenter));
        assert_eq!(builder.add_transformation(Transformation::new(Distortion::neutral(), Direction4::D0.translation(6))), Err(TransformationError::Disconnected));
        assert_eq!(builder, unchanged);
        builder.add_transformation(Transformation::new(Distortion::new(false, Direction4::D90), Direction4::D0.translation(5))).unwrap();
        let unchanged: WMBuilder<Direction4> = builder.clone();
        assert_eq!(builder.add_transformation(Transformation::new(Distortion::neutral(), Direction4::D0.translation(5))), Err(TransformationError::CollidingTransformation));
        assert_eq!(builder.add_transformation(Transformation::new(Distortion::neutral(), Direction4::D0.translation(-5))), Err(TransformationError::CollidingTransformation));
        assert_eq!(builder, unchanged);
        let mut builder = WMBuilder::<Direction4>::new(PointMap::new(5, 4, false));
        builder.add_transformation(Transformation::new(Distortion::new(false, Direction4::D90), Direction4::D0.translation(5) + Direction4::D90.translation(2))).unwrap();
        assert_eq!(builder.add_transformation(Transformation::new(Distortion::new(true, Direction4::D0), Direction4::D0.translation(-5))), Err(TransformationError::CollidingTransformation));
        Ok(())
    }

    #[test]
    fn straight_line() {
        let config = Arc::new(Config::test_config());
        let map = PointMap::new(8, 5, false);
        let map = WMBuilder::<Direction4>::with_transformations(map, vec![Transformation::new(Distortion::new(false, Direction4::D90), Direction4::D0.translation(6))]).unwrap();
        let mut map = Map::new(map.build(), &config);
        let map_env = map.environment().clone();
        for x in 0..8 {
            for y in 0..5 {
                map.set_terrain(Point::new(x, y), TerrainType::ChessTile.instance(&map_env).build_with_defaults());
            }
        }
        map.set_unit(Point::new(3, 2), Some(UnitType::Rook.instance(&map_env).set_owner_id(0).build_with_defaults()));
        map.set_unit(Point::new(4, 0), Some(UnitType::Marine.instance(&map_env).set_owner_id(1).build_with_defaults()));
        assert_eq!(
            map.wrapping_logic().get_neighbor(Point::new(3, 0), Direction4::D90),
            Some((Point::new(7, 3), Distortion::new(false, Direction4::D90)))
        );
        assert_eq!(
            map.get_line(Point::new(3, 2), Direction4::D90, 5, NeighborMode::FollowPipes),
            vec![
                OrientedPoint::new(Point::new(3, 2), false, Direction4::D90),
                OrientedPoint::new(Point::new(3, 1), false, Direction4::D90),
                OrientedPoint::new(Point::new(3, 0), false, Direction4::D90),
                OrientedPoint::new(Point::new(7, 3), false, Direction4::D180),
                OrientedPoint::new(Point::new(6, 3), false, Direction4::D180),
            ]
        );
        let settings = map.settings().unwrap();
        let (game, _) = Game::new_server(map, settings.build_default(), Arc::new(|| 0.));
        let environment = game.environment();
        let rook = UnitType::Rook.instance(&environment).set_owner_id(0).build_with_defaults();
        rook.shortest_path_to(&*game, &Path::new(Point::new(3, 2)), None, Point::new(0, 3)).unwrap();
    }
}
