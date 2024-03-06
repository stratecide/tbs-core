use std::collections::{BinaryHeap, HashSet, HashMap};
use std::cmp::Ordering;
use std::fmt::Debug;
use std::hash::Hash;

use num_rational::Rational32;
use zipper::*;
use zipper_derive::*;

use crate::config::movement_type_config::MovementPattern;
use crate::game::commands::CommandError;
use crate::map::direction::Direction;
use crate::map::point::Point;
use crate::game::game::Game;
use crate::game::fog::FogIntensity;
use crate::map::map::*;
use crate::map::wrapping_map::Distortion;
use crate::terrain::AmphibiousTyping;
use crate::terrain::terrain::Terrain;

use super::attributes::{Amphibious, AttributeKey};
use super::hero::Hero;
use super::unit::Unit;

#[derive(Debug, PartialEq, Eq)]
pub enum PathSearchFeedback {
    Continue,
    ContinueWithoutStopping,
    Rejected,
    Found,
}

crate::listable_enum! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
        Chess2,
        None,
    }
}

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
    pub fn progress(&self, map: &Map<D>, pos: Point) -> Result<(Point, Distortion<D>), CommandError> {
        match *self {
            Self::Dir(d) => {
                map.get_neighbor(pos, d)
                .ok_or(CommandError::InvalidPath)
            }
            Self::Jump(d) => {
                map.get_neighbor(pos, d).and_then(|(pos, distortion)| {
                    map.get_neighbor(pos, distortion.update_direction(d))
                    .map(|(pos, disto)| (pos, distortion + disto))
                }).ok_or(CommandError::InvalidPath)
            }
            Self::Diagonal(d) => {
                get_diagonal_neighbor(map, pos, d)
                .ok_or(CommandError::InvalidPath)
            }
            Self::Knight(d, turn_left) => {
                get_knight_neighbor(map, pos, d, turn_left)
                .ok_or(CommandError::InvalidPath)
            }
            //Self::Point(p) => Ok((OrientedPoint::new(*p, false, D::list()[0]), Self::Point(pos))),
        }
    }

    pub fn dir(&self) -> Option<D> {
        match self {
            Self::Dir(d) => Some(*d),
            Self::Jump(d) => Some(*d),
            _ => None,
        }
    }

    pub fn diagonal_dir(&self) -> Option<D> {
        match self {
            Self::Diagonal(d) => Some(*d),
            _ => None,
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

    pub fn end(&self, map: &Map<D>) -> Result<(Point, Distortion<D>), CommandError> {
        let mut current = self.start;
        let mut distortion = Distortion::neutral();
        for step in &self.steps {
            let c = step.progress(map, current)?;
            current = c.0;
            distortion += c.1;
        }
        Ok((current, distortion))
    }
    
    pub fn points(&self, map: &Map<D>) -> Result<Vec<Point>, CommandError> {
        let mut points = vec![self.start];
        let mut current = self.start;
        for step in self.steps.iter() {
            current = step.progress(map, current)?.0;
            points.push(current);
        }
        Ok(points)
    }

    /*pub fn end_ballast(&self, map: &Map<D>) -> Result<TemporaryBallast<D>, CommandError> {
        let mut current = self.start;
        let mut distortion = Distortion::neutral();
        for step in &self.steps {
            let c = step.progress(map, current)?;
            current = c.0;
            distortion += c.1;
        }
        Ok((current, distortion))
    }*/
}

// rotated slightly counter-clockwise compared to dir
pub fn get_diagonal_neighbor<D: Direction>(map: &Map<D>, p: Point, dir: D) -> Option<(Point, Distortion<D>)> {
    let map = map.wrapping_logic();
    let dir2 = dir.rotate(false);
    for (dir, dir2) in [(dir, dir2), (dir2, dir)] {
        if let Some((p1, distortion)) = map.get_neighbor(p, dir) {
            if let Some((p2, disto2)) = map.get_neighbor(p1, distortion.update_direction(dir2)) {
                return Some((p2, distortion + disto2));
            }
        }
    }
    None
}

// moves 2 fields in the given direction, then turns left or right and moves another field
pub fn get_knight_neighbor<D: Direction>(map: &Map<D>, p: Point, dir: D, turn_left: bool) -> Option<(Point, Distortion<D>)> {
    let map = map.wrapping_logic();
    let dir2 = dir.rotate(!turn_left);
    for (dir, dir2, dir3) in [(dir, dir, dir2), (dir, dir2, dir), (dir2, dir, dir)] {
        if let Some((p, distortion)) = map.get_neighbor(p, dir) {
            if let Some((p, disto)) = map.get_neighbor(p, distortion.update_direction(dir2)) {
                let distortion = distortion + disto;
                if let Some((p, disto)) = map.get_neighbor(p, distortion.update_direction(dir3)) {
                    return Some((p, distortion + disto));
                }
            }
        }
    }
    None
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PathStepTakes {
    Allow,
    Force,
    Deny,
}

impl PathStepTakes {
    // bigger number better
    fn value(&self) -> u8 {
        match self {
            Self::Allow => 1,
            _ => 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TBallast<D: Direction> {
    MovementPoints(Rational32),
    Direction(Option<D>),
    DiagonalDirection(Option<D>),
    ForbiddenDirection(Option<D>),
    Takes(PathStepTakes),
    //StepCount(usize),
}

impl<D: Direction> TBallast<D> {
    fn heap_order(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Self::MovementPoints(m1), Self::MovementPoints(m2)) => m1.cmp(m2),
            //(Self::StepCount(m1), Self::StepCount(m2)) => m1.cmp(m2),
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
            (Self::DiagonalDirection(_), Self::DiagonalDirection(_)) => {
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
            (Self::Takes(t1), Self::Takes(t2)) => {
                // this TBallast only matters when deciding whether to reject the step
                // comparison happens only if the step isn't rejected, so doesn't matter.
                //t1.value().cmp(&t2.value())
                Ordering::Equal
            }
            _ => panic!("TemporaryBallast have incompatible types: {self:?} - {other:?}")
        }
    }

    fn useful_with<'a>(&self, mut others: impl Iterator<Item = &'a Self>, _map: &Map<D>, _point: Point) -> bool {
        match self {
            Self::MovementPoints(mp) => {
                others.all(|other| match other {
                    Self::MovementPoints(other) => mp < other,
                    _ => panic!("TemporaryBallast have incompatible types: {self:?} - {other:?}")
                })
            }
            /*Self::StepCount(mp) => {
                others.all(|other| match other {
                    Self::StepCount(other) => mp < other,
                    _ => panic!("TemporaryBallast have incompatible types: {self:?} - {other:?}")
                })
            }*/
            Self::Direction(dir) => {
                others.all(|other| match other {
                    Self::Direction(other) => other.is_some() && dir != other,
                    _ => panic!("TemporaryBallast have incompatible types: {self:?} - {other:?}")
                })
            }
            Self::DiagonalDirection(dir) => {
                others.all(|other| match other {
                    Self::DiagonalDirection(other) => other.is_some() && dir != other,
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
            Self::Takes(_) => {
                if others.next().is_some() {
                    // this TBallast only matters when deciding whether to reject the step
                    // comparison happens only if the step isn't rejected, so doesn't matter.
                    return false;
                }
                true
            }
        }
    }
}

/**
 * TemporaryBallast influences what kinds of next steps are allowed
 * they may be constructed from unit data and permanent ballast, but can't write back
 * for a given path, the TemporaryBallast at its end can be returned
 */
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TemporaryBallast<D: Direction> {
    entries: Vec<TBallast<D>>,
}

impl<D: Direction> TemporaryBallast<D> {
    const EMPTY:Self = Self {
        entries: Vec::new(),
    };

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

    pub fn get_entries(&self) -> &[TBallast<D>] {
        &self.entries
    }

    /*fn update_on_step(&self, map: &Map<D>, point: Point, step: &PathStep<D>) -> Option<Self> {
        let distortion = match step.progress(map, point) {
            Ok((point, distortion)) => distortion,
            _ => return None
        };
        if let Some(cost) = permanent_ballast.movement_cost(terrain, unit) {
            let cost = transform_movement_cost(cost, unit);
        }
        let mut ballast = Vec::new();
        for p in self.entries.iter() {
            ballast.push(match p {
                TBallast::Direction(_) => {
                    TBallast::Direction(step.dir().map(|d| distortion.update_direction(d)))
                }
                TBallast::DiagonalDirection(_) => {
                    TBallast::DiagonalDirection(step.diagonal_dir().map(|d| distortion.update_diagonal_direction(d)))
                }
                TBallast::ForbiddenDirection(_) => {
                    TBallast::ForbiddenDirection(step.dir().map(|d| distortion.update_direction(d.opposite_direction())))
                }
                TBallast::MovementPoints(mp) => {
                    if cost > *mp {
                        return None;
                    }
                    TBallast::MovementPoints(*mp - cost)
                }
                TBallast::StepCount(step_count) => {
                    if *step_count == 0 {
                        return None;
                    }
                    TBallast::StepCount(*step_count - 1)
                }
            });
        }
        Some(Self::new(ballast))
    }*/
}

/**
 * PermanentBallast represents unit attributes change when the unit is moved
 */
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
pub enum PbEntry<D: Direction> {
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
    FindSteps: Fn(Point, usize, &PermanentBallast<D>, &TemporaryBallast<D>) -> Vec<PathStep<D>>,
    DoStep: Fn(Point, PathStep<D>, &PermanentBallast<D>, &TemporaryBallast<D>) -> Option<(Point, PermanentBallast<D>, TemporaryBallast<D>)>,
    CALLBACK: FnMut(&[Path<D>], &Path<D>, Point, &TemporaryBallast<D>) -> PathSearchFeedback,
{
    let start_terrain = map.get_terrain(start).expect(&format!("Map doesn't have terrain at {:?}", start));
    if !can_start_from(&start_terrain, &starting_ballast) {
        let path = Path::new(start);
        return match callback(&[], &path, start, &TemporaryBallast::EMPTY) {
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
        let can_stop = match callback(&meta.previous_turns, &meta.path, pos, &meta.temporary) {
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
        for step in find_steps(pos, meta.path.steps.len(), &meta.permanent, &meta.temporary) {
            let (next_point, permanent, temporary) = match do_step(pos, step, &meta.permanent, &meta.temporary) {
                None => continue,
                Some(data) => data,
            };
            steps_used.insert(step);
            let mut path = meta.path.clone();
            path.steps.push(step);
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
            for step in find_steps(pos, 0, &permanent, &temporary) {
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
    game: Option<&Game<D>>,
    map: &Map<D>,
    unit: &Unit<D>,
    start: Point,
    rounds: usize,
    get_unit: impl Fn(Point) -> Option<Unit<D>>,
    callback: Callback,
    transform_movement_cost: TransformMovementCost,
)
where
   Callback: FnMut(&[Path<D>], &Path<D>, Point, &TemporaryBallast<D>) -> PathSearchFeedback,
   TransformMovementCost: Fn(Rational32, &Unit<D>) -> Rational32,
{
    if rounds == 0 {
        return;
    }
    // TODO: would be cleaner to make transporter a parameter
    let transporter = map.get_unit(start)
        .filter(|u| unit.get_owner_id() == u.get_owner_id() && unit != *u);
    let movement_pattern = unit.movement_pattern();
    let first_permanent = {
        let terrain = map.get_terrain(start).unwrap();
        let mut permanents = Vec::new();
        match movement_pattern {
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
        let transporter = if round == 0 {
            transporter
        } else {
            None
        };
        // TODO: if rounds > 0 this hero influence will be calculated a lot
        // in that case it would be more efficient to use the aura_range map method from event_handler.rs
        let mut heroes = Hero::hero_influence_at(game, map, pos, unit.get_owner_id());
        if round > 0 {
            for (_, hero, _, _) in &mut heroes {
                // TODO: this isn't enough because a hero without active power might not have enough range to influence "unit" in the first place
                //hero.set_power_active(false);
            }
        }
        let heroes: Vec<_> = heroes.iter().collect();
        let mp = unit.movement_points(game, map, pos, transporter, &heroes);
        temps.push(TBallast::MovementPoints(mp));
        /*match movement_pattern {
            MovementPattern::Standard |
            MovementPattern::StandardLoopLess => temps.push(TBallast::ForbiddenDirection(None)),
            MovementPattern::Straight => temps.push(TBallast::Direction(None)),
            MovementPattern::Diagonal => temps.push(TBallast::DiagonalDirection(None)),
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
                let mut step_count = 1;
                if terrain.extra_step_options() == ExtraMovementOptions::PawnStart {
                    mp += Rational32::from_integer(1);
                    step_count += 1;
                }
                temps.push(TBallast::StepCount(step_count))
            }
            MovementPattern::None => (),
            MovementPattern::Knight => {
                temps.push(TBallast::StepCount(1))
            }
            MovementPattern::Rays => {
                temps.push(TBallast::Direction(None));
                temps.push(TBallast::DiagonalDirection(None));
            }
        }*/
        movement_pattern.add_temporary_ballast(terrain, &permanent.entries, &mut temps);
        // TODO: doesn't seem necessary for most units. there's probably a better way to do this...
        temps.push(TBallast::Takes(PathStepTakes::Allow));
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
        |point, step_id, permanent_ballast, temporary_ballast| {
            movement_pattern.find_steps(
                map,
                point,
                step_id,
                &permanent_ballast.entries,
                temporary_ballast.get_entries().get(1..).unwrap_or(&[]),
                map.get_terrain(point).unwrap().extra_step_options(),
                /*&get_unit,
                |p| {
                    if let Some(u) = get_unit(p) {
                        return unit.could_take(&u)
                    }
                    if unit.has_attribute(super::attributes::AttributeKey::EnPassant) {
                        for dp in map.all_points() {
                            if let Some(u) = get_unit(dp) {
                                if unit.could_take(&u) && u.get_en_passant() == Some(p) {
                                    return true;
                                }
                            }
                        }
                    }
                    false
                },*/
            )
        },
        |point, step, permanent_ballast, temporary_ballast| {
            if let Ok((point, distortion)) = step.progress(map, point) {
                let terrain = map.get_terrain(point).unwrap();
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
                                PbEntry::PawnDirection(distortion.update_direction(*dir))
                            }
                        });
                    }
                    let mut temporary = temporary_ballast.entries.clone();
                    match temporary.get_mut(0) {
                        Some(TBallast::MovementPoints(mp)) => {
                            if cost > *mp {
                                return None;
                            }
                            *mp -= cost;
                        }
                        _ => panic!("the first temporary ballast should be MovementPoints, was {:?}", temporary[0])
                    }
                    if let Some(temporary) = temporary.get_mut(1..).filter(|s| s.len() > 0) {
                        movement_pattern.update_temporary_ballast(&step, distortion, temporary);
                    }
                    return Some((point, PermanentBallast::new(permanent), TemporaryBallast::new(temporary)));
                }
            }
            None
        },
        callback,
    );
}

pub fn movement_search_game<D: Direction, U, F>(game: &Game<D>, unit: &Unit<D>, path_so_far: &Path<D>, rounds: usize, get_unit: U, mut callback: F)
where
U: Fn(Point) -> Option<Unit<D>>,
F: FnMut(&[Path<D>], &Path<D>, Point, bool, bool, &TemporaryBallast<D>) -> PathSearchFeedback {
    //let commander = unit.get_commander(game);
    movement_search_map(
        Some(game),
        game.get_map(),
        unit,
        path_so_far.start,
        rounds,
        &get_unit,
        |previous_turns: &[Path<D>], path: &Path<D>, destination, temporary_ballast| {
            if previous_turns.len() == 0 && path.steps.len() <= path_so_far.steps.len() && path.steps[..] != path_so_far.steps[..path.steps.len()] {
                return PathSearchFeedback::Rejected;
            }
            let mut takes = PathStepTakes::Allow;
            for ballast in temporary_ballast.get_entries() {
                match ballast {
                    TBallast::Takes(t) => takes = *t,
                    _ => (),
                }
            }
            let mut can_stop_here = true;
            let mut can_continue = true;
            if let Some(blocking_unit) = get_unit(destination) {
                let is_self = path_so_far.start == destination && blocking_unit == *unit;
                if !is_self {
                    can_stop_here = false;
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
                    if unit.could_take(&blocking_unit, takes) {
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
            } else if takes == PathStepTakes::Force {
                can_continue = false;
                let mut reject = true;
                if previous_turns.len() == 0 && unit.has_attribute(AttributeKey::EnPassant) {
                    for dp in game.get_map().all_points() {
                        if let Some(u) = game.get_map().get_unit(dp) {
                            if unit.could_take(&u, takes) && u.get_en_passant() == Some(destination) {
                                reject = false;
                                break;
                            }
                        }
                    }
                }
                if reject {
                    return PathSearchFeedback::Rejected;
                }
        }
            callback(previous_turns, path, destination, can_continue, can_stop_here, temporary_ballast)
        },
        |cost, _unit| {
            // TODO
            //commander.transform_movement_cost(unit, cost)
            cost
        }
    )
}

fn movement_search_map_without_game<D: Direction, F>(map: &Map<D>, unit: &Unit<D>, start: Point, rounds: usize, get_unit: impl Fn(Point) -> Option<Unit<D>>, callback: F)
where F: FnMut(&[Path<D>], &Path<D>, Point, &TemporaryBallast<D>) -> PathSearchFeedback {
    movement_search_map(
        None,
        map,
        unit,
        start,
        rounds,
        get_unit,
        callback,
        |cost, _| cost,
    )
}

pub fn movement_area_map<D: Direction>(map: &Map<D>, unit: &Unit<D>, path_so_far: &Path<D>, rounds: usize) -> HashMap<Point, usize> {
    let mut result = HashMap::new();
    // movement_area_map ignores units
    let get_unit = |_| None;
    let callback = |previous_turns: &[Path<D>], path: &Path<D>, point, _: &TemporaryBallast<D>| {
        if previous_turns.len() == 0 && path.steps.len() <= path_so_far.steps.len() && path.steps[..] != path_so_far.steps[..path.steps.len()] {
            return PathSearchFeedback::Rejected;
        }
        if !result.contains_key(&point) {
            result.insert(point, previous_turns.len());
        }
        PathSearchFeedback::Continue
    };
    movement_search_map_without_game(map, unit, path_so_far.start, rounds, get_unit, callback);
    result
}

pub fn movement_area_game<D: Direction>(game: &Game<D>, unit: &Unit<D>, path_so_far: &Path<D>, rounds: usize) -> HashMap<Point, usize> {
    let mut result = HashMap::new();
    movement_search_game(game, unit, path_so_far, rounds,
        |p| game.get_map().get_unit(p).cloned(),
        |previous_turns: &[Path<D>], path: &Path<D>, destination, can_continue, can_stop_here, _| {
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

pub fn search_path<D: Direction, F>(game: &Game<D>, unit: &Unit<D>, path_so_far: &Path<D>, fog: Option<&HashMap<Point, FogIntensity>>, callback: F) -> Option<(Path<D>, TemporaryBallast<D>)>
where F: Fn(&Path<D>, Point, bool, &TemporaryBallast<D>) -> PathSearchFeedback {
    let mut result = None;
    movement_search_game(game, unit, path_so_far, 1,
        |p| {
            game.get_map().get_unit(p)
            .and_then(|u| u.fog_replacement(game, p, fog.and_then(|fog| fog.get(&p)).cloned().unwrap_or(FogIntensity::TrueSight)))
        },
        |_, path, destination, can_continue, can_stop_here, temporary_ballast| {
        if path.steps.len() < path_so_far.steps.len() {
            return PathSearchFeedback::Continue;
        }
        let feedback = callback(path, destination, can_stop_here, temporary_ballast);
        if feedback == PathSearchFeedback::Found {
            result = Some((path.clone(), temporary_ballast.clone()));
        } else if feedback == PathSearchFeedback::Continue && !can_continue {
            return PathSearchFeedback::Rejected;
        } else if feedback == PathSearchFeedback::Continue && !can_stop_here {
            return PathSearchFeedback::ContinueWithoutStopping;
        }
        feedback
    });
    result
}

