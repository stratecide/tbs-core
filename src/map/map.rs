use std::collections::{HashMap, HashSet};

use interfaces::game_interface::Events;
use zipper::*;

use crate::config::Environment;
use crate::game::settings;
use crate::game::game::*;
use crate::game::fog::*;
use crate::game::events;
use crate::game::settings::PlayerSettings;
use crate::map::wrapping_map::*;
use crate::map::direction::*;
use crate::map::point::*;
use crate::details::*;
use crate::terrain::terrain::Terrain;
use crate::units::hero::Hero;
use crate::units::unit::Unit;

#[derive(Clone, PartialEq)]
pub struct Map<D>
where D: Direction
{
    environment: Environment,
    wrapping_logic: WrappingMap<D>,
    terrain: HashMap<Point, Terrain>,
    units: HashMap<Point, Unit<D>>,
    details: HashMap<Point, Vec<Detail<D>>>,
}

impl<D> Map<D>
where D: Direction
{
    pub fn new(wrapping_logic: WrappingMap<D>, environment: &Environment) -> Self {
        let mut terrain = HashMap::new();
        for p in wrapping_logic.pointmap().get_valid_points() {
            terrain.insert(p, environment.default_terrain());
        }
        Map {
            environment: environment.clone(),
            wrapping_logic,
            terrain,
            units: HashMap::new(),
            details: HashMap::new(),
        }
    }

    /*pub fn odd_if_hex(&self) -> bool {
        self.wrapping_logic.pointmap().odd_if_hex()
    }*/

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

    pub fn is_point_valid(&self, point: Point) -> bool {
        self.wrapping_logic.pointmap().is_point_valid(point)
    }

    /**
     * checks the pipe at dp.point for whether it can be entered by dp.direction and if true, returns the position of the next pipe tile
     * returns None if no pipe is at the given location, for example because the previous pipe tile was an exit
     */
    fn next_pipe_tile(&self, dp: &OrientedPoint<D>) -> Option<OrientedPoint<D>> {
        for det in self.details.get(&dp.point)? {
            match det {
                Detail::Pipe(connection) => {
                    if let Some(d) = connection.transform_direction(dp.direction) {
                        return self.wrapping_logic.get_neighbor(dp.point, d)
                    }
                }
                _ => (),
            }
        }
        None
    }

    pub fn get_direction(&self, from: Point, to: Point) -> Option<D> {
        for d in D::list() {
            if let Some(dp) = self.get_neighbor(from, d) {
                if dp.point == to {
                    return Some(d);
                }
            }
        }
        None
    }

    pub fn get_neighbor(&self, p: Point, d: D) -> Option<OrientedPoint<D>> {
        if let Some(n) = self.wrapping_logic.get_neighbor(p, d) {
            for det in self.get_details(n.point) {
                match det {
                    Detail::Pipe(pipe_state) => {
                        if pipe_state.transform_direction(n.direction).is_some() {
                            let mut dp = n.clone();
                            while let Some(next) = self.next_pipe_tile(&dp) {
                                dp = next;
                                if dp.point == n.point {
                                    // infinite loop, abort
                                    return None;
                                }
                            }
                            return Some(dp);
                        } else {
                            break;
                        }
                    }
                    _ => (),
                }
            }
            Some(n)
        } else {
            None
        }
    }

    pub fn get_neighbors(&self, p: Point, mode: NeighborMode) -> Vec<OrientedPoint<D>> {
        let mut result = vec![];
        for d in D::list() {
            match mode {
                NeighborMode::Direct => {
                    if let Some(neighbor) = self.wrapping_logic.get_neighbor(p, d) {
                        result.push(neighbor);
                    }
                }
                NeighborMode::FollowPipes => {
                    if let Some(neighbor) = self.get_neighbor(p, d) {
                        result.push(neighbor);
                    }
                }
            }
        }
        result
    }

    // the result includes start, the OrientedPoints point towards the next point
    // the result may be shorter than the requested length if not enough points could be found
    pub fn get_line(&self, start: Point, d: D, length: usize, mode: NeighborMode) -> Vec<OrientedPoint<D>> {
        let mut result = vec![OrientedPoint::new(start, false, d)];
        while result.len() < length {
            let current = result.get(result.len() - 1).unwrap();
            let next = match mode {
                NeighborMode::Direct => self.wrapping_logic.get_neighbor(current.point, current.direction),
                NeighborMode::FollowPipes => self.get_neighbor(current.point, current.direction),
            };
            if let Some(mut dp) = next {
                dp.mirrored = dp.mirrored != current.mirrored;
                result.push(dp);
            } else {
                break;
            }
        }
        result
    }
    
    pub fn width_search<F: FnMut(Point) -> bool>(&self, start: Point, mut f: F) -> HashSet<Point> {
        let mut result = HashSet::new();
        let mut to_check = HashSet::new();
        to_check.insert(start);
        while to_check.len() > 0 {
            let mut next = HashSet::new();
            for p in to_check {
                if f(p) {
                    result.insert(p);
                    for p in self.get_neighbors(p, NeighborMode::Direct) {
                        if !result.contains(&p.point) {
                            next.insert(p.point);
                        }
                    }
                }
            }
            to_check = next;
        }
        result
    }

    pub fn get_terrain(&self, p: Point) -> Option<&Terrain> {
        self.terrain.get(&p)
    }
    pub fn get_terrain_mut(&mut self, p: Point) -> Option<&mut Terrain> {
        self.terrain.get_mut(&p)
    }
    pub fn set_terrain(&mut self, p: Point, t: Terrain) {
        // TODO: return a Result<(), ?>
        if self.is_point_valid(p) {
            self.terrain.insert(p, t);
        }
    }

    pub fn get_unit(&self, p: Point) -> Option<&Unit<D>> {
        self.units.get(&p)
    }
    pub fn get_unit_mut(&mut self, p: Point) -> Option<&mut Unit<D>> {
        self.units.get_mut(&p)
    }
    pub fn set_unit(&mut self, p: Point, unit: Option<Unit<D>>) -> Option<Unit<D>> {
        // TODO: return a Result<(), ?>, returning an error if the point is invalid
        if let Some(unit) = unit {
            if self.is_point_valid(p) {
                self.units.insert(p, unit)
            } else {
                None
            }
        } else {
            self.units.remove(&p)
        }
    }

    pub fn get_details(&self, p: Point) -> Vec<Detail<D>> {
        self.details.get(&p).and_then(|v| Some(v.to_vec())).unwrap_or(Vec::new())
    }
    pub fn set_details(&mut self, p: Point, value: Vec<Detail<D>>) {
        if self.is_point_valid(p) {
            let value = Detail::correct_stack(value, &self.environment);
            if value.len() > 0 {
                self.details.insert(p, value.try_into().unwrap());
            } else {
                self.details.remove(&p);
            }
        }
    }
    pub fn add_detail(&mut self, p: Point, value: Detail<D>) {
        let mut list = self.get_details(p);
        list.push(value);
        self.set_details(p, list);
    }
    pub fn insert_detail(&mut self, p: Point, index: usize, value: Detail<D>) {
        let mut list = self.get_details(p);
        if index <= list.len() {
            list.insert(index, value);
            self.set_details(p, list);
        }
    }
    pub fn remove_detail(&mut self, p: Point, index: usize) -> Option<Detail<D>> {
        if let Some(list) = self.details.get_mut(&p) {
            if list.len() > index {
                return Some(list.remove(index));
            }
        }
        None
    }
    
    // returns a random DroneId that isn't in use yet
    pub fn new_drone_id(&self, rng: f32) -> u16 {
        let mut existing_ids = HashSet::new();
        for unit in self.units.values() {
            if let Some(id) = unit.get_drone_station_id().or(unit.get_drone_id()) {
                existing_ids.insert(id);
            }
        }
        /*for details in self.details.values() {
            for det in details {
                match det {
                    Detail::Skull(_, unit) => {
                        if let Some(id) = unit.get_drone_station_id().or(unit.get_drone_id()) {
                            existing_ids.insert(id);
                        }
                    }
                    _ => ()
                }
            }
        }*/
        let mut drone_id = (u16::MAX as f32 * rng) as u16;
        while existing_ids.contains(&drone_id) {
            drone_id = (drone_id + 1) % u16::MAX as u16;
        }
        drone_id
    }

    pub fn range_in_layers(&self, center: Point, range: usize) -> Vec<HashSet<Point>> {
        if range == 0 {
            return Vec::new();
        }
        let mut result = Vec::new();
        let mut layer: HashSet<(Point, D, Option<D>)> = HashSet::new();
        for dp in self.get_neighbors(center, NeighborMode::FollowPipes) {
            layer.insert((dp.point, dp.direction, None));
        }
        for _ in 1..range {
            let previous_layer = layer;
            layer = HashSet::new();
            let mut result_layer = HashSet::new();
            for (p, dir, dir_change) in previous_layer {
                result_layer.insert(p);
                if let Some(dp) = self.get_neighbor(p, dir) {
                    let dir_change = match (dp.mirrored, dir_change) {
                        (_, None) => None,
                        (true, Some(angle)) => Some(angle.mirror_vertically()),
                        (false, Some(angle)) => Some(angle),
                    };
                    layer.insert((dp.point, dp.direction, dir_change));
                }
                let mut dir_changes = vec![];
                if let Some(dir_change) = dir_change {
                    // if we already have 2 directions, only those 2 directions can find new points
                    dir_changes.push(dir_change);
                } else {
                    // since only one direction has been used so far, try both directions that are directly neighboring
                    let d = *D::list().last().unwrap();
                    dir_changes.push(d.mirror_vertically());
                    dir_changes.push(d);
                }
                for dir_change in dir_changes {
                    if let Some(dp) = self.get_neighbor(p, dir.rotate_by(dir_change)) {
                        let mut dir_change = dir_change;
                        if dp.mirrored {
                            dir_change = dir_change.mirror_vertically();
                        }
                        let dir = dp.direction.rotate_by(dir_change.mirror_vertically());
                        layer.insert((dp.point, dir, Some(dir_change)));
                    }
                }
            }
            result.push(result_layer);
        }
        let mut result_layer = HashSet::new();
        for (p, _, _) in layer {
            result_layer.insert(p);
        }
        result.push(result_layer);
        result
    }

    pub fn mercenary_influence_at(&self, point: Point, owner_id: i8) -> Vec<(Point, Hero)> {
        let mut result = vec![];
        for p in self.all_points() {
            if let Some(unit) = self.get_unit(p) {
                let hero = unit.get_hero();
                if unit.get_owner_id() == owner_id && hero.in_range(self, p, point) {
                    result.push((p, hero));
                }
            }
        }
        result
    }

    /*
     * maybe in the future
     *
    pub fn available_player_actions(&self, point: Point, player: Owner) -> bool {
        if let Some(unit) = self.get_unit(point) {
            return unit.can_act(player);
        }
        for det in self.details.get(&point).unwrap_or(&vec![]) {
            if det.can_act(player) {
            }
        }
    }*/

    pub fn fix_errors_details(&self) -> HashMap<Point, Vec<Detail<D>>> {
        let mut corrected = HashMap::new();
        for p in self.all_points() {
            let stack = Detail::correct_stack(self.get_details(p), &self.environment);
            if *self.details.get(&p).unwrap_or(&stack) != stack {
                corrected.insert(p, stack);
            }
        }
        // fix_self can depend on surrounding details
        // so Detail::correct_stack which can remove details has to be in a separate loop before this one
        for p in self.all_points() {
            let stack = corrected.remove(&p).unwrap_or(self.get_details(p))
            .into_iter()
            .map(|mut det| {
                det.fix_self(self, p);
                det
            })
            .collect();
            if *self.details.get(&p).unwrap_or(&stack) != stack {
                corrected.insert(p, stack);
            }
        }
        corrected
    }
    
    pub fn get_income_factor(&self, owner_id: i8) -> i32 {
        // income from properties
        let mut income_factor = 0;
        for p in self.all_points() {
            let t = self.get_terrain(p).unwrap();
            if t.get_owner_id() == owner_id {
                income_factor += t.income_factor();
            }
        }
        income_factor
    }
    
    pub fn get_viable_player_ids(&self) -> Vec<u8> {
        let mut owners = HashSet::new();
        for p in self.all_points() {
            if let Some(unit) = self.get_unit(p) {
                if unit.get_owner_id() >= 0 {
                    owners.insert(unit.get_owner_id() as u8);
                }
            }
            let t = self.get_terrain(p).unwrap();
            if t.get_owner_id() >= 0 && t.can_build() {
                owners.insert(t.get_owner_id() as u8);
            }
            for detail in self.get_details(p) {
                match detail {
                    Detail::Bubble(owner, _) => {
                        if owner >= 0 {
                            owners.insert(owner as u8);
                        }
                    }
                    _ => {}
                }
            }
        }
        let mut owners: Vec<u8> = owners.into_iter().collect();
        owners.sort();
        owners
    }

    pub fn get_field_data(&self, p: Point) -> FieldData<D> {
        FieldData {
            terrain: self.terrain.get(&p).unwrap().clone(),
            details: self.details.get(&p).cloned().unwrap_or(Vec::new()),
            unit: self.units.get(&p).cloned(),
        }
    }
    pub fn export_field(&self, zipper: &mut Zipper, p: Point, fog_intensity: FogIntensity) {
        let fd = self.get_field_data(p).fog_replacement(fog_intensity);
        fd.export(zipper);
    }

    pub fn zip(&self, zipper: &mut Zipper, fog: Option<&HashMap<Point, FogIntensity>>) {
        self.wrapping_logic.export(zipper);
        for p in self.all_points() {
            self.export_field(zipper, p, fog.and_then(|fog| fog.get(&p).cloned()).unwrap_or(FogIntensity::TrueSight));
        }
    }

    pub fn import_from_unzipper(environment: &Environment, unzipper: &mut Unzipper) -> Result<Self, ZipperError> {
        let wrapping_logic = WrappingMap::import(unzipper)?;
        let mut terrain = HashMap::new();
        let mut units = HashMap::new();
        let mut details = HashMap::new();
        for p in wrapping_logic.pointmap().get_valid_points() {
            terrain.insert(p, Terrain::import(unzipper)?);
            let det = LVec::<Detail<D>, MAX_STACK_SIZE>::import(unzipper)?;
            if det.len() > 0 {
                details.insert(p, det);
            }
            // could be more memory-efficient by returning Option<Unit> from import and removing this read_bool
            if unzipper.read_bool()? {
                units.insert(p, Unit::import(environment, unzipper, None)?);
            }
        }
        Ok(Self {
            wrapping_logic,
            terrain,
            units,
            details,
        })
    }
}

pub enum MapType {
    Square(Map<Direction4>),
    Hex(Map<Direction6>),
}

pub fn import_map(environment: &Environment, bytes: Vec<u8>) -> Result<MapType, ZipperError> {
    let mut unzipper = Unzipper::new(bytes);
    if unzipper.read_bool()? {
        Ok(MapType::Hex(Map::import_from_unzipper(environment, &mut unzipper)?))
    } else {
        Ok(MapType::Square(Map::import_from_unzipper(environment, &mut unzipper)?))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FieldData<D: Direction> {
    pub terrain: Terrain,
    pub details: Vec<Detail<D>>,
    pub unit: Option<Unit<D>>,
}
impl<D: Direction> FieldData<D> {
    pub fn fog_replacement(self, intensity: FogIntensity) -> Self {
        let details: Vec<_> = self.details.into_iter()
        .filter_map(|d| d.fog_replacement(intensity))
        .collect();
        Self {
            unit: self.unit.and_then(|unit| unit.fog_replacement(&self.terrain, intensity)),
            details: details.try_into().expect("Detail list shouldn't become longer after filtering"),
            terrain: self.terrain.fog_replacement(intensity),
        }
    }
}

impl<D: Direction> interfaces::map_interface::MapInterface for Map<D> {
    type Terrain = Terrain;
    type Detail = Detail<D>;
    type Unit = Unit<D>;
    type GameSettings = settings::GameSettings;
    type Game = Game<D>;

    fn export(&self) -> Vec<u8> {
        let mut zipper = Zipper::new();
        zipper.write_bool(D::is_hex());
        self.zip(&mut zipper, None);
        zipper.finish()
    }

    fn settings(&self) -> Result<Self::GameSettings, interfaces::map_interface::NotPlayable> {
        let owners = self.get_viable_player_ids();
        if owners.len() < 2 {
            return Err(interfaces::map_interface::NotPlayable::TooFewPlayers);
        }
        let players:Vec<PlayerSettings> = owners.into_iter()
            .map(|owner| PlayerSettings::new(owner))
            .collect();
        Ok(settings::GameSettings {
            name: "".to_string(),
            fog_mode: FogMode::Constant(FogSetting::Light(0)),
            players: players.try_into().unwrap(),
        })
    }

    fn game_server<R: 'static + Fn() -> f32>(self, settings: &settings::GameSettings, random: R) -> (Game<D>, Events<Game<D>>) {
        Game::new_server(self, settings, random)
    }
    fn game_client(self, settings: &settings::GameSettings, events: &Vec<events::Event<D>>) -> Game<D> {
        Game::new_client(self, settings, events)
    }

}

#[derive(Debug, Clone, Copy)]
pub enum NeighborMode {
    Direct,
    FollowPipes,
}
