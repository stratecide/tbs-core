
pub trait Position<T> {
    fn new(x: T, y: T) -> Self;
    fn x(&self) -> T;
    fn y(&self) -> T;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Point {
    x: u8,
    y: u8,
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
