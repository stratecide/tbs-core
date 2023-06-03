use std::collections::{BinaryHeap, HashSet, HashMap};
use std::cmp::Ordering;
use std::hash::Hash;
use std::ops::{Add, Sub, AddAssign, SubAssign};

use zipper::*;
use zipper_derive::*;

use crate::game::events::CommandError;
use crate::map::direction::Direction;
use crate::map::point::Point;
use crate::game::game::Game;
use crate::map::map::*;

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
            steps.push((on_sea, step.clone())).unwrap();
            prev_terrain = terrain;
        }
        steps
    }
}

pub trait PathStepExt<D: Direction>: Clone {
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
pub struct MovementSearchMeta<D: Direction> {
    pub movement_type: MovementType,
    pub stealth: bool,
    pub illegal_next_dir: Option<D>, // units aren't allowed to turn around 180°
    pub remaining_movement: MovementPoints,
    pub path: Path<D>,
}
impl<D: Direction> MovementSearchMeta<D> {
    fn order(&self, other: &Self) -> Ordering {
        if self.movement_type == other.movement_type {
            let mut orderings = HashSet::new();
            orderings.insert(if self.stealth == other.stealth {
                Ordering::Equal
            } else if self.stealth {
                Ordering::Greater
            } else {
                Ordering::Less
            });
            orderings.insert(self.remaining_movement.cmp(&other.remaining_movement));
            orderings.insert(if self.illegal_next_dir.is_some() == other.illegal_next_dir.is_some() {
                Ordering::Equal
            } else if self.illegal_next_dir.is_some() {
                Ordering::Less
            } else {
                Ordering::Greater
            });
            if orderings.len() == 1 || orderings.len() == 2 && orderings.contains(&Ordering::Equal) {
                for ord in orderings {
                    if ord != Ordering::Equal {
                        return ord;
                    }
                }
            }
            Ordering::Equal
        } else {
            Ordering::Equal
        }
    }
}
impl<D: Direction> PartialOrd for MovementSearchMeta<D> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.order(other))
    }
}
impl<D: Direction> Ord for MovementSearchMeta<D> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.order(other)
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

pub fn movement_search<D, F>(game: &Game<D>, unit: &NormalUnit, path_so_far: &Path<D>, fog: Option<&HashSet<Point>>, mut callback: F)
where D: Direction, F: FnMut(&Path<D>, Point, bool) -> PathSearchFeedback {
    // if the unit can't move from it's position, no need to go further
    let (movement_type, remaining_movement) = match game.get_map().get_terrain(path_so_far.start) {
        Some(t) => {
            let (movement_type, remaining_movement) = unit.get_movement(t);
            if let Some(cost) = t.movement_cost(movement_type) {
                if cost > remaining_movement {
                    return
                } else {
                    (movement_type, remaining_movement)
                }
            } else {
                return
            }
        }
        None => return,
    };

    // start width-search
    let mut best_metas: HashMap<Point, HashSet<MovementSearchMeta<D>>> = HashMap::new();
    let mut next_checks = BinaryHeap::new();
    next_checks.push(MovementSearch {
        pos: path_so_far.start,
        meta: MovementSearchMeta {
            movement_type,
            remaining_movement,
            stealth: unit.has_stealth(),
            illegal_next_dir: None,
            path: Path::new(path_so_far.start),
        }
    });
    while let Some(MovementSearch{pos, meta}) = next_checks.pop() {
        let blocking_unit = game.get_map().get_unit(pos);
        if meta.path.steps.len() <= path_so_far.steps.len() && meta.path.steps[..] != path_so_far.steps[..meta.path.steps.len()] {
            // only follow path_so_far until its end, then the search can start
            continue;
        }
        if meta.path.steps.len() >= path_so_far.steps.len() {
            match callback(&meta.path, pos, blocking_unit == None || path_so_far.start == pos && blocking_unit.unwrap() == &unit.as_unit()) {
                PathSearchFeedback::Found => return,
                PathSearchFeedback::Rejected => continue,
                PathSearchFeedback::Continue => {}
            }
            if let Some(metas) = best_metas.get_mut(&pos) {
                // check if this pos already has a MovementSearchMeta that's superior in every way
                // if so, skip this MovementSearch, it can't be useful
                if metas.iter().any(|m| meta.order(m) == Ordering::Less) {
                    continue;
                }
                let mut set = HashSet::new();
                for m in metas.drain() {
                    if meta.order(&m) != Ordering::Greater {
                        set.insert(m);
                    }
                }
                *metas = set;
                metas.insert(meta.clone());
            } else {
                let mut set = HashSet::new();
                set.insert(meta.clone());
                best_metas.insert(pos, set);
            }
        }
        let prev_terrain = game.get_map().get_terrain(pos).unwrap();
        for (neighbor, step) in game.get_map().get_unit_movement_neighbors(pos, meta.movement_type) {
            if step.dir().is_some() && step.dir() == meta.illegal_next_dir {
                // don't turn around 180°
                continue;
            }
            let hidden_by_fog = fog.and_then(|fog| Some(fog.contains(&neighbor.point))).unwrap_or(false);
            if !hidden_by_fog {
                match game.get_map().get_unit(neighbor.point) {
                    Some(other) => {
                        if !other.can_be_moved_through(unit, game) {
                            continue;
                        }
                    }
                    _ => {}
                }
            }
            if let Some(mut meta) = game.get_map().get_terrain(neighbor.point).and_then(|t| t.update_movement(&meta, prev_terrain)) {
                // todo: check if maybe the PathStep is disallowed by some Detail at neighbor.point
                if meta.path.steps.push(step.clone()).is_ok() {
                    meta.illegal_next_dir = None;
                    if step.dir().is_some() {
                        meta.illegal_next_dir = Some(neighbor.direction.opposite_direction());
                    }
                    next_checks.push(MovementSearch {
                        pos: neighbor.point,
                        meta,
                    });
                }
            }
        }
    }
}

