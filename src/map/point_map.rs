use crate::map::point::*;

use zipper::*;
use zipper::zipper_derive::*;

pub const MIN_SIZE:u8 = 3;
pub const MAX_SIZE:u32 = 50;
pub const MAX_AREA:u32 = MAX_SIZE * MAX_SIZE;

#[derive(Debug, Clone, PartialEq, Eq, Zippable)]
pub struct PointMap {
    odd_if_hex: bool,
    point_validity: LVec::<LVec::<bool, MAX_SIZE>, MAX_SIZE>,
}

impl PointMap {
    pub fn new(width: u8, height: u8, odd_if_hex: bool) -> Self {
        Self::filled(width, height, odd_if_hex, true)
    }
    fn filled(width: u8, height: u8, odd_if_hex: bool, value: bool) -> Self {
        PointMap {
            odd_if_hex: odd_if_hex,
            point_validity: vec![vec![value; width as usize].try_into().unwrap(); height as usize].try_into().unwrap(),
        }
    }
    pub fn width(&self) -> u8 {
        self.point_validity[0].len() as u8
    }
    pub fn height(&self) -> u8 {
        self.point_validity.len() as u8
    }
    pub fn odd_if_hex(&self) -> bool {
        self.odd_if_hex
    }
    /**
     * removes columns and rows from the sides that contain no valid points
     */
    pub fn crop(&mut self) {
        // from bottom
        while self.height() > MIN_SIZE && !self.point_validity[self.height() as usize - 1].iter().any(|b| *b) {
            self.point_validity.pop();
        }
        // from top
        while self.height() > MIN_SIZE && !self.point_validity[0].iter().any(|b| *b) {
            self.point_validity.remove(0);
            self.odd_if_hex = !self.odd_if_hex;
        }
        // from left
        while self.width() > MIN_SIZE && !self.point_validity.iter().any(|b| b[0]) {
            for row in self.point_validity.iter_mut() {
                row.remove(0);
            }
        }
        // from right
        while self.width() > MIN_SIZE && !self.point_validity.iter().any(|b| b[self.width() as usize - 1]) {
            for row in self.point_validity.iter_mut() {
                row.pop();
            }
        }
    }
    pub fn is_inside(&self, point: &Point) -> bool {
        point.x() < self.width() &&
        point.y() < self.height()
    }
    pub fn is_point_valid(&self, point: &Point) -> bool {
        self.is_inside(point) &&
        self.point_validity[point.y() as usize][point.x() as usize]
    }
    pub fn set_valid(&mut self, point: &Point, value: bool) {
        if self.is_inside(point) {
            *self.point_validity.get_mut(point.y() as usize).unwrap().get_mut(point.x() as usize).unwrap() = value;
        }
    }
    pub fn get_valid_points(&self) -> Vec<Point> {
        let mut result = vec![];
        for x in 0..self.width() {
            for y in 0..self.height() {
                let p = Point::new(x, y);
                if self.is_point_valid(&p) {
                    result.push(p);
                }
            }
        }
        result
    }
}

