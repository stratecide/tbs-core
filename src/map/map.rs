use std::collections::HashMap;

use crate::game::settings;
use crate::game::game::*;
use crate::game::events;
use crate::map::wrapping_map::*;
use crate::map::direction::*;
use crate::map::point::*;
use crate::player::*;
use crate::terrain::*;
use crate::units::*;
use crate::units::mercenary::Mercenary;
use crate::details::*;

#[derive(Clone)]
pub struct Map<D>
where D: Direction
{
    wrapping_logic: WrappingMap<D>,
    terrain: HashMap<Point, Terrain<D>>,
    units: HashMap<Point, UnitType<D>>,
    details: HashMap<Point, Vec<Detail>>,
}

impl<D> Map<D>
where D: Direction
{
    pub fn new(wrapping_logic: WrappingMap<D>) -> Self {
        let mut terrain = HashMap::new();
        for p in wrapping_logic.pointmap().get_valid_points() {
            terrain.insert(p, Terrain::Grass);
        }
        Map {
            wrapping_logic,
            terrain,
            units: HashMap::new(),
            details: HashMap::new(),
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
    /**
     * checks the pipe at dp.point for whether it can be entered by dp.direction and if true, returns the position of the next pipe tile
     * returns None if no pipe is at the given location, for example because the previous pipe tile was an exit
     */
    fn next_pipe_tile(&self, dp: &OrientedPoint<D>) -> Option<OrientedPoint<D>> {
        match self.terrain.get(dp.point()) {
            Some(Terrain::Pipe(pipe_state)) => {
                if pipe_state.connects_towards(&dp.direction().opposite_direction()) || pipe_state.enterable_from(dp.direction()) {
                    self.wrapping_logic.get_neighbor(dp.point(), &pipe_state.next_dir(dp.direction()))
                } else {
                    None
                }
            }
            _ => None,
        }
    }
    pub fn get_direction(&self, from: &Point, to: &Point) -> Option<D> {
        for d in D::list() {
            if let Some(dp) = self.get_neighbor(from, &d) {
                if dp.point() == to {
                    return Some(*d);
                }
            }
        }
        None
    }
    pub fn get_neighbor(&self, p: &Point, d: &D) -> Option<OrientedPoint<D>> {
        if let Some(n) = self.wrapping_logic.get_neighbor(p, d) {
            match self.terrain.get(n.point()) {
                Some(Terrain::Pipe(pipe_state)) => {
                    if pipe_state.enterable_from(n.direction()) || pipe_state.connects_towards(&n.direction().opposite_direction()) {
                        let mut dp = n.clone();
                        while let Some(next) = self.next_pipe_tile(&dp) {
                            dp = next;
                            if dp.point() == n.point() {
                                // infinite loop, abort
                                return None;
                            }
                        }
                        Some(dp)
                    } else {
                        Some(n)
                    }
                }
                _ => Some(n),
            }
        } else {
            None
        }
    }
    pub fn get_neighbors(&self, p: &Point, mode: NeighborMode) -> Vec<OrientedPoint<D>> {
        let mut result = vec![];
        for d in D::list() {
            match mode {
                NeighborMode::Direct => {
                    if let Some(neighbor) = self.wrapping_logic.get_neighbor(p, &d) {
                        result.push(neighbor);
                    }
                }
                NeighborMode::FollowPipes => {
                    if let Some(neighbor) = self.get_neighbor(p, &d) {
                        result.push(neighbor);
                    }
                }
                NeighborMode::UnitMovement => {
                    if let Some(neighbor) = self.get_neighbor(p, &d) {
                        if self.terrain.get(p) == Some(&Terrain::Fountain) {
                            if let Some(neighbor) = self.get_neighbor(neighbor.point(), neighbor.direction()) {
                                result.push(neighbor);
                            }
                        }
                        result.push(neighbor);
                    }
                }
            }
        }
        result
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
    pub fn get_unit(&self, p: &Point) -> Option<&UnitType<D>> {
        self.units.get(p)
    }
    pub fn get_unit_mut(&mut self, p: &Point) -> Option<&mut UnitType<D>> {
        self.units.get_mut(p)
    }
    pub fn set_unit(&mut self, p: Point, unit: Option<UnitType<D>>) -> Option<UnitType<D>> {
        // TODO: return a Result<(), ?>, returning an error if the point is invalid
        if let Some(unit) = unit {
            if self.wrapping_logic.pointmap().is_point_valid(&p) {
                self.units.insert(p, unit)
            } else {
                None
            }
        } else {
            self.units.remove(&p)
        }
    }
    pub fn get_details(&self, p: &Point) -> Vec<Detail> {
        self.details.get(p).and_then(|v| Some(v.clone())).unwrap_or(vec![])
    }
    pub fn set_details(&mut self, p: Point, value: Vec<Detail>) {
        if self.wrapping_logic.pointmap().is_point_valid(&p) {
            // remove Detail from value that conflict with other Detail
            // starting from the back, so add_detail can be used by the editor to overwrite previous data
            let mut bubble = false;
            let mut coin = false;
            let value: Vec<Detail> = value.into_iter().rev().filter(|detail| {
                let remove;
                match detail {
                    Detail::FactoryBubble(_) => {
                        remove = bubble;
                        bubble = true;
                    }
                    Detail::Coins1 | Detail::Coins2 | Detail::Coins4 => {
                        remove = coin;
                        coin = true;
                    }
                }
                !remove
            }).take(MAX_STACK_SIZE).collect();
            let value = value.into_iter().rev().collect();
            self.details.insert(p, value);
        }
    }
    pub fn add_detail(&mut self, p: Point, value: Detail) {
        let mut list: Vec<Detail> = self.get_details(&p).into_iter().map(|f| f.clone()).collect();
        list.push(value);
        self.set_details(p, list);
    }
    pub fn insert_detail(&mut self, p: Point, index: usize, value: Detail) {
        let mut list: Vec<Detail> = self.get_details(&p).into_iter().map(|f| f.clone()).collect();
        if index <= list.len() {
            list.insert(index, value);
            self.set_details(p, list);
        }
    }
    pub fn remove_detail(&mut self, p: &Point, index: usize) -> Option<Detail> {
        if let Some(list) = self.details.get_mut(p) {
            if list.len() > index {
                return Some(list.remove(index));
            }
        }
        None
    }
    pub fn mercenary_influence_at(&self, point: &Point, owner: Option<&Owner>) -> Vec<(Point, &Mercenary)> {
        let mut result = vec![];
        for p in self.wrapping_logic.pointmap().get_valid_points() {
            if let Some(UnitType::Mercenary(merc)) = self.get_unit(&p) {
                if (owner.is_none() || owner == Some(&merc.unit.owner)) && merc.in_range(self, &p, &point) {
                    result.push((p.clone(), merc));
                }
            }
        }
        result
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
    pub fn get_players(&self) -> Vec<Player> {
        vec![
            Player::new(0, 0, 100, 333),
            Player::new(1, 1, 144, 210),
        ]
    }
    /**
     * returns Ok(...) if the map is playable
     * returns Err(...) containing the reason otherwise
     */
    pub fn settings(&self) -> Result<settings::GameSettings, settings::NotPlayable> {
        // TODO: check if playable
        Ok(settings::GameSettings {
            fog_mode: FogMode::DarkRegular(3, 4, 3),
        })
    }
    pub fn game_server<R: Fn() -> f32>(self, settings: &settings::GameSettings, random: R) -> (Game<D>, HashMap<Option<Perspective>, Vec<events::Event<D>>>) {
        Game::new_server(self, settings, random)
    }
    pub fn game_client(self, settings: &settings::GameSettings, events: &Vec<events::Event<D>>) -> Game<D> {
        Game::new_client(self, settings, events)
    }
}

#[derive(Debug, Clone, Copy)]
pub enum NeighborMode {
    Direct,
    FollowPipes,
    UnitMovement,
}
