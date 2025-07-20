use crate::map::point::*;

use zipper::*;
use zipper::zipper_derive::*;

use super::direction::*;

pub const MIN_SIZE:u8 = 3;
pub const MAX_SIZE:u32 = 50;
pub const MAX_AREA:u32 = MAX_SIZE * MAX_SIZE;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Zippable)]
pub struct MapSize {
    pub width: U<{MAX_SIZE as i32}>,
    pub height: U<{MAX_SIZE as i32}>,
}

impl MapSize {
    pub fn new(width: u8, height: u8) -> Self {
        Self {
            width: width.into(),
            height: height.into(),
        }
    }
    pub fn width(&self) -> u8 {
        *self.width as u8
    }
    pub fn height(&self) -> u8 {
        *self.height as u8
    }
}

impl<Z: Zippable> SupportedZippable<MapSize> for Vec<Vec<Z>> {
    fn export(&self, zipper: &mut Zipper, support: MapSize) {
        for y in 0..*support.height as usize {
            for x in 0..*support.width as usize {
                self[y][x].zip(zipper);
            }
        }
    }
    fn import(unzipper: &mut Unzipper, support: MapSize) -> Result<Self, ZipperError> {
        let mut rows = Vec::new();
        for _ in 0..*support.height {
            let mut row = Vec::new();
            for _ in 0..*support.width {
                row.push(Z::unzip(unzipper)?)
            }
            rows.push(row);
        }
        Ok(rows)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Zippable)]
#[zippable(support = MapSize, get_support = size)]
pub struct PointMap {
    odd_if_hex: bool,
    point_validity: Vec<Vec<bool>>,
}

impl PointMap {
    pub fn new(width: u8, height: u8, odd_if_hex: bool) -> Self {
        Self::filled(width, height, odd_if_hex, true)
    }

    pub fn filled(width: u8, height: u8, odd_if_hex: bool, value: bool) -> Self {
        PointMap {
            odd_if_hex,
            point_validity: vec![vec![value; width as usize].try_into().unwrap(); height as usize].try_into().unwrap(),
        }
    }

    pub fn size(&self) -> MapSize {
        MapSize {
            width: self.width().into(),
            height: self.height().into(),
        }
    }
    pub fn width(&self) -> u8 {
        self.point_validity.get(0).map(|pv| pv.len() as u8).unwrap_or(0)
    }
    pub fn height(&self) -> u8 {
        self.point_validity.len() as u8
    }

    /**
     * top left corner of the map:
     * true | false
     *   ___|   ____
     *  /   |   \
     *  \   |   /
     */
    pub fn odd_if_hex(&self) -> bool {
        self.odd_if_hex
    }

    /**
     * removes columns and rows from the sides that contain no valid points
     */
    pub fn crop<D: Direction>(&mut self) -> [D::T; 2] {
        let was_odd = self.odd_if_hex;
        // from bottom
        while self.height() > 0 && !self.point_validity[self.height() as usize - 1].iter().any(|b| *b) {
            self.point_validity.pop();
        }
        let mut from_top = 0;
        // from top
        while self.height() > 0 && !self.point_validity[0].iter().any(|b| *b) {
            from_top += 1;
            self.point_validity.remove(0);
            if D::is_hex() {
                self.odd_if_hex = !self.odd_if_hex;
            }
        }
        let mut from_left = 0;
        // from left
        while self.width() > 0 && !self.point_validity.iter().any(|b| b[0]) {
            from_left += 1;
            for row in self.point_validity.iter_mut() {
                row.remove(0);
            }
        }
        let mut translations = [D::T::between(&GlobalPoint::new(from_left, from_top), &GlobalPoint::ZERO, was_odd); 2];
        if D::is_hex() && self.width() > 0 && self.height() > 0 {
            // check if flipping oddness could reduce size
            let flipped_oddness = if (0..self.height() as usize).all(|y| {
                (self.odd_if_hex == (y % 2 == 0))
                || !self.point_validity[y][0]
            }) {
                for y in 0..self.height() as usize {
                    if self.odd_if_hex != (y % 2 == 0) {
                        self.point_validity[y].remove(0);
                        self.point_validity[y].push(false);
                    };
                }
                self.odd_if_hex = !self.odd_if_hex;
                true
            } else {
                false
            };
            let move_one_left = D::T::between(&GlobalPoint::new(1, 0), &GlobalPoint::ZERO, false);
            if flipped_oddness && self.odd_if_hex {
                translations[0] += move_one_left;
                translations[1] = translations[0];
            }
            if was_odd != self.odd_if_hex {
                translations[1] += if self.odd_if_hex {
                    -move_one_left
                } else {
                    move_one_left
                };
            }
        }
        // from right
        while self.width() > 0 && !self.point_validity.iter().any(|b| b[self.width() as usize - 1]) {
            for row in self.point_validity.iter_mut() {
                row.pop();
            }
        }
        translations
    }

    pub fn is_inside(&self, point: Point) -> bool {
        point.x() < self.width() &&
        point.y() < self.height()
    }
    pub fn is_point_valid(&self, point: Point) -> bool {
        self.is_inside(point) &&
        self.point_validity[point.y() as usize][point.x() as usize]
    }
    pub fn set_valid(&mut self, point: Point, value: bool) {
        if self.is_inside(point) {
            *self.point_validity.get_mut(point.y() as usize).unwrap().get_mut(point.x() as usize).unwrap() = value;
        }
    }
    pub fn get_valid_points(&self) -> Vec<Point> {
        let mut result = vec![];
        for x in 0..self.width() {
            for y in 0..self.height() {
                let p = Point::new(x, y);
                if self.is_point_valid(p) {
                    result.push(p);
                }
            }
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use semver::Version;

    use crate::{map::point::Point, VERSION};
    use super::*;


    #[test]
    fn filled_point_map() {
        let map = PointMap::new(5, 6, false);
        assert_eq!(map.width(), 5);
        assert_eq!(map.height(), 6);
        assert_eq!(map.size().width(), 5);
        assert_eq!(map.size().height(), 6);
        for x in 0..5 {
            for y in 0..6 {
                assert!(map.is_point_valid(Point::new(x, y)));
            }
            assert!(!map.is_point_valid(Point::new(x, 6)));
        }
        for y in 0..6 {
            assert!(!map.is_point_valid(Point::new(5, y)));
        }
    }

    #[test]
    fn export_import_point_map() {
        let mut map = PointMap::new(5, 6, false);
        map.set_valid(Point::new(2, 5), false);
        map.set_valid(Point::new(1, 0), false);
        let mut zipper = Zipper::new();
        map.zip(&mut zipper);
        zipper.write_u8(0b10101010, 8);
        let data = zipper.finish();
        crate::debug!("export_import_point_map, {data:?}");
        let mut unzipper = Unzipper::new(data, Version::parse(VERSION).unwrap());
        assert_eq!(Ok(map), PointMap::unzip(&mut unzipper));
        assert_eq!(Ok(0b10101010), unzipper.read_u8(8));
    }

    #[test]
    fn crop() {
        let mut map = PointMap::filled(5, 6, false, false);
        let translations = map.crop::<Direction4>();
        assert_eq!(translations, [Direction4::D0.translation(0); 2]);
        assert_eq!(map.width(), 0);
        assert_eq!(map.height(), 0);
        assert_eq!(map.odd_if_hex(), false);

        let mut map = PointMap::filled(7, 6, false, false);
        for y in 0..map.height() {
            map.set_valid(Point::new(1, y), true);
        }
        let points = map.get_valid_points();
        let translations = map.crop::<Direction6>();
        assert_eq!(translations, [Direction6::D0.translation(-1); 2]);
        assert_eq!(map.width(), 1);
        assert_eq!(map.height(), 6);
        assert_eq!(map.odd_if_hex(), false);
        for p in points {
            assert!(map.is_point_valid(p.translate::<Direction6>(&translations[p.y as usize % 2], map.odd_if_hex())));
        }

        let mut map = PointMap::filled(7, 6, false, false);
        for x in 0..map.width() {
            map.set_valid(Point::new(x, 3), true);
        }
        let points = map.get_valid_points();
        let translations = map.crop::<Direction4>();
        assert_eq!(translations, [Direction4::D270.translation(-3); 2]);
        assert_eq!(map.width(), 7);
        assert_eq!(map.height(), 1);
        assert_eq!(map.odd_if_hex(), false);
        for p in points {
            assert!(map.is_point_valid(p.translate::<Direction4>(&translations[p.y as usize % 2], map.odd_if_hex())));
        }

        let mut map = PointMap::filled(7, 6, false, false);
        map.set_valid(Point::new(3, 0), true);
        map.set_valid(Point::new(2, 1), true);
        let points = map.get_valid_points();
        let translations = map.crop::<Direction6>();
        assert_eq!(translations, [Direction6::D0.translation(-3), Direction6::D0.translation(-2)]);
        assert_eq!(map.width(), 1);
        assert_eq!(map.height(), 2);
        assert_eq!(map.odd_if_hex(), true);
        for p in points {
            assert!(map.is_point_valid(p.translate::<Direction6>(&translations[p.y as usize % 2], map.odd_if_hex())));
        }

        let mut map = PointMap::filled(2, 2, false, false);
        map.set_valid(Point::new(1, 0), true);
        map.set_valid(Point::new(0, 1), true);
        let points = map.get_valid_points();
        let translations = map.crop::<Direction6>();
        assert_eq!(translations, [Direction6::D0.translation(-1), Direction6::D0.translation(0)]);
        assert_eq!(map.width(), 1);
        assert_eq!(map.height(), 2);
        assert_eq!(map.odd_if_hex(), true);
        for p in points {
            assert!(map.is_point_valid(p.translate::<Direction6>(&translations[p.y as usize % 2], map.odd_if_hex())));
        }

        let mut map = PointMap::filled(2, 2, true, false);
        map.set_valid(Point::new(0, 0), true);
        map.set_valid(Point::new(1, 1), true);
        let points = map.get_valid_points();
        let translations = map.crop::<Direction6>();
        assert_eq!(translations, [Direction6::D0.translation(0), Direction6::D0.translation(-1)]);
        assert_eq!(map.width(), 1);
        assert_eq!(map.height(), 2);
        assert_eq!(map.odd_if_hex(), false);
        for p in points {
            assert!(map.is_point_valid(p.translate::<Direction6>(&translations[p.y as usize % 2], map.odd_if_hex())));
        }
    
        let mut map = PointMap::filled(2, 3, true, false);
        map.set_valid(Point::new(1, 1), true);
        map.set_valid(Point::new(0, 2), true);
        let points = map.get_valid_points();
        let translations = map.crop::<Direction6>();
        assert_eq!(translations, [Direction6::D0.translation(-1) + Direction6::D60.translation(1); 2]);
        assert_eq!(map.width(), 1);
        assert_eq!(map.height(), 2);
        assert_eq!(map.odd_if_hex(), true);
        for p in points {
            assert!(map.is_point_valid(p.translate::<Direction6>(&translations[p.y as usize % 2], map.odd_if_hex())), "{p:?} -> {:?}", p.translate::<Direction6>(&translations[p.y as usize % 2], map.odd_if_hex()));
        }
    
        let mut map = PointMap::filled(2, 3, false, false);
        map.set_valid(Point::new(0, 1), true);
        map.set_valid(Point::new(1, 2), true);
        let points = map.get_valid_points();
        let translations = map.crop::<Direction6>();
        assert_eq!(translations, [Direction6::D0.translation(-1) + Direction6::D60.translation(1); 2]);
        assert_eq!(map.width(), 1);
        assert_eq!(map.height(), 2);
        assert_eq!(map.odd_if_hex(), false);
        for p in points {
            assert!(map.is_point_valid(p.translate::<Direction6>(&translations[p.y as usize % 2], map.odd_if_hex())), "{p:?}, {:?}", p.translate::<Direction6>(&translations[p.y as usize % 2], map.odd_if_hex()));
        }
    
        let mut map = PointMap::filled(2, 3, false, false);
        map.set_valid(Point::new(1, 1), true);
        map.set_valid(Point::new(1, 2), true);
        let points = map.get_valid_points();
        let translations = map.crop::<Direction6>();
        assert_eq!(translations, [Direction6::D0.translation(-2) + Direction6::D60.translation(1), Direction6::D0.translation(-1) + Direction6::D60.translation(1)]);
        assert_eq!(map.width(), 1);
        assert_eq!(map.height(), 2);
        assert_eq!(map.odd_if_hex(), true);
        for p in points {
            assert!(map.is_point_valid(p.translate::<Direction6>(&translations[p.y as usize % 2], map.odd_if_hex())), "{p:?}, {:?}", p.translate::<Direction6>(&translations[p.y as usize % 2], map.odd_if_hex()));
        }

        let mut map = PointMap::filled(2, 3, true, false);
        map.set_valid(Point::new(0, 1), true);
        map.set_valid(Point::new(1, 1), true);
        map.set_valid(Point::new(0, 2), true);
        map.set_valid(Point::new(1, 2), true);
        let points = map.get_valid_points();
        let translations = map.crop::<Direction6>();
        assert_eq!(map.width(), 2);
        assert_eq!(map.height(), 2);
        assert_eq!(map.odd_if_hex(), false);
        for p in points {
            assert!(map.is_point_valid(p.translate::<Direction6>(&translations[p.y as usize % 2], map.odd_if_hex())), "{p:?} -> {:?}", p.translate::<Direction6>(&translations[p.y as usize % 2], map.odd_if_hex()));
        }
    }
}
