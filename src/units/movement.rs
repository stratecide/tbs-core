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


#[derive(Debug, Clone, Copy, PartialEq, Eq, Zippable, Hash)]
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
        Ok(self.progress_reversible(map, pos)?.0)
    }
    pub fn progress_reversible(&self, map: &Map<D>, pos: Point) -> Result<(Point, Self), CommandError> {
        match self {
            Self::Dir(d) => {
                if let Some(o) = map.get_neighbor(pos, *d) {
                    Ok((o.point, Self::Dir(o.direction.opposite_direction())))
                } else {
                    Err(CommandError::InvalidPath)
                }
            }
            Self::Jump(d) => {
                if let Some(o) = map.get_neighbor(pos, *d).and_then(|o| map.get_neighbor(o.point, o.direction)) {
                    Ok((o.point, Self::Jump(o.direction.opposite_direction())))
                } else {
                    Err(CommandError::InvalidPath)
                }
            }
            Self::Diagonal(d) => {
                if let Some(o) = chess::get_diagonal_neighbor(map, pos, *d) {
                    Ok((o.point, Self::Diagonal(o.direction.opposite_direction())))
                } else {
                    Err(CommandError::InvalidPath)
                }
            }
            Self::Knight(d, turn_left) => {
                if let Some(o) = chess::get_knight_neighbor(map, pos, *d, *turn_left) {
                    Ok((o.point, Self::Knight(o.direction.opposite_direction(), *turn_left != o.mirrored)))
                } else {
                    Err(CommandError::InvalidPath)
                }
            }
            Self::Point(p) => Ok((*p, Self::Point(pos))),
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
    pub fn blocks(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Dir(d1) | Self::Jump(d1), Self::Dir(d2) | Self::Jump(d2)) => d1 == d2,
            _ => self == other,
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
    blocked_step: Option<PathStep<D>>, // units aren't allowed to turn around 180°
    previous_turns: Vec<Path<D>>,
    remaining_movement: MovementPoints,
    path: Path<D>,
}
impl<D: Direction> MovementSearchMeta<D> {
    fn heap_order(&self, other: &Self) -> Ordering {
        if self.previous_turns.len() == other.previous_turns.len() {
            self.remaining_movement.cmp(&other.remaining_movement)
        } else {
            other.previous_turns.len().cmp(&self.previous_turns.len())
        }
    }
    fn useful_with(&self, other: &Self) -> bool {
        if self.movement_type != other.movement_type {
            return true;
        }
        match (self.blocked_step, other.blocked_step) {
            (None, Some(_)) => return true,
            (Some(a), Some(b)) => {
                if a != b {
                    return true;
                }
            }
            _ => (),
        }
        self > other
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

fn find_normal_steps<D: Direction>(map: &Map<D>, point: Point, _movement_type: MovementType) -> HashSet<PathStep<D>> {
    let mut result = HashSet::new();
    for d in D::list() {
        result.insert(PathStep::Dir(d));
        if map.get_terrain(point) == Some(&Terrain::Fountain) {
            result.insert(PathStep::Jump(d));
        }
    }
    result
}

fn update_movement_default<D: Direction>(_prev_terrain: &Terrain<D>, _next_terrain: &Terrain<D>, movement_type: MovementType, remaining_points: MovementPoints, cost: MovementPoints) -> Option<(MovementType, MovementPoints)> {
    let remaining_points = if cost <= remaining_points {
        Some(remaining_points - cost)
    } else {
        None
    }?;
    Some((movement_type, remaining_points))
}

type BaseMovement<'a, D> = dyn Fn(&Terrain<D>, Option<MovementType>) -> (MovementType, MovementPoints) + 'a;
type UpdateMovement<'a, D> = dyn Fn(&Terrain<D>, &Terrain<D>, MovementType, MovementPoints, MovementPoints) -> Option<(MovementType, MovementPoints)> + 'a;
type FindSteps<'a, D> = dyn Fn(&Path<D>, Point, MovementType, Option<PathStep<D>>) -> HashSet<PathStep<D>> + 'a;
type SearchPathCallback<'a, D> = dyn FnMut(&[Path<D>], &Path<D>, Point) -> PathSearchFeedback + 'a;

fn search_path<D>(
    map: &Map<D>,
    start: Point,
    additional_turns: usize,
    base_movement: Box<BaseMovement<D>>,
    update_movement: Box<UpdateMovement<D>>,
    find_steps: Box<FindSteps<D>>,
    mut callback: Box<SearchPathCallback<D>>
) -> Option<Path<D>>
where
    D: Direction,
{
    // if the unit can't move from it's position, no need to go further
    let (movement_type, points) = if let Some(terrain) = map.get_terrain(start) {
        let (movement_type, points) = base_movement(terrain, None);
        if terrain.movement_cost(movement_type).is_none() {
            let path = Path::new(start);
            return match callback(&[], &path, start) {
                PathSearchFeedback::Found => Some(path),
                _ => None,
            };
        }
        (movement_type, points)
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
            blocked_step: None,
            path: Path::new(start),
        }
    });
    while let Some(MovementSearch { pos, meta }) = next_checks.pop() {
        // check if our current meta is accepted by the callback
        match callback(&meta.previous_turns, &meta.path, pos) {
            PathSearchFeedback::Found => return Some(meta.path),
            PathSearchFeedback::Rejected => continue,
            PathSearchFeedback::Continue => (),
        }
        // the meta was acceptable, attempt to add it to best_metas
        if let Some(metas) = best_metas.get(&pos) {
            if metas.iter().any(|m| !meta.useful_with(m)) {
                // we already found a strictly better meta
                continue;
            }
        }
        let mut set = HashSet::new();
        if let Some(mut metas) = best_metas.remove(&pos) {
            for m in metas.drain() {
                if m.useful_with(&meta) {
                    // new meta isn't strictly better
                    set.insert(m);
                }
            }
        }
        set.insert(meta.clone());
        best_metas.insert(pos, set);
        // the meta was good enough, let's find its next neighbors
        let prev_terrain = map.get_terrain(pos).unwrap();
        let mut steps_used = HashSet::new();
        for step in find_steps(&meta.path, pos, meta.movement_type, meta.blocked_step) {
            let (next_point, blocked_step) = match step.progress_reversible(map, pos) {
                Ok(ok) => ok,
                _ => {
                    steps_used.insert(step);
                    continue;
                }
            };
            let terrain = if let Some(terrain) = map.get_terrain(next_point) {
                terrain
            } else {
                steps_used.insert(step);
                continue;
            };
            if let Some(cost) = terrain.movement_cost(meta.movement_type) {
                if meta.blocked_step.and_then(|s| Some(s.blocks(&step))).unwrap_or(false) {
                    // don't turn around 180°
                    continue;
                } else if let Some((movement_type, remaining_movement)) = update_movement(prev_terrain, terrain, meta.movement_type, meta.remaining_movement, cost) {
                    let mut path = meta.path.clone();
                    path.steps.push(step);
                    steps_used.insert(step);
                    next_checks.push(MovementSearch {
                        pos: next_point,
                        meta: MovementSearchMeta {
                            previous_turns: meta.previous_turns.clone(),
                            movement_type,
                            remaining_movement,
                            path,
                            blocked_step: Some(blocked_step),
                        }
                    });
                }
            }
        }
        // add steps that weren't possible due to missing movement_points or blocked_step
        if meta.previous_turns.len() < additional_turns {
            let (movement_type, points) = base_movement(prev_terrain, Some(meta.movement_type));
            let path = Path::new(pos);
            for step in find_steps(&path, pos, movement_type, None) {
                if steps_used.contains(&step) {
                    continue;
                }
                let (next_point, blocked_step) = match step.progress_reversible(map, pos) {
                    Ok(ok) => ok,
                    _ => {
                        steps_used.insert(step);
                        continue;
                    }
                };
                let terrain = if let Some(terrain) = map.get_terrain(next_point) {
                    terrain
                } else {
                    steps_used.insert(step);
                    continue;
                };
                if let Some((movement_type, remaining_movement)) = terrain.movement_cost(movement_type)
                .and_then(|cost| update_movement(prev_terrain, terrain, movement_type, points, cost)) {
                    let mut previous_turns = meta.previous_turns.clone();
                    previous_turns.push(meta.path.clone());
                    let mut path = path.clone();
                    path.steps.push(step);
                    next_checks.push(MovementSearch {
                        pos: next_point,
                        meta: MovementSearchMeta {
                            previous_turns,
                            movement_type,
                            remaining_movement,
                            path,
                            blocked_step: Some(blocked_step),
                        }
                    });
                }
            }
        }
    }
    None
}

pub fn movement_area<D: Direction>(map: &Map<D>, unit: &UnitType<D>, start: Point, rounds: usize) -> HashMap<Point, usize> {
    let mut result = HashMap::new();
    if rounds > 0 {
        let mut update_movement: Box<dyn Fn(&Terrain<D>, &Terrain<D>, MovementType, MovementPoints, MovementPoints) -> Option<(MovementType, MovementPoints)>> = Box::new(update_movement_default);
        let (base_movement, find_steps): (Box<BaseMovement<D>>, Box<FindSteps<D>>) = match unit {
            UnitType::Structure(_) => return result,
            UnitType::Normal(unit) => {
                if unit.changes_movement_type() {
                    update_movement = Box::new(move |prev_terrain, next_terrain: &Terrain<D>, movement_type, remaining_points, cost| {
                        let mut result = update_movement_default(prev_terrain, next_terrain, movement_type, remaining_points, cost)?;
                        if let Some(m) = next_terrain.update_movement_type(result.0, prev_terrain) {
                            result.0 = m;
                            Some(result)
                        } else {
                            None
                        }
                    });
                }
                (
                    Box::new(|terrain: &Terrain<D>, movement_type| {
                        let mut result = unit.get_movement(terrain);
                        if let Some(mt) = movement_type {
                            result.0 = mt;
                        }
                        result
                    }),
                    Box::new(|_, point, movement_type, _| {
                        find_normal_steps(map, point, movement_type)
                    }),
                )
            }
            UnitType::Chess(unit) => {
                (
                    Box::new(|_, _| {
                        (MovementType::Chess, MovementPoints::from(8.))
                    }),
                    Box::new(|path, point, _, back_step| {
                        unit.find_steps(map, path, point, back_step)
                    }),
                )
            }
        };
        search_path(
            map,
            start,
            rounds - 1,
            base_movement,
            update_movement,
            find_steps,
            Box::new(|previous_turns, _, point| {
                if !result.contains_key(&point) {
                    result.insert(point, previous_turns.len());
                }
                PathSearchFeedback::Continue
            })
        );
    }
    result
}

pub fn movement_search<D, F>(game: &Game<D>, unit: &NormalUnit, path_so_far: &Path<D>, vision: Option<&HashMap<Point, Vision>>, mut callback: F)
where D: Direction, F: FnMut(&Path<D>, Point, bool) -> PathSearchFeedback {
    search_path(
        game.get_map(),
        path_so_far.start,
        0,
        Box::new(|terrain, _| unit.get_movement(terrain)),
        Box::new(|prev_terrain, next_terrain, movement_type, remaining_points, cost| {
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
        }),
        Box::new(|_, point, movement_type, _| {
            find_normal_steps(game.get_map(), point, movement_type)
        }),
        Box::new(|_, path, destination| {
            if path.steps.len() <= path_so_far.steps.len() && path.steps[..] != path_so_far.steps[..path.steps.len()] {
                // first follow path_so_far until its end, then the search can start
                return PathSearchFeedback::Rejected;
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
            if path.steps.len() < path_so_far.steps.len() {
                // first follow path_so_far until its end, then the search can start
                return PathSearchFeedback::Continue;
            }
            callback(path, destination, can_stop_here)
        }),
    );
}

