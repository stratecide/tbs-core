use zipper::*;
use zipper_derive::Zippable;
use crate::config::environment::Environment;

pub trait Position<T> {
    fn new(x: T, y: T) -> Self;
    fn x(&self) -> T;
    fn y(&self) -> T;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Zippable)]
#[zippable(support_ref = Environment)]
pub struct Point {
    #[supp(*support.map_size.width as u8)]
    pub x: u8,
    #[supp(*support.map_size.height as u8)]
    pub y: u8,
}

impl Position<u8> for Point {
    fn new(x: u8, y: u8) -> Self {
        Point {x, y}
    }
    fn x(&self) -> u8 {
        self.x
    }
    fn y(&self) -> u8 {
        self.y
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct GlobalPoint {
    pub x: i16,
    pub y: i16,
}

impl Position<i16> for GlobalPoint {
    fn new(x: i16, y: i16) -> Self {
        GlobalPoint {x, y}
    }
    fn x(&self) -> i16 {
        self.x
    }
    fn y(&self) -> i16 {
        self.y
    }
}

