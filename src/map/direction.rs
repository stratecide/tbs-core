use std::fmt;
use std::hash::Hash;
//use num::{CheckedAdd, CheckedSub, Integer};
use crate::map::point::*;
//use crate::map::point_map::*;

pub trait Direction: Eq + Copy + Hash + fmt::Debug {
    type T: Translation<Self> + Clone + Copy + Hash + PartialEq + Eq + fmt::Debug;
    fn is_hex() -> bool;
    fn angle_0() -> Self;
    fn translation(&self, distance: i16) -> Self::T;
    fn list() -> Vec<Box<Self>>;
    fn mirror_vertically(&self) -> Self;
    //fn rotate_point_map(&self, map: &PointMap) -> PointMap;
    fn get_neighbor(&self, point: &Point, odd_if_hex: bool) -> Option<Point> {
        let gp = self.translation(1).translate_point(&GlobalPoint::new(point.x() as i16, point.y() as i16), odd_if_hex);
        if gp.x() >= 0 && gp.x() <= 255 && gp.y() >= 0 && gp.y() <= 255 {
            Some(Point::new(gp.x() as u8, gp.y() as u8))
        } else {
            None
        }
    }
    fn rotate_around_center<P: Position<i16>>(&self, point: &P, center: &P, odd_if_hex: bool) -> P {
        let trans = Self::T::between(center, point, odd_if_hex);
        let trans = trans.rotate_by(self);
        trans.translate_point(center, odd_if_hex)
    }
    fn list_index(&self) -> usize {
        let list = Self::list();
        list.iter().position(|d| self == d.as_ref()).expect("Unable to find Direction in list of all Directions")
    }
    fn rotate_counter_clockwise(&self) -> Self {
        let list = Self::list();
        let index = self.list_index();
        *list[(index + 1) % list.len()]
    }
    fn rotate_clockwise(&self) -> Self {
        let list = Self::list();
        if self == list[0].as_ref() {
            return **list.last().unwrap();
        }
        let index = self.list_index();
        *list[index - 1]
    }
    fn rotate_by(&self, other: &Self) -> Self {
        let list = Self::list();
        let index = self.list_index();
        let index2 = other.list_index();
        *list[(index + index2) % list.len()]
    }
    fn opposite_angle(&self) -> Self {
        let list = Self::list();
        let index = self.list_index();
        *list[(list.len() - index) % list.len()]
    }
    fn opposite_direction(&self) -> Self {
        let list = Self::list();
        let index = self.list_index();
        *list[(index + list.len() / 2) % list.len()]
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum Direction4 {
    D0,
    D90,
    D180,
    D270,
}
impl Direction for Direction4 {
    type T = Translation4;
    fn is_hex() -> bool {
        false
    }
    fn angle_0() -> Self {
        Self::D0
    }
    fn translation(&self, distance: i16) -> Translation4 {
        Translation4::new(self, distance)
    }
    fn list() -> Vec<Box<Self>> {
        vec![
            Box::new(Self::D0),
            Box::new(Self::D90),
            Box::new(Self::D180),
            Box::new(Self::D270),
        ]
    }
    fn mirror_vertically(&self) -> Self {
        match self {
            Self::D0 => Self::D180,
            Self::D180 => Self::D0,
            _ => self.clone()
        }
    }
    /*fn rotate_point_map(&self, map: &PointMap) -> PointMap {
        let mut result = match self {
            Self::D0 => PointMap::new(map.width(), map.height(), false),
            Self::D90 => PointMap::new(map.height(), map.width(), false),
            Self::D180 => PointMap::new(map.width(), map.height(), false),
            Self::D270 => PointMap::new(map.height(), map.width(), false),
        };
        for x in 0..map.width() {
            for y in 0..map.height() {
                let origin = Point::new(x, y);
                let destination = match self {
                    Self::D0 => Point::new(origin.x(), origin.y()),
                    Self::D90 => Point::new(origin.y(), map.width() - 1 - origin.x()),
                    Self::D180 => Point::new(map.width() - 1 - origin.x(), map.height() - 1 - origin.y()),
                    Self::D270 => Point::new(map.height() - 1 - origin.y(), origin.x()),
                };
                result.set_valid(&destination, map.is_point_valid(&origin));
            }
        }
        result
    }*/
    /*fn get_neighbor<T: CheckedAdd + CheckedSub + Integer, P: Position<T>>(&self, point: &P) -> Option<P> {
        match self {
            Direction4::D0 => point.x().checked_add(&T::one()).map(|x| {P::new(x, point.y())}),
            Direction4::D90 => point.y().checked_sub(&T::one()).map(|y| {P::new(point.x(), y)}),
            Direction4::D180 => point.x().checked_sub(&T::one()).map(|x| {P::new(x, point.y())}),
            Direction4::D270 => point.y().checked_add(&T::one()).map(|y| {P::new(point.x(), y)}),
        }
    }*/
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

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum Direction6 {
    D0,
    D60,
    D120,
    D180,
    D240,
    D300,
}
impl Direction for Direction6 {
    type T = Translation6;
    fn is_hex() -> bool {
        true
    }
    fn angle_0() -> Self {
        Self::D0
    }
    fn translation(&self, distance: i16) -> Translation6 {
        Translation6::new(self, distance)
    }
    fn list() -> Vec<Box<Self>> {
        vec![
            Box::new(Self::D0),
            Box::new(Self::D60),
            Box::new(Self::D120),
            Box::new(Self::D180),
            Box::new(Self::D240),
            Box::new(Self::D300),
        ]
    }
    fn mirror_vertically(&self) -> Self {
        match self {
            Self::D0 => Self::D180,
            Self::D180 => Self::D0,
            Self::D60 => Self::D120,
            Self::D120 => Self::D60,
            Self::D240 => Self::D300,
            Self::D300 => Self::D240,
        }
    }
    /*fn rotate_point_map(&self, map: &PointMap) -> PointMap {
        let mut result = match self {
            Self::D0 => PointMap::new(map.width(), map.height(), map.odd_if_hex()),
            Self::D90 => PointMap::new(map.height(), map.width(), false),
            Self::D180 => PointMap::new(map.width(), map.height(), map.odd_if_hex() == (map.height() % 2 == 0)),
            Self::D270 => PointMap::new(map.height(), map.width(), false),
        };
        for x in 0..map.width() {
            for y in 0..map.height() {
                let origin = Point::new(x, y);
                let destination = match self {
                    Self::D0 => Point::new(origin.x(), origin.y()),
                    Self::D90 => Point::new(origin.y(), map.width() - 1 - origin.x()),
                    Self::D180 => Point::new(map.width() - 1 - origin.x(), map.height() - 1 - origin.y()),
                    Self::D270 => Point::new(map.height() - 1 - origin.y(), origin.x()),
                };
                result.set_valid(&destination, map.is_point_valid(&origin));
            }
        }
        result
    }*/
    /*fn get_neighbor<T: CheckedAdd + CheckedSub + Integer, P: Position<T>>(&self, point: &P) -> Option<P> {
        match (self, point.y().is_even()) {
            (Direction6::D0, _) => point.x().checked_add(&T::one()).map(|x| {P::new(x, point.y())}),
            (Direction6::D180, _) => point.x().checked_sub(&T::one()).map(|x| {P::new(x, point.y())}),
            (Direction6::D60, true) => point.y().checked_sub(&T::one()).map(|y| {P::new(point.x(), y)}),
            (Direction6::D60, false) => point.y().checked_sub(&T::one()).and_then(|y| point.x().checked_add(&T::one()).map(|x| {P::new(x, y)})),
            (Direction6::D120, true) => point.y().checked_sub(&T::one()).and_then(|y| point.x().checked_sub(&T::one()).map(|x| {P::new(x, y)})),
            (Direction6::D120, false) => point.y().checked_sub(&T::one()).map(|y| {P::new(point.x(), y)}),
            (Direction6::D240, true) => point.y().checked_sub(&T::one()).and_then(|y| point.x().checked_sub(&T::one()).map(|x| {P::new(x, y)})),
            (Direction6::D240, false) => point.y().checked_add(&T::one()).map(|y| {P::new(point.x(), y)}),
            (Direction6::D300, true) => point.y().checked_add(&T::one()).map(|y| {P::new(point.x(), y)}),
            (Direction6::D300, false) => point.y().checked_sub(&T::one()).and_then(|y| point.x().checked_add(&T::one()).map(|x| {P::new(x, y)})),
        }
    }*/
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
    fn new(direction: &D, distance: i16) -> Self;
    fn len(&self) -> u16;
    fn between<P: Position<i16>>(from: &P, to: &P, odd_if_hex: bool) -> Self;
    fn plus(&self, other: &Self) -> Self;
    fn minus(&self, other: &Self) -> Self;
    fn is_parallel(&self, other: &Self) -> bool;
    fn screen_coordinates(&self) -> (f32, f32);
    fn rotate_by(&self, angle: &D) -> Self;
    fn mirror_vertically(&self) -> Self;
    fn translate_point<P: Position<i16>>(&self, p: &P, odd_if_hex: bool) -> P;
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub struct Translation4 {
    x: i16,
    y: i16,
}
impl Translation<Direction4> for Translation4 {
    fn new(direction: &Direction4, distance: i16) -> Self {
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
    fn screen_coordinates(&self) -> (f32, f32) {
        (self.x as f32, self.y as f32)
    }
    fn rotate_by(&self, angle: &Direction4) -> Self {
        match angle {
            Direction4::D0 => self.clone(),
            Direction4::D90 => Translation4 {x: self.y, y: -self.x},
            Direction4::D180 => Translation4 {x: -self.x, y: -self.y},
            Direction4::D270 => Translation4 {x: -self.y, y: self.x},
        }
    }
    fn mirror_vertically(&self) -> Self {
        Translation4 {x: -self.x, y: self.y}
    }
    fn translate_point<P: Position<i16>>(&self, p: &P, _: bool) -> P {
        P::new(p.x() + self.x, p.y() + self.y)
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub struct Translation6 {
    d0: i16,
    d60: i16,
}
impl Translation<Direction6> for Translation6 {
    fn new(direction: &Direction6, distance: i16) -> Self {
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
        let mut y = to.y() - from.y();
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
    fn screen_coordinates(&self) -> (f32, f32) {
        (self.d0 as f32 + (self.d60 as f32) / 2., -self.d60 as f32)
    }
    fn rotate_by(&self, angle: &Direction6) -> Self {
        match angle {
            Direction6::D0 => self.clone(),
            Direction6::D60 => Translation6 {d0: -self.d60, d60: self.d0 + self.d60},
            Direction6::D120 => Translation6 {d0: -self.d0 - self.d60, d60: self.d0},
            Direction6::D180 => Translation6 {d0: -self.d0, d60: -self.d60},
            Direction6::D240 => Translation6 {d0: self.d60, d60: -self.d0 - self.d60},
            Direction6::D300 => Translation6 {d0: self.d0 + self.d60, d60: -self.d0},
        }
    }
    fn mirror_vertically(&self) -> Self {
        Translation6 {d0: -self.d0 - self.d60, d60: self.d60}
    }
    fn translate_point<P: Position<i16>>(&self, p: &P, odd_if_hex: bool) -> P {
        let mut x = p.x() + self.d0 + self.d60 / 2;
        let mut y = p.y() - self.d60;
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
