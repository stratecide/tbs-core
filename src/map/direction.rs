use std::fmt;
use std::hash::Hash;
use crate::map::point::*;
use crate::units::attributes::{Attribute, AttributeError, AttributeKey, TrAttribute};

use zipper::*;
use zipper::zipper_derive::*;


pub trait Direction: 'static + Eq + Copy + Hash + fmt::Debug + Sync + Send + Zippable + fmt::Display + TrAttribute<Self> {
    type T: Translation<Self> + Clone + Copy + Hash + PartialEq + Eq + fmt::Debug + Sync + Send + SupportedZippable<u16>;
    //type P: PipeState<Self> + Clone + Copy + Hash + PartialEq + Eq + fmt::Debug + Sync + Send + Zippable;
    fn is_hex() -> bool;
    fn angle_0() -> Self;
    fn translation(&self, distance: i16) -> Self::T;
    //fn pipe_entry(&self) -> Self::P;
    fn list() -> Vec<Self>; // TODO: turn into &'static[Self]
    fn mirror_horizontally(&self) -> Self;
    //fn rotate_point_map(&self, map: &PointMap) -> PointMap;
    fn get_neighbor(&self, point: Point, odd_if_hex: bool) -> Option<Point> {
        let gp = self.translation(1).translate_point(&GlobalPoint::new(point.x() as i16, point.y() as i16), odd_if_hex);
        if gp.x() >= 0 && gp.x() <= 255 && gp.y() >= 0 && gp.y() <= 255 {
            Some(Point::new(gp.x() as u8, gp.y() as u8))
        } else {
            None
        }
    }
    fn rotate_around_center<P: Position<i16>>(&self, point: &P, center: &P, odd_if_hex: bool) -> P {
        let trans = Self::T::between(center, point, odd_if_hex);
        let trans = trans.rotate_by(*self);
        trans.translate_point(center, odd_if_hex)
    }
    fn list_index(&self) -> usize {
        let list = Self::list();
        list.iter().position(|d| self == d).expect("Unable to find Direction in list of all Directions")
    }
    fn rotate(&self, clockwise: bool) -> Self {
        let list = Self::list();
        let index = self.list_index();
        if clockwise {
            list[(index + list.len() - 1) % list.len()]
        } else {
            list[(index + 1) % list.len()]
        }
    }
    fn rotate_by(&self, other: Self) -> Self {
        let list = Self::list();
        let index = self.list_index();
        let index2 = other.list_index();
        list[(index + index2) % list.len()]
    }
    fn mirror_vertically(&self) -> Self {
        let list = Self::list();
        let index = self.list_index();
        list[(list.len() - index) % list.len()]
    }
    fn opposite_direction(&self) -> Self {
        let list = Self::list();
        let index = self.list_index();
        list[(index + list.len() / 2) % list.len()]
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash, Zippable)]
#[zippable(bits = 2)]
pub enum Direction4 {
    D0,
    D90,
    D180,
    D270,
}

impl TryFrom<Attribute<Direction4>> for Direction4 {
    type Error = AttributeError;
    fn try_from(value: Attribute<Direction4>) -> Result<Self, Self::Error> {
        if let Attribute::Direction(value) = value {
            Ok(value)
        } else {
            Err(AttributeError { requested: AttributeKey::Direction, received: Some(value.key()) })
        }
    }
}

impl From<Direction4> for Attribute<Direction4> {
    fn from(value: Direction4) -> Self {
        Attribute::Direction(value)
    }
}

impl Direction for Direction4 {
    type T = Translation4;
    //type P = PipeState4;
    fn is_hex() -> bool {
        false
    }
    fn angle_0() -> Self {
        Self::D0
    }
    fn translation(&self, distance: i16) -> Translation4 {
        Translation4::new(*self, distance)
    }
    /*fn pipe_entry(&self) -> Self::P {
        PipeState4 {
            d1: self.clone(),
            d2: self.clone(),
        }
    }*/
    fn list() -> Vec<Self> {
        vec![
            Self::D0,
            Self::D90,
            Self::D180,
            Self::D270,
        ]
    }
    fn mirror_horizontally(&self) -> Self {
        match self {
            Self::D0 => Self::D180,
            Self::D180 => Self::D0,
            _ => self.clone()
        }
    }
}
impl fmt::Display for Direction4 {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::D0 => write!(f, "right"),
            Self::D90 => write!(f, "up"),
            Self::D180 => write!(f, "left"),
            Self::D270 => write!(f, "down"),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash, Zippable)]
#[zippable(bits = 3)]
pub enum Direction6 {
    D0,
    D60,
    D120,
    D180,
    D240,
    D300,
}

impl TryFrom<Attribute<Direction6>> for Direction6 {
    type Error = AttributeError;
    fn try_from(value: Attribute<Direction6>) -> Result<Self, Self::Error> {
        if let Attribute::Direction(value) = value {
            Ok(value)
        } else {
            Err(AttributeError { requested: AttributeKey::Direction, received: Some(value.key()) })
        }
    }
}

impl From<Direction6> for Attribute<Direction6> {
    fn from(value: Direction6) -> Self {
        Attribute::Direction(value)
    }
}

impl Direction for Direction6 {
    type T = Translation6;
    //type P = PipeState6;
    fn is_hex() -> bool {
        true
    }
    fn angle_0() -> Self {
        Self::D0
    }
    fn translation(&self, distance: i16) -> Translation6 {
        Translation6::new(*self, distance)
    }
    /*fn pipe_entry(&self) -> Self::P {
        PipeState6 {
            d1: self.clone(),
            d2: self.clone(),
        }
    }*/
    fn list() -> Vec<Self> {
        vec![
            Self::D0,
            Self::D60,
            Self::D120,
            Self::D180,
            Self::D240,
            Self::D300,
        ]
    }
    fn mirror_horizontally(&self) -> Self {
        match self {
            Self::D0 => Self::D180,
            Self::D180 => Self::D0,
            Self::D60 => Self::D120,
            Self::D120 => Self::D60,
            Self::D240 => Self::D300,
            Self::D300 => Self::D240,
        }
    }
}
impl fmt::Display for Direction6 {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::D0 => write!(f, "right"),
            Self::D60 => write!(f, "up right"),
            Self::D120 => write!(f, "up left"),
            Self::D180 => write!(f, "left"),
            Self::D240 => write!(f, "down left"),
            Self::D300 => write!(f, "down right"),
        }
    }
}

pub trait Translation<D>: Clone + PartialEq
where D: Direction {
    fn new(direction: D, distance: i16) -> Self;
    fn len(&self) -> u16;
    fn between<P: Position<i16>>(from: &P, to: &P, odd_if_hex: bool) -> Self;
    fn plus(&self, other: &Self) -> Self;
    fn minus(&self, other: &Self) -> Self;
    fn multiply(&self, factor: i16) -> Self;
    fn is_parallel(&self, other: &Self) -> bool;
    #[cfg(feature = "rendering")]
    fn screen_coordinates(&self) -> (f32, f32);
    fn rotate_by(&self, angle: D) -> Self;
    fn mirror_horizontally(&self) -> Self;
    fn translate_point<P: Position<i16>>(&self, p: &P, odd_if_hex: bool) -> P;
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash, Zippable)]
#[zippable(support = u16)]
pub struct Translation4 {
    x: i16,
    y: i16,
}
impl Translation<Direction4> for Translation4 {
    fn new(direction: Direction4, distance: i16) -> Self {
        match direction {
            Direction4::D0 => Translation4 {x: distance, y: 0},
            Direction4::D90 => Translation4 {x: 0, y: -distance},
            Direction4::D180 => Translation4 {x: -distance, y: 0},
            Direction4::D270 => Translation4 {x: 0, y: distance},
        }
    }
    fn len(&self) -> u16 {
        (self.x.abs() + self.y.abs()) as u16
    }
    fn between<P: Position<i16>>(from: &P, to: &P, _: bool) -> Self {
        Translation4 {
            x: to.x() - from.x(),
            y: to.y() - from.y(),
        }
    }
    fn plus(&self, other: &Self) -> Self {
        Translation4 {
            x: self.x + other.x,
            y: self.y + other.y,
        }
    }
    fn minus(&self, other: &Self) -> Self {
        Translation4 {
            x: self.x - other.x,
            y: self.y - other.y,
        }
    }
    fn multiply(&self, factor: i16) -> Self {
        Translation4 {
            x: self.x * factor,
            y: self.y * factor,
        }
    }
    fn is_parallel(&self, other: &Self) -> bool {
        if self == other {
            true
        } else if (self.x == 0) != (other.x == 0) || (self.y == 0) != (other.y == 0) {
            false
        } else if self.x == 0 {
            self.y % other.y == 0 || other.y % self.y == 0
        } else {
            self.x % other.x == 0 && self.x / other.x * other.y == self.y || other.x % self.x == 0 && other.x / self.x * self.y == other.y
        }
    }
    #[cfg(feature = "rendering")]
    fn screen_coordinates(&self) -> (f32, f32) {
        (self.x as f32, self.y as f32)
    }
    fn rotate_by(&self, angle: Direction4) -> Self {
        match angle {
            Direction4::D0 => self.clone(),
            Direction4::D90 => Translation4 {x: self.y, y: -self.x},
            Direction4::D180 => Translation4 {x: -self.x, y: -self.y},
            Direction4::D270 => Translation4 {x: -self.y, y: self.x},
        }
    }
    fn mirror_horizontally(&self) -> Self {
        Translation4 {x: -self.x, y: self.y}
    }
    fn translate_point<P: Position<i16>>(&self, p: &P, _: bool) -> P {
        P::new(p.x() + self.x, p.y() + self.y)
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash, Zippable)]
#[zippable(support = u16)]
pub struct Translation6 {
    d0: i16,
    d60: i16,
}
impl Translation<Direction6> for Translation6 {
    fn new(direction: Direction6, distance: i16) -> Self {
        match direction {
            Direction6::D0 => Translation6 {d0: distance, d60: 0},
            Direction6::D60 => Translation6 {d0: 0, d60: distance},
            Direction6::D120 => Translation6 {d0: -distance, d60: distance},
            Direction6::D180 => Translation6 {d0: -distance, d60: 0},
            Direction6::D240 => Translation6 {d0: 0, d60: -distance},
            Direction6::D300 => Translation6 {d0: distance, d60: -distance},
        }
    }
    fn len(&self) -> u16 {
        std::cmp::max((self.d0 + self.d60).abs(), (self.d0 - self.d60).abs()) as u16
    }
    fn between<P: Position<i16>>(from: &P, to: &P, odd_if_hex: bool) -> Self {
        let mut x = to.x() - from.x();
        let y = to.y() - from.y();
        if y % 2 != 0 {
            if y < 0 {
                x -= 1;
            }
            if (from.y() % 2 != 0) == odd_if_hex {
                x += 1;
            }
        }
        Translation6 {
            d0: x + y / 2,
            d60: -y,
        }
    }
    fn plus(&self, other: &Self) -> Self {
        if self.d60.abs() > 1000 || other.d60.abs() > 1000 {
            println!("Translation6 are going too far!, {:?}, {:?}", self, other);
        }
        Translation6 {
            d0: self.d0 + other.d0,
            d60: self.d60 + other.d60,
        }
    }
    fn minus(&self, other: &Self) -> Self {
        Translation6 {
            d0: self.d0 - other.d0,
            d60: self.d60 - other.d60,
        }
    }
    fn multiply(&self, factor: i16) -> Self {
        Translation6 {
            d0: self.d0 * factor,
            d60: self.d60 * factor,
        }
    }
    fn is_parallel(&self, other: &Self) -> bool {
        if self == other {
            true
        } else if (self.d0 == 0) != (other.d0 == 0) || (self.d60 == 0) != (other.d60 == 0) {
            false
        } else if self.d0 == 0 {
            self.d60 % other.d60 == 0 || other.d60 % self.d60 == 0
        } else {
            self.d0 % other.d0 == 0 && self.d0 / other.d0 * other.d60 == self.d60 || other.d0 % self.d0 == 0 && other.d0 / self.d0 * self.d60 == other.d60
        }
    }
    #[cfg(feature = "rendering")]
    fn screen_coordinates(&self) -> (f32, f32) {
        (self.d0 as f32 + (self.d60 as f32) / 2., -self.d60 as f32)
    }
    fn rotate_by(&self, angle: Direction6) -> Self {
        match angle {
            Direction6::D0 => self.clone(),
            Direction6::D60 => Translation6 {d0: -self.d60, d60: self.d0 + self.d60},
            Direction6::D120 => Translation6 {d0: -self.d0 - self.d60, d60: self.d0},
            Direction6::D180 => Translation6 {d0: -self.d0, d60: -self.d60},
            Direction6::D240 => Translation6 {d0: self.d60, d60: -self.d0 - self.d60},
            Direction6::D300 => Translation6 {d0: self.d0 + self.d60, d60: -self.d0},
        }
    }
    fn mirror_horizontally(&self) -> Self {
        Translation6 {d0: -self.d0 - self.d60, d60: self.d60}
    }
    fn translate_point<P: Position<i16>>(&self, p: &P, odd_if_hex: bool) -> P {
        let mut x = p.x() + self.d0 + self.d60 / 2;
        let y = p.y() - self.d60;
        if self.d60 % 2 != 0 {
            if self.d60 < 0 {
                x -= 1;
            }
            if (p.y() % 2 == 0) == odd_if_hex {
                x += 1;
            }
        }
        P::new(x, y)
    }
}

/*pub trait PipeState<D: Direction> {
    fn is_enterable(&self) -> bool;
    fn enterable_from(&self, d: D) -> bool;
    fn connections(&self) -> Vec<D>;
    fn connects_towards(&self, d: D) -> bool;
    fn connect_to(&mut self, d: D); // TODO: maybe return result depending on whether it was able to connect?
    fn next_dir(&self, entered_from: D) -> D;
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash, Zippable)]
pub struct PipeState4 {
    d1: Direction4,
    d2: Direction4,
}
impl PipeState<Direction4> for PipeState4 {
    fn is_enterable(&self) -> bool {
        self.d1 == self.d2
    }
    fn enterable_from(&self, d: Direction4) -> bool {
        self.d1 == self.d2 && self.d1 == d
    }
    fn connections(&self) -> Vec<Direction4> {
        let mut result = vec![self.d1];
        if self.d1 != self.d2 {
            result.push(self.d2);
        }
        result
    }
    fn connects_towards(&self, d: Direction4) -> bool {
        self.d1 == d || self.d2 == d
    }
    fn connect_to(&mut self, d: Direction4) {
        if self.is_enterable() {
            self.d2 = d.clone();
        }
    }
    fn next_dir(&self, entered_from: Direction4) -> Direction4 {
        if self.d1 == entered_from.opposite_direction() {
            if self.is_enterable() {
                self.d2.opposite_direction()
            } else {
                self.d2.clone()
            }
        } else {
            self.d1.clone()
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash, Zippable)]
pub struct PipeState6 {
    d1: Direction6,
    d2: Direction6,
}
impl PipeState<Direction6> for PipeState6 {
    fn is_enterable(&self) -> bool {
        self.d1 == self.d2
    }
    fn enterable_from(&self, d: Direction6) -> bool {
        self.d1 == self.d2 && self.d1 == d
    }
    fn connections(&self) -> Vec<Direction6> {
        let mut result = vec![self.d1];
        if self.d1 != self.d2 {
            result.push(self.d2);
        }
        result
    }
    fn connects_towards(&self, d: Direction6) -> bool {
        self.d1 == d || self.d2 == d
    }
    fn connect_to(&mut self, d: Direction6) {
        let angle_difference = (d.list_index() as i8 - self.d1.list_index() as i8).abs();
        if self.is_enterable() && angle_difference > 1 && angle_difference < 5 {
            self.d2 = d.clone();
        }
    }
    fn next_dir(&self, entered_from: Direction6) -> Direction6 {
        if self.d1 == entered_from.opposite_direction() {
            if self.is_enterable() {
                self.d2.opposite_direction()
            } else {
                self.d2.clone()
            }
        } else {
            self.d1.clone()
        }
    }
}*/
