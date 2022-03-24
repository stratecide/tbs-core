use std::collections::HashMap;

use crate::map::wrapping_map::*;
use crate::map::direction::*;
use crate::map::point::*;
use crate::terrain::*;
use crate::units::*;


pub struct Map<D>
where D: Direction
{
    wrapping_logic: WrappingMap<D>,
    terrain: HashMap<Point, Terrain>,
    units: HashMap<Point, UnitType>,
}

impl<D> Map<D>
where D: Direction
{
    pub fn new(wrapping_logic: WrappingMap<D>) -> Self {
        let mut terrain = HashMap::new();
        for p in wrapping_logic.pointmap().get_valid_points() {
            terrain.insert(p, Terrain::Fountain);
        }
        Map {
            wrapping_logic,
            terrain,
            units: HashMap::new(),
        }
    }
    pub fn wrapping_logic(&self) -> &WrappingMap<D> {
        &self.wrapping_logic
    }
    pub fn width(&self) -> u8 {
        self.wrapping_logic.pointmap().width()
    }
    pub fn height(&self) -> u8 {
        self.wrapping_logic.pointmap().height()
    }
    pub fn all_points(&self) -> Vec<Point> {
        self.wrapping_logic.pointmap().get_valid_points()
    }
    pub fn get_neighbor(&self, p: &Point, d: &D) -> Option<OrientedPoint<D>> {
        self.wrapping_logic.get_neighbor(p, d)
    }
    pub fn get_terrain(&self, p: &Point) -> Option<&Terrain> {
        self.terrain.get(p)
    }
    pub fn get_terrain_mut(&mut self, p: &Point) -> Option<&mut Terrain> {
        self.terrain.get_mut(p)
    }
    pub fn set_terrain(&mut self, p: Point, t: Terrain) {
        // TODO: return a Result<(), ?>
        if self.wrapping_logic.pointmap().is_point_valid(&p) {
            self.terrain.insert(p, t);
        }
    }
    pub fn get_unit(&self, p: &Point) -> Option<&UnitType> {
        self.units.get(p)
    }
    pub fn get_unit_mut(&mut self, p: &Point) -> Option<&mut UnitType> {
        self.units.get_mut(p)
    }
    pub fn set_unit(&mut self, p: Point, unit: Option<UnitType>) {
        // TODO: return a Result<(), ?>
        if let Some(unit) = unit {
            if self.wrapping_logic.pointmap().is_point_valid(&p) {
                self.units.insert(p, unit);
            }
        } else {
            self.units.remove(&p);
        }
    }
}
