use std::collections::{BinaryHeap, HashSet, HashMap};
use std::cmp::Ordering;
use std::fmt::Debug;
use std::hash::Hash;
use std::ops::{Add, Sub, AddAssign, SubAssign};

use zipper::*;
use zipper_derive::*;

use crate::commanders::Commander;
use crate::game::events::CommandError;
use crate::map::direction::Direction;
use crate::map::point::Point;
use crate::game::game::{Game, Vision};
use crate::map::map::*;
use crate::map::wrapping_map::OrientedPoint;
use crate::terrain::Terrain;
use crate::units::chess::*;

use super::normal_units::{NormalUnits, NormalUnit};
use super::{chess, UnitType};

#[derive(Debug, PartialEq, Eq)]
pub enum PathSearchFeedback {
    Continue,
    ContinueWithoutStopping,
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
        Ok(self.progress_reversible(map, pos)?.0.point)
    }
    pub fn progress_reversible(&self, map: &Map<D>, pos: Point) -> Result<(OrientedPoint<D>, Self), CommandError> {
        match self {
            Self::Dir(d) => {
                if let Some(o) = map.get_neighbor(pos, *d) {
                    Ok((o.clone(), Self::Dir(o.direction.opposite_direction())))
                } else {
                    Err(CommandError::InvalidPath)
                }
            }
            Self::Jump(d) => {
                if let Some(o) = map.get_neighbor(pos, *d).and_then(|o| map.get_neighbor(o.point, o.direction)) {
                    Ok((o.clone(), Self::Jump(o.direction.opposite_direction())))
                } else {
                    Err(CommandError::InvalidPath)
                }
            }
            Self::Diagonal(d) => {
                if let Some(o) = chess::get_diagonal_neighbor(map, pos, *d) {
                    Ok((o.clone(), Self::Diagonal(o.direction.opposite_direction())))
                } else {
                    Err(CommandError::InvalidPath)
                }
            }
            Self::Knight(d, turn_left) => {
                if let Some(o) = chess::get_knight_neighbor(map, pos, *d, *turn_left) {
                    Ok((o.clone(), Self::Knight(o.direction.opposite_direction(), *turn_left != o.mirrored)))
                } else {
                    Err(CommandError::InvalidPath)
                }
            }
            Self::Point(p) => Ok((OrientedPoint::new(*p, false, D::list()[0]), Self::Point(pos))),
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

trait TemporaryBallast<D: Direction>: 'static + Eq + Clone + Debug {
    //self.remaining_movement.cmp(&other.remaining_movement)
    fn heap_order(&self, other: &Self) -> Ordering;
    fn useful_with<'a>(&self, others: impl Iterator<Item = (&'a Self, bool)>, map: &Map<D>, point: Point) -> bool;
}

trait PermanentBallast<D: Direction>: 'static + Eq + Clone + Debug {
    fn worse_or_equal(&self, other: &Self, map: &Map<D>, point: Point) -> bool;
}

impl<D: Direction> PermanentBallast<D> for () {
    fn worse_or_equal(&self, _other: &Self, _map: &Map<D>, _point: Point) -> bool {
        true
    }
}
impl<D: Direction> TemporaryBallast<D> for () {
    fn heap_order(&self, _other: &Self) -> Ordering {
        Ordering::Equal
    }
    fn useful_with<'a>(&self, mut others: impl Iterator<Item = (&'a Self, bool)>, _map: &Map<D>, _point: Point) -> bool {
        !others.next().is_some()
    }
}

impl<D: Direction> TemporaryBallast<D> for MovementPoints {
    fn heap_order(&self, other: &Self) -> Ordering {
        self.cmp(other)
    }
    fn useful_with<'a>(&self, mut others: impl Iterator<Item = (&'a Self, bool)>, _map: &Map<D>, _point: Point) -> bool {
        others.all(|(other, other_is_earlier_turn)| self < other && !other_is_earlier_turn)
    }
}

// for chess pawn: can only move a certain direction when on chess board
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct PawnPermanent<D: Direction>(D);
impl<D: Direction> PermanentBallast<D> for PawnPermanent<D> {
    fn worse_or_equal(&self, other: &Self, map: &Map<D>, point: Point) -> bool {
        if map.get_terrain(point).unwrap().is_chess() {
            self.0 == other.0
        } else {
            // direction doesn't matter here, so counts as equal
            true
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct PawnTemporary<D: Direction> {
    may_take: bool,
    steps_left: u8,
    dir: Option<D>,
}
impl<D: Direction> TemporaryBallast<D> for PawnTemporary<D> {
    fn heap_order(&self, other: &Self) -> Ordering {
        self.steps_left.cmp(&other.steps_left)
    }
    fn useful_with<'a>(&self, mut others: impl Iterator<Item = (&'a Self, bool)>, _map: &Map<D>, _point: Point) -> bool {
        others.all(|(other, other_is_earlier_turn)| {
            other.dir.is_some() && self.dir != other.dir
            || self.steps_left > other.steps_left && !other_is_earlier_turn
            || self.may_take && !other.may_take
        })
    }
}

// ...and other chess units that keep moving straight after the first step
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct ChessTemporary<D: Direction>(MovementPoints, Option<D>);
impl<D: Direction> TemporaryBallast<D> for ChessTemporary<D> {
    fn heap_order(&self, other: &Self) -> Ordering {
        self.0.cmp(&other.0)
    }
    fn useful_with<'a>(&self, mut others: impl Iterator<Item = (&'a Self, bool)>, _map: &Map<D>, _point: Point) -> bool {
        others.all(|(other, other_is_earlier_turn)| !other_is_earlier_turn && (self.0 > other.0 || other.1.is_some() && self.1 != other.1))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct QueenTemporary<D: Direction>(MovementPoints, Option<PathStep<D>>);
impl<D: Direction> TemporaryBallast<D> for QueenTemporary<D> {
    fn heap_order(&self, other: &Self) -> Ordering {
        self.0.cmp(&other.0)
    }
    fn useful_with<'a>(&self, mut others: impl Iterator<Item = (&'a Self, bool)>, _map: &Map<D>, _point: Point) -> bool {
        others.all(|(other, other_is_earlier_turn)| !other_is_earlier_turn && (self.0 > other.0 || other.1.is_some() && self.1 != other.1))
    }
}

impl<D: Direction> PermanentBallast<D> for MovementType {
    fn worse_or_equal(&self, other: &Self, _map: &Map<D>, _point: Point) -> bool {
        *self == *other
    }
}

// for normal units: can't turn around 180°
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct NormalBallast<D: Direction> {
    points: MovementPoints,
    forbidden_dir: Option<D>,
}
impl<D: Direction> TemporaryBallast<D> for NormalBallast<D> {
    fn heap_order(&self, other: &Self) -> Ordering {
        self.points.cmp(&other.points)
    }
    fn useful_with<'a>(&self, others: impl Iterator<Item = (&'a Self, bool)>, _map: &Map<D>, _point: Point) -> bool {
        let mut found: Option<D> = None;
        for (other, other_is_earlier_turn) in others {
            if other_is_earlier_turn {
                return false;
            }
            match (self.forbidden_dir, other.forbidden_dir) {
                (_, None) => return false,
                (None, Some(_)) => (),
                (Some(blocked), Some(other)) => {
                    if blocked == other {
                        return false;
                    }
                    if found.is_some() && found != Some(other) {
                        // found 2 other steps that reach here
                        // a third isn't needed
                        return false
                    } else {
                        found = Some(other);
                    }
                }
            }
        }
        true
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct MovementSearchMeta<D: Direction, P: PermanentBallast<D>, T: TemporaryBallast<D>> {
    previous_turns: Vec<Path<D>>,
    path: Path<D>,
    permanent: P,
    temporary: T,
}
impl<D: Direction, P: PermanentBallast<D>, T: TemporaryBallast<D>> MovementSearchMeta<D, P, T> {
    fn heap_order(&self, other: &Self) -> Ordering {
        if self.previous_turns.len() == other.previous_turns.len() {
            let tmp = self.temporary.heap_order(&other.temporary);
            if tmp == Ordering::Equal {
                other.path.steps.len().cmp(&self.path.steps.len())
            } else {
                tmp
            }
        } else {
            other.previous_turns.len().cmp(&self.previous_turns.len())
        }
    }
    fn useful_with<'a>(&self, others: impl Iterator<Item = &'a Self>, map: &Map<D>, point: Point) -> bool {
        // search for items that are at least as good as this one
        let relevant: Vec<(&T, bool)> = others.filter(|other| {
            self.previous_turns.len() >= other.previous_turns.len()
            && self.permanent.worse_or_equal(&other.permanent, map, point)
            && self <= other
        }).map(|other| {
            (&other.temporary, self.previous_turns.len() > other.previous_turns.len())
        }).collect();
        if relevant.len() == 0 {
            true
        } else {
            self.temporary.useful_with(relevant.into_iter(), map, point)
        }
    }
}



impl<D: Direction, P: PermanentBallast<D>, T: TemporaryBallast<D>> PartialOrd for MovementSearchMeta<D, P, T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.heap_order(other))
    }
}
impl<D: Direction, P: PermanentBallast<D>, T: TemporaryBallast<D>> Ord for MovementSearchMeta<D, P, T> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.heap_order(other)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct MovementSearch<D: Direction, P: PermanentBallast<D>, T: TemporaryBallast<D>> {
    pos: Point,
    meta: MovementSearchMeta<D, P, T>,
}
impl<D: Direction, P: PermanentBallast<D>, T: TemporaryBallast<D>> PartialOrd for MovementSearch<D, P, T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.meta.partial_cmp(&other.meta)
    }
}
impl<D: Direction, P: PermanentBallast<D>, T: TemporaryBallast<D>> Ord for MovementSearch<D, P, T> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.meta.cmp(&other.meta)
    }
}

fn find_normal_steps<D: Direction>(map: &Map<D>, point: Point, blocked: Option<D>) -> Vec<PathStep<D>> {
    let mut result = Vec::new();
    for d in D::list() {
        if Some(d) == blocked {
            continue;
        }
        result.push(PathStep::Dir(d));
        if map.get_terrain(point) == Some(&Terrain::Fountain) {
            result.push(PathStep::Jump(d));
        }
    }
    result
}

// units can have two types of extra ballast
// - changes to themselves (stay after end-turn)
//      pawn direction or hover_mode for Hoverbikes
// - previous steps of a path excluding some next steps (reset after end-turn)
//      chess units moving only straight
//      normal units unable to turn around 180°
//      movement_points for most units
// 
// changes to themself s

fn movement_search_core<D, P, T, CanStartFrom, BaseMovement, FindSteps, DoStep, CALLBACK>(
    map: &Map<D>,
    start: Point,
    additional_turns: usize,
    can_start_from: CanStartFrom,
    base_movement: BaseMovement,
    find_steps: FindSteps,
    do_step: DoStep,
    mut callback: CALLBACK
) -> Option<Path<D>>
where
    D: Direction,
    P: PermanentBallast<D>, // permanent, stays after end-turn
    T: TemporaryBallast<D>, // temporary, gets reset after end-turn
    CanStartFrom: Fn(&Terrain<D>) -> bool,
    BaseMovement: Fn(&Terrain<D>, Option<&P>, usize) -> (P, T),
    FindSteps: Fn(Point, bool, &P, &T) -> Vec<PathStep<D>>,
    DoStep: Fn(Point, PathStep<D>, &P, &T) -> Option<(Point, P, T)>,
    CALLBACK: FnMut(&[Path<D>], &Path<D>, Point) -> PathSearchFeedback,
{
    let start_terrain = map.get_terrain(start).expect(&format!("Map doesn't have terrain at {:?}", start));
    if !can_start_from(&start_terrain) {
        let path = Path::new(start);
        return match callback(&[], &path, start) {
            PathSearchFeedback::Found => Some(path),
            _ => None,
        };
    }
    let (permanent, temporary) = base_movement(start_terrain, None, 0);
    // some ways that arrive at the same point may be incomparable
    // (better in one way, worse in another)
    // so for each point, a HashSet is used to store the best paths
    let mut best_metas: HashMap<Point, Vec<MovementSearchMeta<D, P, T>>> = HashMap::new();
    let mut next_checks: BinaryHeap<MovementSearch<D, P, T>> = BinaryHeap::with_capacity(map.all_points().len());
    next_checks.push(MovementSearch {
        pos: start,
        meta: MovementSearchMeta {
            previous_turns: Vec::new(),
            path: Path::new(start),
            permanent,
            temporary,
        }
    });
    while let Some(MovementSearch { pos, meta }) = next_checks.pop() {
        // check if our current meta is accepted by the callback
        let can_stop = match callback(&meta.previous_turns, &meta.path, pos) {
            PathSearchFeedback::Found => return Some(meta.path),
            PathSearchFeedback::Rejected => continue,
            PathSearchFeedback::Continue => true,
            PathSearchFeedback::ContinueWithoutStopping => false,
        };
        // the meta was acceptable, attempt to add it to best_metas
        if let Some(metas) = best_metas.get(&pos) {
            if !meta.useful_with(metas.iter(), map, pos) {
                // we already found a strictly better meta
                continue;
            }
        };
        let mut set = Vec::new();
        set.push(meta.clone());
        if let Some(metas) = best_metas.remove(&pos) {
            for m in metas {
                if m.useful_with(set.iter(), map, pos) {
                    // new meta isn't strictly better
                    set.push(m);
                }
            }
        }
        best_metas.insert(pos, set);
        // the meta was good enough, let's find its next neighbors
        let mut steps_used = HashSet::new();
        for step in find_steps(pos, meta.path.steps.len() == 0, &meta.permanent, &meta.temporary) {
            let (next_point, permanent, temporary) = match do_step(pos, step, &meta.permanent, &meta.temporary) {
                None => continue,
                Some(data) => data,
            };
            steps_used.insert(step);
            let mut path = meta.path.clone();
            path.steps.push(step);
            steps_used.insert(step);
            next_checks.push(MovementSearch {
                pos: next_point,
                meta: MovementSearchMeta {
                    previous_turns: meta.previous_turns.clone(),
                    path,
                    permanent,
                    temporary,
                }
            });
        }
        // add steps that become possible in the next round
        if can_stop && meta.previous_turns.len() < additional_turns {
            let prev_terrain = map.get_terrain(pos).unwrap();
            let (permanent, temporary) = base_movement(prev_terrain, Some(&meta.permanent), meta.previous_turns.len() + 1);
            let path = Path::new(pos);
            for step in find_steps(pos, true, &permanent, &temporary) {
                if steps_used.contains(&step) {
                    continue;
                }
                let (next_point, permanent, temporary) = match do_step(pos, step, &permanent, &temporary) {
                    None => continue,
                    Some(data) => data,
                };
                let mut previous_turns = meta.previous_turns.clone();
                previous_turns.push(meta.path.clone());
                let mut path = path.clone();
                path.steps.push(step);
                next_checks.push(MovementSearch {
                    pos: next_point,
                    meta: MovementSearchMeta {
                        previous_turns,
                        path,
                        permanent,
                        temporary,
                    }
                });
            }
        }
    }
    None
}

fn movement_search_map<D: Direction, Callback, TransformMovementPoints, TransformMovementCost>(
    map: &Map<D>,
    unit: &UnitType<D>,
    start: Point,
    rounds: usize,
    callback: Callback,
    transform_movement_points: TransformMovementPoints,
    transform_movement_cost: TransformMovementCost,
)
where
   Callback: FnMut(&[Path<D>], &Path<D>, Point) -> PathSearchFeedback,
   TransformMovementPoints: Fn(MovementPoints, usize) -> MovementPoints,
   TransformMovementCost: Fn(MovementPoints, &NormalUnit, MovementType) -> MovementPoints,
{
    if rounds > 0 {
        match unit {
            UnitType::Structure(_) => return,
            UnitType::Normal(unit) if unit.changes_movement_type() => {
                let (starting_movement_type, movement_points) = unit.get_movement(map.get_terrain(start).expect(&format!("Map doesn't have terrain at {:?}", start)));
                let base_movement = |_terrain: &Terrain<D>, permanent: Option<&MovementType>, round: usize| {
                    (permanent.cloned().unwrap_or(starting_movement_type), NormalBallast { points: transform_movement_points(movement_points, round), forbidden_dir: None })
                };
                movement_search_core(
                    map,
                    start,
                    rounds - 1,
                    |terrain| {
                        terrain.movement_cost(starting_movement_type).is_some()
                    },
                    base_movement,
                    |point, _, _, temporary: &NormalBallast<D>| {
                        // TODO: add movement type if Fountain only affects water units
                        find_normal_steps(map, point, temporary.forbidden_dir)
                    },
                    |point, step, permanent: &MovementType, temporary: &NormalBallast<D>| {
                        if let Ok((dp, _)) = step.progress_reversible(map, point) {
                            let terrain = map.get_terrain(dp.point).unwrap();
                            if let Some(cost) = terrain.movement_cost(*permanent) {
                                let cost = transform_movement_cost(cost, unit, *permanent);
                                if cost <= temporary.points {
                                    if let Some(movement_type) = terrain.update_movement_type(*permanent, map.get_terrain(point).unwrap()) {
                                        return Some((dp.point, movement_type, NormalBallast { points: temporary.points - cost, forbidden_dir: Some(dp.direction.opposite_direction()) }));
                                    }
                                }
                            }
                        }
                        None
                    },
                    callback
                );
            }
            UnitType::Normal(unit) => {
                let (movement_type, movement_points) = unit.get_movement(map.get_terrain(start).expect(&format!("Map doesn't have terrain at {:?}", start)));
                let base_movement = |_terrain: &Terrain<D>, _permanent: Option<&()>, round: usize| {
                    ((), NormalBallast { points: transform_movement_points(movement_points, round), forbidden_dir: None })
                };
                movement_search_core(
                    map,
                    start,
                    rounds - 1,
                    |terrain| {
                        terrain.movement_cost(movement_type).is_some()
                    },
                    base_movement,
                    |point, _, _, temporary: &NormalBallast<D>| {
                        // TODO: add movement type if Fountain only affects water units
                        find_normal_steps(map, point, temporary.forbidden_dir)
                    },
                    |point, step, _permanent: &(), temporary: &NormalBallast<D>| {
                        if let Ok((dp, _)) = step.progress_reversible(map, point) {
                            let terrain = map.get_terrain(dp.point).unwrap();
                            if let Some(cost) = terrain.movement_cost(movement_type) {
                                // TODO: preventing beach <-> bridge only needs the prev Terrain
                                // if the current one is either bridge or beach
                                if cost <= temporary.points && terrain.update_movement_type(movement_type, map.get_terrain(point).unwrap()).is_some() {
                                    return Some((dp.point, (), NormalBallast { points: temporary.points - cost, forbidden_dir: Some(dp.direction.opposite_direction()) }));
                                }
                            }
                        }
                        None
                    },
                    callback
                );
            }
            UnitType::Chess(ChessUnit { typ: ChessUnits::Pawn(starting_dir, _), .. }) => {
                movement_search_core(
                    map,
                    start,
                    rounds - 1,
                    |terrain| {
                        terrain.movement_cost(MovementType::Chess).is_some()
                    },
                    Box::new(|terrain: &Terrain<D>, permanent: Option<&PawnPermanent<D>>, _| {
                        let permanent = permanent.cloned().unwrap_or(PawnPermanent(*starting_dir));
                        let steps_left = if *terrain == Terrain::ChessPawnTile {
                            2
                        } else {
                            1
                        };
                        let dir = if terrain.is_chess() {
                            Some(permanent.0)
                        } else {
                            None
                        };
                        let temporary = PawnTemporary {
                            may_take: true,
                            steps_left,
                            dir,
                        };
                        (permanent, temporary)
                    }),
                    |_point, _first_step: bool, permanent: &PawnPermanent<D>, temporary: &PawnTemporary<D>| {
                        if temporary.steps_left == 0 {
                            return Vec::new();
                        }
                        let mut steps = Vec::new();
                        let directions = if let Some(dir) = temporary.dir {
                            vec![dir]
                        } else {
                            D::list()
                        };
                        for d in directions {
                            steps.push(PathStep::Dir(d));
                        }
                        if temporary.may_take {
                            let dir = temporary.dir.unwrap_or(permanent.0);
                            steps.push(PathStep::Diagonal(dir));
                            steps.push(PathStep::Diagonal(dir.rotate(true)));
                        }
                        steps
                    },
                    |point, step, permanent: &PawnPermanent<D>, temporary: &PawnTemporary<D>| {
                        if let Ok((dp, _)) = step.progress_reversible(map, point) {
                            let terrain = map.get_terrain(dp.point).unwrap();
                            if terrain.movement_cost(MovementType::Chess).is_some() {
                                let mut direction = dp.direction;
                                let mut steps_left = temporary.steps_left - 1;
                                if let PathStep::Diagonal(d) = step {
                                    steps_left = 0;
                                    if d != temporary.dir.unwrap_or(permanent.0) {
                                        direction = direction.rotate(dp.mirrored);
                                    }
                                }
                                return Some((dp.point, PawnPermanent(direction), PawnTemporary { steps_left, dir: Some(direction), may_take: false }));
                            }
                        }
                        None
                    },
                    callback
                );
            }
            UnitType::Chess(ChessUnit { typ: typ @ ChessUnits::King(_), .. }) |
            UnitType::Chess(ChessUnit { typ: typ @ ChessUnits::Knight, .. }) => {
                movement_search_core(
                    map,
                    start,
                    rounds - 1,
                    |terrain| {
                        terrain.movement_cost(MovementType::Chess).is_some()
                    },
                    |_terrain, _, _| ((), ()),
                    |_point, first_step: bool, _permanent, _temporary| {
                        if !first_step {
                            Vec::new()
                        } else if *typ == ChessUnits::Knight {
                            find_knight_steps()
                        } else {
                            find_king_steps()
                        }
                    },
                    |point, step, _: &(), _temporary: &()| {
                        if let Ok(p) = step.progress(map, point) {
                            let terrain = map.get_terrain(p).unwrap();
                            if terrain.movement_cost(MovementType::Chess).is_some() {
                                return Some((p, (), ()));
                            }
                        }
                        None
                    },
                    callback
                );
            }
            UnitType::Chess(ChessUnit { typ: ChessUnits::Queen, .. }) => {
                movement_search_core(
                    map,
                    start,
                    rounds - 1,
                    |terrain| {
                        terrain.movement_cost(MovementType::Chess).is_some()
                    },
                    |_terrain, _, _| ((), QueenTemporary(MovementPoints::from(8.), None)),
                    |_point, _, _, temporary| {
                        find_queen_steps(temporary.1)
                    },
                    |point, step, _, temporary: &QueenTemporary<D>| {
                        if let Ok((dp, reverse)) = step.progress_reversible(map, point) {
                            let terrain = map.get_terrain(dp.point).unwrap();
                            if let Some(cost) = terrain.movement_cost(MovementType::Chess) {
                                if cost <= temporary.0 {
                                    return Some((dp.point, (), QueenTemporary(temporary.0 - cost, Some(reverse))));
                                }
                            }
                        }
                        None
                    },
                    callback
                );
            }
            UnitType::Chess(ChessUnit { typ: typ @ ChessUnits::Bishop, .. }) |
            UnitType::Chess(ChessUnit { typ: typ @ ChessUnits::Rook(_), .. }) => {
                movement_search_core(
                    map,
                    start,
                    rounds - 1,
                    |terrain| {
                        terrain.movement_cost(MovementType::Chess).is_some()
                    },
                    |_terrain, _, _| ((), ChessTemporary(MovementPoints::from(8.), None)),
                    |_point, _, _, temporary| {
                        if *typ == ChessUnits::Bishop {
                            find_bishop_steps(temporary.1)
                        } else {
                            find_rook_steps(temporary.1)
                        }
                    },
                    |point, step, _, temporary: &ChessTemporary<D>| {
                        if let Ok((dp, _)) = step.progress_reversible(map, point) {
                            let terrain = map.get_terrain(dp.point).unwrap();
                            if let Some(cost) = terrain.movement_cost(MovementType::Chess) {
                                if cost <= temporary.0 {
                                    return Some((dp.point, (), ChessTemporary(temporary.0 - cost, Some(dp.direction))));
                                }
                            }
                        }
                        None
                    },
                    callback
                );
            }
        };
    }
}

pub fn movement_search_game<D: Direction, F>(game: &Game<D>, unit: &UnitType<D>, start: Point, rounds: usize, callback: F)
where F: FnMut(&[Path<D>], &Path<D>, Point) -> PathSearchFeedback {
    let player = unit.get_owner().and_then(|owner| game.get_owning_player(owner));
    let commander = player.and_then(|player| Some(&player.commander)).unwrap_or(&Commander::None);
    movement_search_map(
        game.get_map(),
        unit,
        start,
        rounds,
        callback,
        |movement_points, _round| {
            // ignores that powers end after some rounds
            movement_points + commander.movement_bonus(unit)
        },
        |cost, unit, movement_type| {
            commander.transform_movement_cost(unit, movement_type, cost)
        }
    )
}

fn movement_search_map_without_game<D: Direction, F>(map: &Map<D>, unit: &UnitType<D>, start: Point, rounds: usize, callback: F)
where F: FnMut(&[Path<D>], &Path<D>, Point) -> PathSearchFeedback {
    movement_search_map(
        map,
        unit,
        start,
        rounds,
        callback,
        |mp, _| mp,
        |cost, _, _| cost,
    )
}

pub fn movement_area_map<D: Direction>(map: &Map<D>, unit: &UnitType<D>, path_so_far: &Path<D>, rounds: usize) -> HashMap<Point, usize> {
    let mut result = HashMap::new();
    let callback = |previous_turns: &[Path<D>], path: &Path<D>, point| {
        if previous_turns.len() == 0 && path.steps.len() <= path_so_far.steps.len() && path.steps[..] != path_so_far.steps[..path.steps.len()] {
            return PathSearchFeedback::Rejected;
        }
        // movement_area_map ignores units
        if let UnitType::Chess(ChessUnit { typ: ChessUnits::Pawn(_, _), .. }) = unit {
            if let Some(PathStep::Diagonal(_)) = path.steps.last() {
                return PathSearchFeedback::Rejected;
            }
        }
        if !result.contains_key(&point) {
            result.insert(point, previous_turns.len());
        }
        PathSearchFeedback::Continue
    };
    movement_search_map_without_game(map, unit, path_so_far.start, rounds, callback);
    result
}

pub fn movement_area_game<D: Direction>(game: &Game<D>, unit: &UnitType<D>, path_so_far: &Path<D>, rounds: usize) -> HashMap<Point, usize> {
    let mut result = HashMap::new();
    let callback = |previous_turns: &[Path<D>], path: &Path<D>, destination| {
        if previous_turns.len() == 0 && path.steps.len() <= path_so_far.steps.len() && path.steps[..] != path_so_far.steps[..path.steps.len()] {
            return PathSearchFeedback::Rejected;
        }
        let mut can_stop_here = true;
        let mut can_continue = true;
        if let Some(blocking_unit) = game.get_map().get_unit(destination) {
            can_stop_here = false;
            let is_self = path_so_far.start == destination && blocking_unit == unit;
            if !is_self {
                match unit {
                    UnitType::Normal(unit) => {
                        if !blocking_unit.can_be_moved_through(unit, game) {
                            return PathSearchFeedback::Rejected;
                        }
                    }
                    UnitType::Chess(ChessUnit { typ: ChessUnits::Pawn(_, _), owner, .. }) => {
                        if let Some(PathStep::Dir(_)) = path.steps.last() {
                            return PathSearchFeedback::Rejected;
                        }
                        if !blocking_unit.can_be_taken_by_chess(game, *owner) {
                            return PathSearchFeedback::Rejected;
                        }
                        can_continue = false;
                    }
                    UnitType::Chess(unit) => {
                        if !blocking_unit.can_be_taken_by_chess(game, unit.owner) {
                            return PathSearchFeedback::Rejected;
                        }
                        can_continue = false;
                    }
                    _ => (),
                }
            }
        } else if let UnitType::Chess(ChessUnit { typ: ChessUnits::Pawn(_, _), .. }) = unit {
            if let Some(PathStep::Diagonal(_)) = path.steps.last() {
                return PathSearchFeedback::Rejected;
            }
        }
        if !result.contains_key(&destination) {
            result.insert(destination, previous_turns.len());
        }
        if can_continue {
            if can_stop_here {
                PathSearchFeedback::ContinueWithoutStopping
            } else {
                PathSearchFeedback::Continue
            }
        } else {
            PathSearchFeedback::Rejected
        }
    };
    movement_search_game(game, unit, path_so_far.start, rounds, callback);
    result
}

pub fn search_path<D: Direction, F>(game: &Game<D>, unit: &UnitType<D>, path_so_far: &Path<D>, vision: Option<&HashMap<Point, Vision>>, callback: F) -> Option<Path<D>>
where F: Fn(&Path<D>, Point, bool) -> PathSearchFeedback {
    let mut result = None;
    movement_search_game(game, unit, path_so_far.start, 1, |_, path, destination| {
        if path.steps.len() <= path_so_far.steps.len() {
            if path.steps[..] != path_so_far.steps[..path.steps.len()] {
                return PathSearchFeedback::Rejected;
            } else if path.steps.len() < path_so_far.steps.len() {
                return PathSearchFeedback::Continue;
            }
        }
        let mut can_stop_here = true;
        let mut can_continue = true;
        if let Some(blocking_unit) = game.get_map().get_unit(destination) {
            let hidden_by_fog = vision.and_then(|vision| Some(match vision.get(&destination) {
                None => true,
                Some(Vision::Normal) => blocking_unit.has_stealth() || game.get_map().get_terrain(destination).unwrap().hides_unit(&blocking_unit),
                Some(Vision::TrueSight) => false,
            })).unwrap_or(false);
            let is_self = path_so_far.start == destination && blocking_unit == unit;
            if !hidden_by_fog && !is_self {
                match unit {
                    UnitType::Normal(unit) => {
                        if !blocking_unit.can_be_moved_through(unit, game) {
                            return PathSearchFeedback::Rejected;
                        }
                        can_stop_here = false;
                    }
                    UnitType::Chess(ChessUnit { typ: ChessUnits::Pawn(_, _), owner, .. }) => {
                        if let Some(PathStep::Dir(_)) = path.steps.last() {
                            return PathSearchFeedback::Rejected;
                        }
                        if !blocking_unit.can_be_taken_by_chess(game, *owner) {
                            return PathSearchFeedback::Rejected;
                        }
                        can_continue = false;
                    }
                    UnitType::Chess(unit) => {
                        if !blocking_unit.can_be_taken_by_chess(game, unit.owner) {
                            return PathSearchFeedback::Rejected;
                        }
                        can_continue = false;
                    }
                    _ => (),
                }
            }
        } else if let UnitType::Chess(ChessUnit { typ: ChessUnits::Pawn(_, _), .. }) = unit {
            if let Some(PathStep::Diagonal(_)) = path.steps.last() {
                return PathSearchFeedback::Rejected;
            }
        }
        let feedback = callback(path, destination, can_stop_here);
        if feedback == PathSearchFeedback::Found {
            result = Some(path.clone());
        } else if feedback == PathSearchFeedback::Continue && !can_continue {
            return PathSearchFeedback::Rejected;
        } else if feedback == PathSearchFeedback::Continue && !can_stop_here {
            return PathSearchFeedback::ContinueWithoutStopping;
        }
        feedback
    });
    result
}

