use std::collections::{BinaryHeap, HashSet, HashMap};
use std::cmp::Ordering;
use std::fmt::Debug;
use std::hash::Hash;
use std::ops::{Add, Sub, AddAssign, SubAssign};

use zipper::*;
use zipper_derive::*;

use crate::game::events::CommandError;
use crate::map::direction::Direction;
use crate::map::point::Point;
use crate::game::game::{Game, Vision};
use crate::map::map::*;
use crate::map::wrapping_map::OrientedPoint;
use crate::terrain::Terrain;

use super::normal_units::{NormalUnits, NormalUnit};
use super::{chess, UnitType};

pub enum PathSearchFeedback {
    Continue,
    Rejected,
    Found,
}


#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HoverMode {
    Land,
    Sea,
    Beach,
}
impl HoverMode {
    pub fn new(on_sea: bool) -> Self {
        if on_sea {
            Self::Sea
        } else {
            Self::Land
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MovementPoints {
    times_6: u8,
}

pub enum MovementPointsError {
    Negative,
    TooBig,
    // for cases where input times 6 isn't close enough to an integer?
    //Imprecise,
}

impl From<f32> for MovementPoints {
    fn from(value: f32) -> Self {
        Self {
            times_6: (value * 6.).max(0.).min(255.).round() as u8,
        }
    }
}

impl Add<MovementPoints> for MovementPoints {
    type Output = Self;
    fn add(self, rhs: MovementPoints) -> Self::Output {
        Self {
            times_6: (self.times_6 as u16 + rhs.times_6 as u16).min(255) as u8,
        }
    }
}

impl AddAssign for MovementPoints {
    fn add_assign(&mut self, rhs: Self) {
        self.times_6 = (self.times_6 as u16 + rhs.times_6 as u16).min(255) as u8;
    }
}

impl Sub<MovementPoints> for MovementPoints {
    type Output = Self;
    fn sub(self, rhs: MovementPoints) -> Self::Output {
        Self {
            times_6: (self.times_6 as i16 - rhs.times_6 as i16).max(0) as u8,
        }
    }
}

impl SubAssign for MovementPoints {
    fn sub_assign(&mut self, rhs: Self) {
        self.times_6 = (self.times_6 as i16 - rhs.times_6 as i16).max(0) as u8;
    }
}

impl PartialOrd for MovementPoints {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.times_6.partial_cmp(&other.times_6)
    }
}

impl Ord for MovementPoints {
    fn cmp(&self, other: &Self) -> Ordering {
        self.times_6.cmp(&other.times_6)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MovementType {
    Foot,
    Wheel,
    Treads,

    Hover(HoverMode),
    
    Boat,
    Ship,

    Heli,
    Plane,

    Chess,
}

#[derive(PartialEq, Eq)]
struct WidthSearch<D: Direction> {
    path: Path<D>,
    path_cost: MovementPoints,
    movement_type: MovementType,
}
impl<D: Direction> PartialOrd for WidthSearch<D> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.path_cost.cmp(&other.path_cost))
    }
}
impl<D: Direction> Ord for WidthSearch<D> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.path_cost.cmp(&other.path_cost)
    }
}


#[derive(Debug, Clone, PartialEq, Eq, Zippable, Hash)]
#[zippable(bits = 3)]
pub enum PathStep<D: Direction> {
    Dir(D),
    Jump(D), // jumps 2 fields, caused by Fountains
    Diagonal(D), // moves diagonally, for chess units
    Knight(D, bool),
    Point(Point),
}
impl<D: Direction> PathStep<D> {
    pub fn progress(&self, map: &Map<D>, pos: Point) -> Result<Point, CommandError> {
        match self {
            Self::Dir(d) => {
                if let Some(o) = map.get_neighbor(pos, *d) {
                    Ok(o.point)
                } else {
                    Err(CommandError::InvalidPath)
                }
            }
            Self::Jump(d) => {
                if let Some(o) = map.get_neighbor(pos, *d).and_then(|o| map.get_neighbor(o.point, o.direction)) {
                    Ok(o.point)
                } else {
                    Err(CommandError::InvalidPath)
                }
            }
            Self::Diagonal(d) => {
                if let Some(o) = chess::get_diagonal_neighbor(map, pos, *d) {
                    Ok(o.point)
                } else {
                    Err(CommandError::InvalidPath)
                }
            }
            Self::Knight(d, turn_left) => {
                if let Some(o) = chess::get_knight_neighbor(map, pos, *d, *turn_left) {
                    Ok(o.point)
                } else {
                    Err(CommandError::InvalidPath)
                }
            }
            Self::Point(p) => Ok(*p),
        }
    }
    pub fn dir(&self) -> Option<D> {
        match self {
            Self::Dir(d) => Some(*d),
            Self::Jump(d) => Some(*d),
            Self::Diagonal(_) => None,
            Self::Knight(_, _) => None,
            Self::Point(_) => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Zippable, Hash)]
pub struct Path<D: Direction> {
    pub start: Point,
    pub steps: LVec::<PathStep::<D>, {crate::map::point_map::MAX_AREA}>,
}
impl<D: Direction> Path<D> {
    pub fn new(start: Point) -> Self {
        Self {
            start,
            steps: LVec::new(),
        }
    }

    pub fn end(&self, map: &Map<D>) -> Result<Point, CommandError> {
        let mut current = self.start;
        for step in &self.steps {
            current = step.progress(map, current)?;
        }
        Ok(current)
    }
    
    pub fn points(&self, map: &Map<D>) -> Result<Vec<Point>, CommandError> {
        let mut points = vec![self.start];
        let mut current = self.start;
        for step in self.steps.iter() {
            current = step.progress(map, current)?;
            points.push(current);
        }
        Ok(points)
    }

    pub fn hover_steps(&self, map: &Map<D>, hover_mode: HoverMode) -> LVec<HoverStep<D>, {crate::map::point_map::MAX_AREA}> {
        let mut steps = LVec::new();
        let mut current = self.start;
        let mut prev_terrain = map.get_terrain(current).unwrap();
        let mut movement_type = MovementType::Hover(hover_mode);
        for step in &self.steps {
            current = step.progress(map, current).unwrap();
            let terrain = map.get_terrain(current).unwrap();
            movement_type = terrain.update_movement_type(movement_type, prev_terrain).unwrap();
            let on_sea = movement_type != MovementType::Hover(HoverMode::Land);
            steps.push((on_sea, step.clone()));
            prev_terrain = terrain;
        }
        steps
    }
}

pub trait PathStepExt<D: Direction>: Debug + Clone {
    fn step(&self) -> &PathStep<D>;
    fn skip_to(&self, p: Point) -> Self;
    fn update_unit(&self, unit: &mut UnitType<D>) {
        match unit {
            UnitType::Normal(unit) => self.update_normal_unit(unit),
            _ => (),
        }
    }
    fn update_normal_unit(&self, unit: &mut NormalUnit);
}
impl<D: Direction> PathStepExt<D> for PathStep<D> {
    fn step(&self) -> &PathStep<D> {
        self
    }
    fn skip_to(&self, p: Point) -> Self {
        PathStep::Point(p)
    }
    fn update_normal_unit(&self, _: &mut NormalUnit) {
        // do nothing
    }
}

type HoverStep<D> = (bool, PathStep<D>);
impl<D: Direction> PathStepExt<D> for HoverStep<D> {
    fn step(&self) -> &PathStep<D> {
        &self.1
    }
    fn skip_to(&self, p: Point) -> Self {
        (self.0, self.1.skip_to(p))
    }
    fn update_normal_unit(&self, unit: &mut NormalUnit) {
        match &mut unit.typ {
            NormalUnits::Hovercraft(on_sea) => *on_sea = self.0,
            _ => {}
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct MovementSearchMeta<D: Direction> {
    movement_type: MovementType,
    illegal_next_dir: Option<D>, // units aren't allowed to turn around 180°
    previous_turns: Vec<Path<D>>,
    remaining_movement: MovementPoints,
    path: Path<D>,
}
impl<D: Direction> MovementSearchMeta<D> {
    fn heap_order(&self, other: &Self) -> Ordering {
        if self.previous_turns.len() == other.previous_turns.len() {
            self.remaining_movement.cmp(&other.remaining_movement)
        } else {
            self.previous_turns.len().cmp(&other.previous_turns.len())
        }
    }
    fn compare(&self, other: &Self) -> Option<Ordering> {
        if self.movement_type != other.movement_type {
            return None;
        }
        let mut is_better = false;
        let mut is_worse = false;
        match (self.illegal_next_dir, other.illegal_next_dir) {
            (None, None) => (),
            (None, Some(_)) => is_better = true,
            (Some(_), None) => is_worse = true,
            (Some(a), Some(b)) => {
                if a != b {
                    return None;
                }
            }
        }
        match self.heap_order(other) {
            Ordering::Less => is_better = true,
            Ordering::Greater => is_worse = true,
            _ => (),
        }
        match (is_better, is_worse) {
            (false, false) => Some(Ordering::Equal),
            (true, false) => Some(Ordering::Less),
            (false, true) => Some(Ordering::Greater),
            (true, true) => None,
        }
    }
}
impl<D: Direction> PartialOrd for MovementSearchMeta<D> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.heap_order(other))
    }
}
impl<D: Direction> Ord for MovementSearchMeta<D> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.heap_order(other)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct MovementSearch<D: Direction> {
    pos: Point,
    meta: MovementSearchMeta<D>,
}
impl<D: Direction> PartialOrd for MovementSearch<D> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.meta.partial_cmp(&other.meta)
    }
}
impl<D: Direction> Ord for MovementSearch<D> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.meta.cmp(&other.meta)
    }
}

fn search_path<D>(
    map: &Map<D>,
    start: Point,
    additional_turns: usize,
    base_movement: impl Fn(&Terrain<D>, Option<MovementType>) -> (MovementType, MovementPoints),
    update_movement: impl Fn(&Terrain<D>, &Terrain<D>, MovementType, MovementPoints, MovementPoints) -> Option<(MovementType, MovementPoints)>,
    find_steps: impl Fn(Point, MovementType) -> HashSet<(OrientedPoint<D>, PathStep<D>)>,
    mut callback: impl FnMut(&Path<D>, Point) -> PathSearchFeedback
) -> Option<Path<D>>
where
    D: Direction,
{
    // if the unit can't move from it's position, no need to go further
    let (movement_type, points) = if let Some(terrain) = map.get_terrain(start) {
        base_movement(terrain, None)
    } else {
        return None;
    };
    // some ways that arrive at the same point may be incomparable
    // (better in one way, worse in another)
    // so for each point, a HashSet is used to store the best paths
    let mut best_metas: HashMap<Point, HashSet<MovementSearchMeta<D>>> = HashMap::new();
    let mut next_checks: BinaryHeap<MovementSearch<D>> = BinaryHeap::new();
    next_checks.push(MovementSearch {
        pos: start,
        meta: MovementSearchMeta {
            movement_type,
            remaining_movement: points,
            previous_turns: Vec::new(),
            illegal_next_dir: None,
            path: Path::new(start),
        }
    });
    while let Some(MovementSearch { pos, meta }) = next_checks.pop() {
        // check if our current meta is accepted by the callback
        match callback(&meta.path, pos) {
            PathSearchFeedback::Found => return Some(meta.path),
            PathSearchFeedback::Rejected => continue,
            PathSearchFeedback::Continue => (),
        }
        // the meta was acceptable, attempt to add it to best_metas
        if let Some(metas) = best_metas.get(&pos) {
            if metas.iter().any(|m| m.compare(&meta) == Some(Ordering::Less)) {
                // we already found a strictly better meta
                continue;
            }
        }
        let mut set = HashSet::new();
        if let Some(mut metas) = best_metas.remove(&pos) {
            for m in metas.drain() {
                if meta.compare(&m) != Some(Ordering::Less) {
                    // new meta isn't strictly better
                    set.insert(m);
                }
            }
        }
        set.insert(meta.clone());
        best_metas.insert(pos, set);
        // the meta was good enough, let's find its next neighbors
        let prev_terrain = map.get_terrain(pos).unwrap();
        for (dp, step) in find_steps(pos, meta.movement_type) {
            let terrain = if let Some(terrain) = map.get_terrain(dp.point) {
                terrain
            } else {
                continue;
            };
            let mut next_meta = if let Some(cost) = terrain.movement_cost(meta.movement_type) {
                if step.dir().is_some() && step.dir() == meta.illegal_next_dir {
                    // don't turn around 180°
                    None
                } else if let Some((movement_type, remaining_movement)) = update_movement(prev_terrain, terrain, meta.movement_type, meta.remaining_movement, cost) {
                    let mut illegal_next_dir = None;
                    if step.dir().is_some() {
                        illegal_next_dir = Some(dp.direction.opposite_direction());
                    }
                    let mut path = meta.path.clone();
                    path.steps.push(step.clone());
                    Some(MovementSearchMeta {
                        previous_turns: meta.previous_turns.clone(),
                        movement_type,
                        remaining_movement,
                        path,
                        illegal_next_dir,
                    })
                } else {
                    None
                }
            } else {
                // doesn't allow automatic changing of movement_type between turns
                None
            };
            if next_meta.is_none() && meta.previous_turns.len() < additional_turns {
                // no meta was found, but maybe next turn we'll have enough moveement
                // points. the illegal_next_dir also gets reset
                let (movement_type, points) = base_movement(prev_terrain, Some(meta.movement_type));
                if let Some((movement_type, remaining_movement)) = terrain.movement_cost(movement_type)
                .and_then(|cost| update_movement(prev_terrain, terrain, movement_type, points, cost)) {
                    let mut previous_turns = meta.previous_turns.clone();
                    previous_turns.push(meta.path.clone());
                    let mut illegal_next_dir = None;
                    if step.dir().is_some() {
                        illegal_next_dir = Some(dp.direction.opposite_direction());
                    }
                    let mut path = Path::new(pos);
                    path.steps.push(step);
                    next_meta = Some(MovementSearchMeta {
                        previous_turns,
                        movement_type,
                        remaining_movement,
                        path,
                        illegal_next_dir,
                    });
                }
            }
            if let Some(meta) = next_meta {
                next_checks.push(MovementSearch {
                    pos: dp.point,
                    meta,
                });
            }
        }
    }
    None
}

pub fn movement_search<D, F>(game: &Game<D>, unit: &NormalUnit, path_so_far: &Path<D>, vision: Option<&HashMap<Point, Vision>>, mut callback: F)
where D: Direction, F: FnMut(&Path<D>, Point, bool) -> PathSearchFeedback {
    search_path(
        game.get_map(),
        path_so_far.start,
        0,
        |terrain, _| unit.get_movement(terrain),
        |prev_terrain, next_terrain, movement_type, remaining_points, cost| {
            let remaining_points = if cost <= remaining_points {
                remaining_points - cost
            } else {
                return None;
            };
            let movement_type = if let Some(m) = next_terrain.update_movement_type(movement_type, prev_terrain) {
                m
            } else {
                return None;
            };
            Some((movement_type, remaining_points))
        },
        |point, movement_type| {
            game.get_map().get_unit_movement_neighbors(point, movement_type)
        },
        |path, destination| {
            if path.steps.len() <= path_so_far.steps.len() {
                // first follow path_so_far until its end, then the search can start
                if path.steps[..] != path_so_far.steps[..path.steps.len()] {
                    return PathSearchFeedback::Rejected;
                } else if path.steps.len() < path_so_far.steps.len() {
                    return PathSearchFeedback::Continue;
                }
            }
            let can_stop_here = if let Some(blocking_unit) = game.get_map().get_unit(destination) {
                let hidden_by_fog = vision.and_then(|vision| Some(match vision.get(&destination) {
                    None => true,
                    Some(Vision::Normal) => blocking_unit.has_stealth() || game.get_map().get_terrain(destination).unwrap().hides_unit(&blocking_unit),
                    Some(Vision::TrueSight) => false,
                })).unwrap_or(false);
                let is_self = path_so_far.start == destination && blocking_unit == &unit.as_unit();
                if !hidden_by_fog && !is_self && !blocking_unit.can_be_moved_through(unit, game) {
                    return PathSearchFeedback::Rejected;
                }
                is_self || hidden_by_fog
            } else {
                true
            };
            callback(path, destination, can_stop_here)
        },
    );
}

