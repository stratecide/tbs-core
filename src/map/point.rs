use zipper::*;

pub trait Position<T> {
    fn new(x: T, y: T) -> Self;
    fn x(&self) -> T;
    fn y(&self) -> T;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Point {
    pub x: u8,
    pub y: u8,
}
impl Zippable for Point {
    fn import(unzipper: &mut Unzipper) -> Result<Self, ZipperError> {
        Ok(Self {
            x: unzipper.read_u8(8)?,
            y: unzipper.read_u8(8)?,
        })
    }
    fn export(&self, zipper: &mut Zipper) {
        zipper.write_u8(self.x, 8);
        zipper.write_u8(self.y, 8);
    }
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
    x: i16,
    y: i16,
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

