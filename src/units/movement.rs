use std::collections::{BinaryHeap, HashSet, HashMap};
use std::cmp::Ordering;
use std::fmt::Debug;
use std::hash::Hash;

use num_rational::Rational32;
use serde::Deserialize;
use zipper::*;
use zipper_derive::*;

use crate::config::movement_type_config::MovementPattern;
use crate::game::commands::CommandError;
use crate::map::direction::Direction;
use crate::map::point::Point;
use crate::game::game::Game;
use crate::game::fog::FogIntensity;
use crate::map::map::*;
use crate::map::wrapping_map::OrientedPoint;
use crate::terrain::{AmphibiousTyping, ExtraMovementOptions};
use crate::terrain::terrain::Terrain;

use super::attributes::Amphibious;
use super::unit::Unit;

#[derive(Debug, PartialEq, Eq)]
pub enum PathSearchFeedback {
    Continue,
    ContinueWithoutStopping,
    Rejected,
    Found,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
pub enum MovementType {
    Foot,
    Bike,
    Wheel,
    Treads,

    Hovercraft,
    Boat,
    Ship,

    Heli,
    Plane,

    Chess,
}

/*#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
}*/


#[derive(Debug, Clone, Copy, PartialEq, Eq, Zippable, Hash)]
#[zippable(bits = 3)]
pub enum PathStep<D: Direction> {
    Dir(D),
    Jump(D), // jumps 2 fields, caused by Fountains
    Diagonal(D), // moves diagonally, for chess units
    Knight(D, bool),
    //Point(Point),
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
                if let Some(o) = get_diagonal_neighbor(map, pos, *d) {
                    Ok((o.clone(), Self::Diagonal(o.direction.opposite_direction())))
                } else {
                    Err(CommandError::InvalidPath)
                }
            }
            Self::Knight(d, turn_left) => {
                if let Some(o) = get_knight_neighbor(map, pos, *d, *turn_left) {
                    Ok((o.clone(), Self::Knight(o.direction.opposite_direction(), *turn_left != o.mirrored)))
                } else {
                    Err(CommandError::InvalidPath)
                }
            }
            //Self::Point(p) => Ok((OrientedPoint::new(*p, false, D::list()[0]), Self::Point(pos))),
        }
    }

    pub fn dir(&self) -> Option<D> {
        match self {
            Self::Dir(d) => Some(*d),
            Self::Jump(d) => Some(*d),
            Self::Diagonal(_) => None,
            Self::Knight(_, _) => None,
            //Self::Point(_) => None,
        }
    }
}

pub const MAX_PATH_LENGTH: u32 = 200;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Path<D: Direction> {
    pub start: Point,
    pub steps: Vec<PathStep::<D>>,
}
impl<D: Direction> Path<D> {
    pub fn new(start: Point) -> Self {
        Self {
            start,
            steps: Vec::new(),
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
}

// rotated slightly counter-clockwise compared to dir
pub fn get_diagonal_neighbor<D: Direction>(map: &Map<D>, p: Point, dir: D) -> Option<OrientedPoint<D>> {
    if let Some(dp1) = map.wrapping_logic().get_neighbor(p, dir) {
        if let Some(dp2) = map.wrapping_logic().get_neighbor(dp1.point, dp1.direction.rotate(dp1.mirrored)) {
            return Some(OrientedPoint::new(dp2.point, dp1.mirrored != dp2.mirrored, dp2.direction.rotate(dp1.mirrored == dp2.mirrored)));
        }
    }
    if let Some(dp1) = map.wrapping_logic().get_neighbor(p, dir.rotate(false)) {
        if let Some(dp2) = map.wrapping_logic().get_neighbor(dp1.point, dp1.direction.rotate(!dp1.mirrored)) {
            return Some(OrientedPoint::new(dp2.point, dp1.mirrored != dp2.mirrored, dp2.direction));
        }
    }
    None
}

pub fn get_knight_neighbor<D: Direction>(map: &Map<D>, p: Point, dir: D, turn_left: bool) -> Option<OrientedPoint<D>> {
    if turn_left {
        if let Some(dp1) = map.wrapping_logic().get_neighbor(p, dir) {
            if let Some(dp2) = get_diagonal_neighbor(map, dp1.point, dp1.direction) {
                return Some(OrientedPoint::new(dp2.point, dp1.mirrored != dp2.mirrored, dp2.direction));
            }
        }
        if let Some(dp1) = get_diagonal_neighbor(map, p, dir) {
            if let Some(dp2) = map.wrapping_logic().get_neighbor(dp1.point, dp1.direction) {
                return Some(OrientedPoint::new(dp2.point, dp1.mirrored != dp2.mirrored, dp2.direction));
            }
        }
    } else {
        if let Some(dp1) = map.wrapping_logic().get_neighbor(p, dir) {
            if let Some(dp2) = get_diagonal_neighbor(map, dp1.point, dp1.direction.rotate(!dp1.mirrored)) {
                return Some(OrientedPoint::new(dp2.point, dp1.mirrored != dp2.mirrored, dp2.direction.rotate(dp1.mirrored != dp2.mirrored)));
            }
        }
        if let Some(dp1) = get_diagonal_neighbor(map, p, dir.rotate(true)) {
            if let Some(dp2) = map.wrapping_logic().get_neighbor(dp1.point, dp1.direction.rotate(dp1.mirrored)) {
                return Some(OrientedPoint::new(dp2.point, dp1.mirrored != dp2.mirrored, dp2.direction));
            }
        }
    };
    None
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum TBallast<D: Direction> {
    MovementPoints(Rational32),
    Direction(Option<D>),
    QueenDirection(Option<(D, bool)>),
    ForbiddenDirection(Option<D>),
}

impl<D: Direction> TBallast<D> {
    fn heap_order(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Self::MovementPoints(m1), Self::MovementPoints(m2)) => m1.cmp(m2),
            (Self::Direction(_d1), Self::Direction(_d2)) => {
                Ordering::Equal
                // TODO: wouldn't the following be more correct?
                // it probably doesn't matter since the direction is non-null after the first step
                /*if d1 == d2 || d1.is_some() && d2.is_some() {
                    Ordering::Equal
                } else if d1.is_none() {
                    Ordering::Greater
                } else {
                    Ordering::Less
                }*/
            },
            (Self::QueenDirection(_), Self::QueenDirection(_)) => {
                Ordering::Equal
            },
            (Self::ForbiddenDirection(_d1), Self::ForbiddenDirection(_d2)) => {
                Ordering::Equal
                // TODO: wouldn't the following be more correct?
                // it probably doesn't matter since the direction is non-null after the first step
                /*if d1 == d2 || d1.is_some() && d2.is_some() {
                    Ordering::Equal
                } else if d1.is_none() {
                    Ordering::Greater
                } else {
                    Ordering::Less
                }*/
            },
            _ => panic!("TemporaryBallast have incompatible types: {self:?} - {other:?}")
        }
    }

    fn useful_with<'a>(&self, mut others: impl Iterator<Item = &'a Self>, map: &Map<D>, point: Point) -> bool {
        match self {
            Self::MovementPoints(mp) => {
                others.all(|other| match other {
                    Self::MovementPoints(other) => mp < other,
                    _ => panic!("TemporaryBallast have incompatible types: {self:?} - {other:?}")
                })
            }
            Self::Direction(dir) => {
                others.all(|other| match other {
                    Self::Direction(other) => other.is_some() && dir != other,
                    _ => panic!("TemporaryBallast have incompatible types: {self:?} - {other:?}")
                })
            }
            Self::QueenDirection(dir) => {
                others.all(|other| match other {
                    Self::QueenDirection(other) => other.is_some() && dir != other,
                    _ => panic!("TemporaryBallast have incompatible types: {self:?} - {other:?}")
                })
            }
            Self::ForbiddenDirection(dir) => {
                let mut found: Option<D> = None;
                for other in others {
                    match (dir, other) {
                        (_, Self::ForbiddenDirection(None)) => return false,
                        (None, Self::ForbiddenDirection(Some(_))) => (),
                        (Some(blocked), Self::ForbiddenDirection(Some(other))) => {
                            if blocked == other {
                                return false;
                            }
                            if found.is_some() && found != Some(*other) {
                                // found 2 other steps that reach here
                                // a third isn't needed
                                return false
                            } else {
                                found = Some(*other);
                            }
                        }
                        _ => panic!("TemporaryBallast have incompatible types: {self:?} - {other:?}")
                    }
                }
                true
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct TemporaryBallast<D: Direction> {
    entries: Vec<TBallast<D>>,
}

impl<D: Direction> TemporaryBallast<D> {
    fn new(entries: Vec<TBallast<D>>) -> Self {
        Self {
            entries,
        }
    }
    fn heap_order(&self, other: &Self) -> Ordering {
        if self.entries.len() != other.entries.len() {
            panic!("TemporaryBallast have different list sizes: {:?} - {:?}", self.entries, other.entries);
        }
        let mut order = Ordering::Equal;
        for (p1, p2) in self.entries.iter().zip(other.entries.iter()) {
            let o = p1.heap_order(p2);
            if order == Ordering::Equal {
                order = o;
            } else if o != Ordering::Equal && order != o {
                return Ordering::Equal;
            }
        }
        order
    }

    fn useful_with<'a>(&self, others: impl Iterator<Item = &'a Self>, map: &Map<D>, point: Point) -> bool {
        let mut sub_others = Vec::new();
        for _ in &self.entries {
            sub_others.push(Vec::new());
        }
        for other in others {
            if self.entries.len() != other.entries.len() {
                panic!("TemporaryBallast have different list sizes: {:?} - {:?}", self.entries, other.entries);
            }
            for (i, t) in other.entries.iter().enumerate() {
                sub_others[i].push(t);
            }
        }
        // assumes that no zero-length lists are created (would be Self::None instead)
        if sub_others[0].len() == 0 {
            return true;
        }
        for (el, others) in self.entries.iter().zip(sub_others.into_iter()) {
            if el.useful_with(others.into_iter(), map, point) {
                return true;
            }
        }
        false
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct PermanentBallast<D: Direction> {
    entries: Vec<PbEntry<D>>,
}

impl<D: Direction> PermanentBallast<D> {
    fn new(entries: Vec<PbEntry<D>>) -> Self {
        Self {
            entries,
        }
    }

    fn worse_or_equal(&self, other: &Self, map: &Map<D>, point: Point) -> bool {
        /*if self.unit_type != other.unit_type {
            return false;
        }*/
        if self.entries.len() != other.entries.len() {
            panic!("PermanentBallast have different list sizes: {:?} - {:?}", self.entries, other.entries);
        }
        for (p1, p2) in self.entries.iter().zip(other.entries.iter()) {
            if !p1.worse_or_equal(p2, map, point) {
                return false;
            }
        }
        true
    }

    fn movement_cost(&self, terrain: &Terrain, unit: &Unit<D>) -> Option<Rational32> {
        let mut amphibious = AmphibiousTyping::Land;
        for e in &self.entries {
            if let PbEntry::Amphibious(a) = e {
                amphibious = *a;
            }
        }
        match amphibious {
            AmphibiousTyping::Land => {
                terrain.movement_cost(unit.movement_type(Amphibious::OnLand))
            }
            AmphibiousTyping::Sea => {
                terrain.movement_cost(unit.movement_type(Amphibious::InWater))
            }
            AmphibiousTyping::Beach => {
                match (terrain.movement_cost(unit.movement_type(Amphibious::OnLand)), terrain.movement_cost(unit.movement_type(Amphibious::InWater))) {
                    (Some(c1), Some(c2)) => Some(c1.min(c2)),
                    (None, None) => None,
                    (c, None) => c,
                    (None, c) => c,
                }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum PbEntry<D: Direction> {
    PawnDirection(D),
    Amphibious(AmphibiousTyping),
}

impl<D: Direction> PbEntry<D> {
    fn worse_or_equal(&self, other: &Self, map: &Map<D>, point: Point) -> bool {
        match (self, other) {
            (Self::PawnDirection(d1), Self::PawnDirection(d2)) => {
                if map.get_terrain(point).unwrap().is_chess() {
                    d1 == d2
                } else {
                    // direction doesn't matter here, so counts as equal
                    true
                }
            }
            (Self::Amphibious(m1), Self::Amphibious(m2)) => {
                *m1 == *m2
            }
            _ => panic!("PbEntry have incompatible types: {self:?} - {other:?}")
        }
    }
}

/*impl<D: Direction> TemporaryBallast<D> for () {
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
}*/

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct MovementSearchMeta<D: Direction> {
    previous_turns: Vec<Path<D>>,
    path: Path<D>,
    permanent: PermanentBallast<D>,
    temporary: TemporaryBallast<D>,
}
impl<D: Direction> MovementSearchMeta<D> {
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
        let mut similar = Vec::new();
        // search for items that are at least as good as this one before considering temporary ballast
        for other in others.filter(|other| {
            self.previous_turns.len() >= other.previous_turns.len()
            && self.permanent.worse_or_equal(&other.permanent, map, point)
            && self <= other
        }) {
            if self.previous_turns.len() > other.previous_turns.len() {
                // found something that's better than self
                return false;
            }
            similar.push(&other.temporary);
        }
        if similar.len() == 0 {
            true
        } else {
            self.temporary.useful_with(similar.into_iter(), map, point)
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

// units can have two types of extra ballast
// - changes to themselves (stay after end-turn)
//      pawn direction or hover_mode for Hoverbikes
// - previous steps of a path excluding some next steps (reset after end-turn)
//      chess units moving only straight
//      normal units unable to turn around 180°
//      movement_points for most units
// 
// changes to themself s

fn movement_search_core<D, CanStartFrom, BaseMovement, FindSteps, DoStep, CALLBACK>(
    map: &Map<D>,
    start: Point,
    starting_ballast: PermanentBallast<D>,
    additional_turns: usize,
    can_start_from: CanStartFrom,
    base_movement: BaseMovement,
    find_steps: FindSteps,
    do_step: DoStep,
    mut callback: CALLBACK
) -> Option<Path<D>>
where
    D: Direction,
    CanStartFrom: Fn(&Terrain, &PermanentBallast<D>) -> bool,
    BaseMovement: Fn(Point, &PermanentBallast<D>, usize) -> TemporaryBallast<D>,
    FindSteps: Fn(Point, bool, &PermanentBallast<D>, &TemporaryBallast<D>) -> Vec<PathStep<D>>,
    DoStep: Fn(Point, PathStep<D>, &PermanentBallast<D>, &TemporaryBallast<D>) -> Option<(Point, PermanentBallast<D>, TemporaryBallast<D>)>,
    CALLBACK: FnMut(&[Path<D>], &Path<D>, Point) -> PathSearchFeedback,
{
    let start_terrain = map.get_terrain(start).expect(&format!("Map doesn't have terrain at {:?}", start));
    if !can_start_from(&start_terrain, &starting_ballast) {
        let path = Path::new(start);
        return match callback(&[], &path, start) {
            PathSearchFeedback::Found => Some(path),
            _ => None,
        };
    }
    let temporary = base_movement(start, &starting_ballast, 0);
    // some ways that arrive at the same point may be incomparable
    // (better in one way, worse in another)
    // so for each point, a HashSet is used to store the best paths
    let mut best_metas: HashMap<Point, Vec<MovementSearchMeta<D>>> = HashMap::new();
    let mut next_checks: BinaryHeap<MovementSearch<D>> = BinaryHeap::with_capacity(map.all_points().len());
    next_checks.push(MovementSearch {
        pos: start,
        meta: MovementSearchMeta {
            previous_turns: Vec::new(),
            path: Path::new(start),
            permanent: starting_ballast,
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
        if can_stop && meta.previous_turns.len() < additional_turns && can_start_from(&map.get_terrain(pos).unwrap(), &meta.permanent) {
            let permanent = meta.permanent.clone();
            let temporary = base_movement(pos, &permanent, meta.previous_turns.len() + 1);
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

fn movement_search_map<D: Direction, Callback, TransformMovementCost>(
    map: &Map<D>,
    unit: &Unit<D>,
    start: Point,
    rounds: usize,
    callback: Callback,
    transform_movement_cost: TransformMovementCost,
)
where
   Callback: FnMut(&[Path<D>], &Path<D>, Point) -> PathSearchFeedback,
   TransformMovementCost: Fn(Rational32, &Unit<D>) -> Rational32,
{
    if rounds == 0 {
        return;
    }
    let transporter = map.get_unit(start)
        .filter(|u| unit.get_owner_id() == u.get_owner_id());
    let first_permanent = {
        let terrain = map.get_terrain(start).unwrap();
        let mut permanents = Vec::new();
        match unit.movement_pattern() {
            MovementPattern::Pawn => {
                permanents.push(PbEntry::PawnDirection(unit.get_direction()));
            }
            MovementPattern::None => return,
            _ => ()
        }
        if unit.is_amphibious() {
            permanents.push(PbEntry::Amphibious(match (terrain.get_amphibious(), unit.get_amphibious()) {
                (None, Amphibious::InWater) => AmphibiousTyping::Sea,
                (None, Amphibious::OnLand) => AmphibiousTyping::Land,
                (Some(AmphibiousTyping::Land), Amphibious::InWater) => AmphibiousTyping::Sea,
                (Some(AmphibiousTyping::Sea), Amphibious::OnLand) => AmphibiousTyping::Land,
                (Some(a), _) => a,
            }));
        }
        PermanentBallast::new(permanents)
    };
    let base_movement = |pos: Point, permanent: &PermanentBallast<D>, round: usize| {
        let terrain = map.get_terrain(pos).unwrap();
        let mut temps = Vec::new();
        // TODO: add hero aura bonuses to mp
        // TODO: if round == 0 add bonus movement from transporter
        // TODO: add movement bonus based on terrain (so far only affects pawns)
        let mut mp = unit.movement_points();
        match unit.movement_pattern() {
            MovementPattern::Standard |
            MovementPattern::StandardLoopLess => temps.push(TBallast::ForbiddenDirection(None)),
            MovementPattern::Diagonal |
            MovementPattern::Straight => temps.push(TBallast::Direction(None)),
            MovementPattern::Pawn => {
                let mut dir = None;
                if terrain.is_chess() {
                    for t in &permanent.entries {
                        if let PbEntry::PawnDirection(d) = t {
                            dir = Some(*d);
                            break;
                        }
                    }
                    if dir.is_none() {
                        panic!("Pawn Permanent missing PawnDirection: {permanent:?}");
                    }
                };
                temps.push(TBallast::Direction(dir));
                if terrain.extra_step_options() == ExtraMovementOptions::PawnStart {
                    mp += Rational32::from_integer(1);
                }
            }
            MovementPattern::None => (),
            MovementPattern::Knight => {
                // could add QueenDirection in the future
            }
            MovementPattern::Rays => temps.push(TBallast::QueenDirection(None)),
        }
        temps.push(TBallast::MovementPoints(mp));
        TemporaryBallast::new(temps)
    };
    movement_search_core(
        map,
        start,
        first_permanent,
        rounds - 1,
        |terrain, permanent| {
            permanent.movement_cost(terrain, unit).is_some()
        },
        base_movement,
        |point, a, b, temporary_ballast| {
            unit.movement_pattern().find_steps(map, point)
            .into_iter()
            .filter(|step| {
                temporary_ballast.entries.iter().all(|temp| {
                    match (temp, step) {
                        (TBallast::ForbiddenDirection(Some(d1)), PathStep::Dir(d2)) => d1 != d2,
                        (TBallast::Direction(Some(d1)), PathStep::Dir(d2)) => d1 == d2,
                        (TBallast::Direction(Some(d1)), PathStep::Diagonal(d2)) => d1 == d2,
                        (TBallast::QueenDirection(Some((d1, true))), PathStep::Dir(d2)) => d1 == d2,
                        (TBallast::QueenDirection(Some((d1, false))), PathStep::Diagonal(d2)) => d1 == d2,
                        _ => true
                    }
                })
            }).collect()
        },
        |point, step, permanent_ballast, temporary_ballast| {
            if let Ok((dp, _)) = step.progress_reversible(map, point) {
                let terrain = map.get_terrain(dp.point).unwrap();
                if let Some(cost) = permanent_ballast.movement_cost(terrain, unit) {
                    let cost = transform_movement_cost(cost, unit);
                    // TODO: preventing beach <-> bridge only needs the prev Terrain
                    // if the current one is either bridge or beach
                    let mut permanent = Vec::new();
                    for p in &permanent_ballast.entries {
                        permanent.push(match p {
                            PbEntry::Amphibious(amph) => {
                                PbEntry::Amphibious(terrain.get_amphibious().unwrap_or(*amph))
                            }
                            PbEntry::PawnDirection(dir) => {
                                let mut direction = dp.direction;
                                if let PathStep::Diagonal(d) = step {
                                    if d != *dir {
                                        direction = direction.rotate(dp.mirrored);
                                    }
                                }
                                PbEntry::PawnDirection(direction)
                            }
                        });
                    }
                    let mut temporary = Vec::new();
                    for p in temporary_ballast.entries.iter() {
                        temporary.push(match p {
                            TBallast::Direction(_) => {
                                TBallast::Direction(Some(dp.direction))
                            }
                            TBallast::ForbiddenDirection(_) => {
                                TBallast::Direction(Some(dp.direction.opposite_direction()))
                            }
                            TBallast::QueenDirection(_) => {
                                let rook_like = match step {
                                    PathStep::Dir(_) => true,
                                    _ => false,
                                };
                                TBallast::QueenDirection(Some((dp.direction, rook_like)))
                            }
                            TBallast::MovementPoints(mp) => {
                                if cost > *mp {
                                    return None;
                                }
                                TBallast::MovementPoints(*mp - cost)
                            }
                        });
                    }
                    return Some((dp.point, PermanentBallast::new(permanent), TemporaryBallast::new(temporary)));
                }
            }
            None
        },
        callback,
    );
    /*match unit {
        UnitType::Unknown |
        UnitType::Structure(_) => return,
        UnitType::Normal(unit) if unit.changes_movement_type() => {
            let (starting_movement_type, starting_movement_points) = unit.get_movement(map.get_terrain(start).expect(&format!("Map doesn't have terrain at {:?}", start)), transporter);
            let movement_points = unit.get_movement(map.get_terrain(start).expect(&format!("Map doesn't have terrain at {:?}", start)), None).1;
            let base_movement = |_terrain: &Terrain, permanent: Option<&MovementType>, round: usize| {
                let mp = if round == 0 {
                    starting_movement_points
                } else {
                    movement_points
                };
                (permanent.cloned().unwrap_or(starting_movement_type), NormalBallast {
                    points: transform_movement_points(mp, round),
                    forbidden_dir: None
                })
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
            let (movement_type, starting_movement_points) = unit.get_movement(map.get_terrain(start).expect(&format!("Map doesn't have terrain at {:?}", start)), transporter);
            let movement_points = unit.get_movement(map.get_terrain(start).expect(&format!("Map doesn't have terrain at {:?}", start)), None).1;
            let base_movement = |_terrain: &Terrain, _permanent: Option<&()>, round: usize| {
                let mp = if round == 0 {
                    starting_movement_points
                } else {
                    movement_points
                };
                ((), NormalBallast {
                    points: transform_movement_points(mp, round),
                    forbidden_dir: None
                })
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
                Box::new(|terrain: &Terrain, permanent: Option<&PawnPermanent<D>>, _| {
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
    };*/
}

pub fn movement_search_game<D: Direction, U, F>(game: &Game<D>, unit: &Unit<D>, path_so_far: &Path<D>, rounds: usize, get_unit: U, mut callback: F)
where
U: Fn(Point) -> Option<Unit<D>>,
F: FnMut(&[Path<D>], &Path<D>, Point, bool, bool) -> PathSearchFeedback {
    //let commander = unit.get_commander(game);
    movement_search_map(
        game.get_map(),
        unit,
        path_so_far.start,
        rounds,
        |previous_turns: &[Path<D>], path: &Path<D>, destination| {
            if previous_turns.len() == 0 && path.steps.len() <= path_so_far.steps.len() && path.steps[..] != path_so_far.steps[..path.steps.len()] {
                return PathSearchFeedback::Rejected;
            }
            let mut can_stop_here = true;
            let mut can_continue = true;
            if let Some(blocking_unit) = get_unit(destination) {
                can_stop_here = false;
                let is_self = path_so_far.start == destination && blocking_unit == *unit;
                if !is_self {
                    let mut reject = true;
                    // friendly unit that can simply be moved past
                    if unit.get_team() == blocking_unit.get_team() && blocking_unit.can_be_moved_through() {
                        reject = false;
                    }
                    // stealth
                    if blocking_unit.can_be_moved_through() && unit.has_stealth_movement(game) {
                        reject = false;
                    }
                    // chess take
                    if unit.get_team() != blocking_unit.get_team() && unit.can_take() && blocking_unit.can_be_taken() {
                        if unit.movement_pattern() == MovementPattern::Pawn {
                            if let Some(PathStep::Dir(_)) = path.steps.last() {
                                return PathSearchFeedback::Rejected;
                            }
                        }
                        can_continue = false;
                        can_stop_here = true;
                        reject = false;
                    }
                    if reject {
                        return PathSearchFeedback::Rejected;
                    }
                    /*match unit {
                        UnitType::Normal(unit) => {
                            if !blocking_unit.can_be_moved_through(unit, game) {
                                return PathSearchFeedback::Rejected;
                            }
                        }
                        UnitType::Chess(ChessUnit { typ: ChessUnits::Pawn(_, _), owner, .. }) => {
                            if let Some(PathStep::Dir(_)) = path.steps.last() {
                                return PathSearchFeedback::Rejected;
                            }
                            if !blocking_unit.killable_by_chess(game.get_team(Some(*owner)), game) {
                                return PathSearchFeedback::Rejected;
                            }
                            can_continue = false;
                        }
                        UnitType::Chess(unit) => {
                            if !blocking_unit.killable_by_chess(game.get_team(Some(unit.owner)), game) {
                                return PathSearchFeedback::Rejected;
                            }
                            can_continue = false;
                        }
                        _ => (),
                    }*/
                }
            } else if unit.movement_pattern() == MovementPattern::Pawn {
                if let Some(PathStep::Diagonal(_)) = path.steps.last() {
                    // en passant
                    if previous_turns.len() > 0 || !game.get_map().all_points()
                    .into_iter()
                    .any(|p| get_unit(p).filter(|u| u.get_en_passant() == Some(destination)).is_some()) {
                        return PathSearchFeedback::Rejected;
                    }
                }
            }
            callback(previous_turns, path, destination, can_continue, can_stop_here)
        },
        |cost, _unit| {
            // TODO
            //commander.transform_movement_cost(unit, cost)
            cost
        }
    )
}

fn movement_search_map_without_game<D: Direction, F>(map: &Map<D>, unit: &Unit<D>, start: Point, rounds: usize, callback: F)
where F: FnMut(&[Path<D>], &Path<D>, Point) -> PathSearchFeedback {
    movement_search_map(
        map,
        unit,
        start,
        rounds,
        callback,
        |cost, _| cost,
    )
}

pub fn movement_area_map<D: Direction>(map: &Map<D>, unit: &Unit<D>, path_so_far: &Path<D>, rounds: usize) -> HashMap<Point, usize> {
    let mut result = HashMap::new();
    let callback = |previous_turns: &[Path<D>], path: &Path<D>, point| {
        if previous_turns.len() == 0 && path.steps.len() <= path_so_far.steps.len() && path.steps[..] != path_so_far.steps[..path.steps.len()] {
            return PathSearchFeedback::Rejected;
        }
        // movement_area_map ignores units
        if unit.movement_pattern() == MovementPattern::Pawn {
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

pub fn movement_area_game<D: Direction>(game: &Game<D>, unit: &Unit<D>, path_so_far: &Path<D>, rounds: usize) -> HashMap<Point, usize> {
    let mut result = HashMap::new();
    movement_search_game(game, unit, path_so_far, rounds,
        |p| game.get_map().get_unit(p).cloned(),
        |previous_turns: &[Path<D>], path: &Path<D>, destination, can_continue, can_stop_here| {
        if !result.contains_key(&destination) {
            result.insert(destination, previous_turns.len());
        }
        if can_continue {
            if can_stop_here {
                PathSearchFeedback::Continue
            } else {
                PathSearchFeedback::ContinueWithoutStopping
            }
        } else {
            PathSearchFeedback::Rejected
        }
    });
    result
}

pub fn search_path<D: Direction, F>(game: &Game<D>, unit: &Unit<D>, path_so_far: &Path<D>, fog: Option<&HashMap<Point, FogIntensity>>, callback: F) -> Option<Path<D>>
where F: Fn(&Path<D>, Point, bool) -> PathSearchFeedback {
    let mut result = None;
    movement_search_game(game, unit, path_so_far, 1,
        |p| {
            game.get_map().get_unit(p)
            .and_then(|u| u.fog_replacement(game.get_map().get_terrain(p).unwrap(), fog.and_then(|fog| fog.get(&p)).cloned().unwrap_or(FogIntensity::TrueSight)))
        },
        |_, path, destination, can_continue, can_stop_here| {
        if path.steps.len() < path_so_far.steps.len() {
            return PathSearchFeedback::Continue;
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

