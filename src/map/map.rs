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
    terrain: HashMap<Point, Terrain<D>>,
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
    pub fn odd_if_hex(&self) -> bool {
        self.wrapping_logic.pointmap().odd_if_hex()
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
    pub fn get_terrain(&self, p: &Point) -> Option<&Terrain<D>> {
        self.terrain.get(p)
    }
    pub fn get_terrain_mut(&mut self, p: &Point) -> Option<&mut Terrain<D>> {
        self.terrain.get_mut(p)
    }
    pub fn set_terrain(&mut self, p: Point, t: Terrain<D>) {
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
    pub fn validate_terrain(&mut self) -> Vec<(Point, Terrain<D>)> {
        let mut corrected = vec![];
        for p in self.wrapping_logic.pointmap().get_valid_points() {
            match self.get_terrain(&p).unwrap() {
                Terrain::Pipe(state) => {
                    let mut is_valid = true;
                    let mut valid_dir = None;
                    for d in state.connections() {
                        if let Some(dp) = self.wrapping_logic.get_neighbor(&p, &d) {
                            match self.get_terrain(&dp.point()).unwrap() {
                                Terrain::Pipe(state) => {
                                    if state.connects_towards(&dp.direction().opposite_direction()) {
                                        valid_dir = Some(d)
                                    } else {
                                        is_valid = false;
                                    }
                                }
                                _ => is_valid = false
                            }
                        } else {
                            is_valid = false;
                        }
                    }
                    if !is_valid {
                        corrected.push((p.clone(), self.terrain.remove(&p).unwrap()));
                        if let Some(dir) = valid_dir {
                            self.set_terrain(p, Terrain::Pipe(dir.pipe_entry()));
                        } else {
                            self.set_terrain(p, Terrain::Grass);
                        }
                    }
                }
                _ => {}
            }
        }
        corrected
    }
}
