use std::collections::{HashMap, HashSet};

use interfaces::game_interface::Events;
use zipper::*;
use zipper::zipper_derive::*;

use crate::details;
use crate::game::settings;
use crate::game::game::*;
use crate::game::events;
use crate::game::settings::PlayerSettings;
use crate::map::wrapping_map::*;
use crate::map::direction::*;
use crate::map::point::*;
use crate::player::*;
use crate::terrain::*;
use crate::units::*;
use crate::details::*;
use crate::units::mercenary::MaybeMercenary;
use crate::units::mercenary::Mercenaries;
use crate::units::movement::MovementType;
use crate::units::movement::PathStep;
use crate::units::normal_units::DroneId;

#[derive(Debug, Clone, PartialEq)]
pub struct Map<D>
where D: Direction
{
    wrapping_logic: WrappingMap<D>,
    terrain: HashMap<Point, Terrain<D>>,
    units: HashMap<Point, UnitType<D>>,
    details: HashMap<Point, LVec<Detail, MAX_STACK_SIZE>>,
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
        match self.terrain.get(&dp.point) {
            Some(Terrain::Pipe(pipe_state)) => {
                if pipe_state.connects_towards(dp.direction.opposite_direction()) || pipe_state.enterable_from(dp.direction) {
                    self.wrapping_logic.get_neighbor(dp.point, pipe_state.next_dir(dp.direction))
                } else {
                    None
                }
            }
            _ => None,
        }
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
            match self.terrain.get(&n.point) {
                Some(Terrain::Pipe(pipe_state)) => {
                    if pipe_state.enterable_from(n.direction) || pipe_state.connects_towards(n.direction.opposite_direction()) {
                        let mut dp = n.clone();
                        while let Some(next) = self.next_pipe_tile(&dp) {
                            dp = next;
                            if dp.point == n.point {
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
    
    pub fn get_unit_movement_neighbors(&self, p: Point, _mov: MovementType) -> Vec<(OrientedPoint<D>, PathStep<D>)> {
        let mut result = vec![];
        for d in D::list() {
            if let Some(neighbor) = self.get_neighbor(p, d) {
                if self.terrain.get(&p) == Some(&Terrain::Fountain) {
                    if let Some(neighbor) = self.get_neighbor(neighbor.point, neighbor.direction) {
                        result.push((neighbor, PathStep::Jump(d)));
                    }
                }
                result.push((neighbor, PathStep::Dir(d)));
            }
        }
        result
    }
    
    pub fn get_terrain(&self, p: Point) -> Option<&Terrain<D>> {
        self.terrain.get(&p)
    }
    pub fn get_terrain_mut(&mut self, p: Point) -> Option<&mut Terrain<D>> {
        self.terrain.get_mut(&p)
    }
    pub fn set_terrain(&mut self, p: Point, t: Terrain<D>) {
        // TODO: return a Result<(), ?>
        if self.is_point_valid(p) {
            self.terrain.insert(p, t);
        }
    }

    pub fn get_unit(&self, p: Point) -> Option<&UnitType<D>> {
        self.units.get(&p)
    }
    pub fn get_unit_mut(&mut self, p: Point) -> Option<&mut UnitType<D>> {
        self.units.get_mut(&p)
    }
    pub fn set_unit(&mut self, p: Point, unit: Option<UnitType<D>>) -> Option<UnitType<D>> {
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

    pub fn get_details(&self, p: Point) -> Vec<Detail> {
        self.details.get(&p).and_then(|v| Some(v.clone().into())).unwrap_or(vec![])
    }
    pub fn set_details(&mut self, p: Point, value: Vec<Detail>) {
        if self.is_point_valid(p) {
            let value: Vec<Detail> = Detail::correct_stack(value);
            if value.len() > 0 {
                self.details.insert(p, value.try_into().unwrap());
            } else {
                self.details.remove(&p);
            }
        }
    }
    pub fn add_detail(&mut self, p: Point, value: Detail) {
        let mut list: Vec<Detail> = self.get_details(p).into_iter().map(|f| f.clone()).collect();
        list.push(value);
        self.set_details(p, list);
    }
    pub fn insert_detail(&mut self, p: Point, index: usize, value: Detail) {
        let mut list: Vec<Detail> = self.get_details(p).into_iter().map(|f| f.clone()).collect();
        if index <= list.len() {
            list.insert(index, value);
            self.set_details(p, list);
        }
    }
    pub fn remove_detail(&mut self, p: Point, index: usize) -> Option<Detail> {
        if let Some(list) = self.details.get_mut(&p) {
            return list.remove(index).ok();
        } else {
            None
        }
    }
    
    // returns a random DroneId that isn't in use yet
    pub fn new_drone_id(&self, rng: f32) -> DroneId {
        let mut existing_ids = HashSet::new();
        for unit in self.units.values() {
            unit.insert_drone_ids(&mut existing_ids);
        }
        for details in self.details.values() {
            for det in details {
                match det {
                    Detail::Skull(_, unit) => {
                        unit.insert_drone_ids(&mut existing_ids);
                    }
                    _ => ()
                }
            }
        }
        let mut drone_id = (DroneId::MAX as f32 * rng) as u16;
        while existing_ids.contains(&drone_id) {
            drone_id = (drone_id + 1) % DroneId::MAX;
        }
        DroneId::new(drone_id)
    }

    pub fn range_in_layers(&self, center: Point, range: usize) -> Vec<HashSet<(Point, D, Option<D>)>> {
        let mut layers: Vec<HashSet<(Point, D, Option<D>)>> = vec![];
        let mut layer = HashSet::new();
        for dp in self.get_neighbors(center, NeighborMode::FollowPipes) {
            layer.insert((dp.point.clone(), dp.direction.clone(), None));
        }
        layers.push(layer);
        while layers.len() < range {
            let mut layer = HashSet::new();
            for (p, dir, dir_change) in layers.last().unwrap() {
                if let Some(dp) = self.get_neighbor(*p, *dir) {
                    let dir_change = match (dp.mirrored, dir_change) {
                        (_, None) => None,
                        (true, Some(angle)) => Some(angle.mirror_vertically()),
                        (false, Some(angle)) => Some(angle.clone()),
                    };
                    layer.insert((dp.point.clone(), dp.direction.clone(), dir_change));
                }
                let mut dir_changes = vec![];
                if let Some(dir_change) = dir_change {
                    // if we already have 2 directions, only those 2 directions can find new points
                    dir_changes.push(dir_change.clone());
                } else {
                    // since only one direction has been used so far, try both directions that are directly neighboring
                    let d = *D::list().last().unwrap();
                    dir_changes.push(d.mirror_vertically());
                    dir_changes.push(d);
                }
                for dir_change in dir_changes {
                    if let Some(dp) = self.get_neighbor(*p, dir.rotate_by(dir_change)) {
                        let mut dir_change = dir_change.clone();
                        if dp.mirrored {
                            dir_change = dir_change.mirror_vertically();
                        }
                        let dir = dp.direction.rotate_by(dir_change.mirror_vertically());
                        layer.insert((dp.point.clone(), dir, Some(dir_change)));
                    }
                }
            }
            layers.push(layer);
        }
        layers
    }

    pub fn mercenary_influence_at(&self, point: Point, owner: Option<Owner>) -> Vec<(Point, &Mercenaries)> {
        let mut result = vec![];
        for p in self.all_points() {
            if let Some(UnitType::Normal(unit)) = self.get_unit(p) {
                if let MaybeMercenary::Some{mercenary, ..} = &unit.data.mercenary {
                    if (owner.is_none() || owner == Some(unit.owner)) && mercenary.in_range(self, p, point) {
                        result.push((p.clone(), mercenary));
                    }
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

    pub fn validate_terrain(&mut self) -> Vec<(Point, Terrain<D>)> {
        let mut corrected = vec![];
        for p in self.all_points() {
            match self.get_terrain(p).unwrap() {
                Terrain::Pipe(state) => {
                    let mut is_valid = true;
                    let mut valid_dir = None;
                    for d in state.connections() {
                        if let Some(dp) = self.wrapping_logic.get_neighbor(p, d) {
                            match self.get_terrain(dp.point).unwrap() {
                                Terrain::Pipe(state) => {
                                    if state.connects_towards(dp.direction.opposite_direction()) {
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
    
    pub fn get_income_factor(&self, owner_id: Owner) -> isize {
        // income from properties
        let mut income_factor = 0;
        for p in self.all_points() {
            match self.get_terrain(p) {
                Some(Terrain::Realty(realty, owner, _)) => {
                    if *owner == Some(owner_id) {
                        income_factor += realty.income_factor() as isize;
                    }
                }
                _ => {}
            }
        }
        income_factor
    }
    
    pub fn get_viable_player_ids(&self) -> Vec<Owner> {
        let mut owners = HashSet::new();
        for p in self.all_points() {
            if let Some(unit) = self.get_unit(p) {
                if let Some(owner) = unit.get_owner() {
                    owners.insert(owner);
                }
            }
            if let Some(Terrain::Realty(realty, owner, _)) = self.get_terrain(p) {
                if let Some(owner) = owner {
                    if realty.can_build() {
                        owners.insert(*owner);
                    }
                }
            }
            for detail in self.get_details(p) {
                match detail {
                    Detail::FactoryBubble(owner) => {
                        owners.insert(owner);
                    }
                    _ => {}
                }
            }
        }
        let mut owners: Vec<Owner> = owners.into_iter().collect();
        owners.sort();
        owners
    }

    pub fn get_field_data(&self, p: Point) -> FieldData<D> {
        FieldData {
            terrain: self.terrain.get(&p).unwrap().clone(),
            details: self.details.get(&p).cloned().unwrap_or(LVec::new()),
            unit: self.units.get(&p).cloned(),
        }
    }
    pub fn export_field(&self, zipper: &mut Zipper, p: Point, vision: Option<&Vision>) {
        let mut fd = self.get_field_data(p);
        fd = match vision {
            Some(Vision::TrueSight) => fd,
            Some(Vision::Normal) => fd.stealth_replacement(),
            None => fd.fog_replacement(),
        };
        fd.export(zipper);
    }

    pub fn zip(&self, zipper: &mut Zipper, vision: Option<&HashMap<Point, Vision>>) {
        self.wrapping_logic.export(zipper);
        for p in self.all_points() {
            let vision = if let Some(vision) = vision {
                vision.get(&p)
            } else {
                Some(&Vision::TrueSight)
            };
            self.export_field(zipper, p, vision);
        }
    }

    pub fn import_from_unzipper(unzipper: &mut Unzipper) -> Result<Self, ZipperError> {
        let wrapping_logic = WrappingMap::import(unzipper)?;
        let mut terrain = HashMap::new();
        let mut units = HashMap::new();
        let mut details = HashMap::new();
        for p in wrapping_logic.pointmap().get_valid_points() {
            terrain.insert(p.clone(), Terrain::import(unzipper)?);
            let det = LVec::<Detail, MAX_STACK_SIZE>::import(unzipper)?;
            if det.len() > 0 {
                details.insert(p.clone(), det);
            }
            if let Some(unit) = Option::<UnitType<D>>::import(unzipper)? {
                units.insert(p.clone(), unit);
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

pub fn import_map(bytes: Vec<u8>) -> Result<MapType, ZipperError> {
    let mut unzipper = Unzipper::new(bytes);
    if unzipper.read_bool()? {
        Ok(MapType::Hex(Map::import_from_unzipper(&mut unzipper)?))
    } else {
        Ok(MapType::Square(Map::import_from_unzipper(&mut unzipper)?))
    }
}

#[derive(Debug, Clone, PartialEq, Zippable)]
pub struct FieldData<D: Direction> {
    pub terrain: Terrain::<D>,
    pub details: LVec::<Detail, {details::MAX_STACK_SIZE}>,
    pub unit: Option::<UnitType<D>>,
}
impl<D: Direction> FieldData<D> {
    pub fn fog_replacement(&self) -> Self {
        Self {
            terrain: self.terrain.fog_replacement(),
            details: details_fog_replacement(&self.details),
            unit: self.unit.clone().and_then(|unit| unit.fog_replacement())
        }
    }
    pub fn stealth_replacement(&self) -> Self {
        let unit = if let Some(unit) = self.unit.as_ref() {
            if unit.fog_replacement().is_none() && self.terrain.hides_unit(unit) {
                None
            } else {
                unit.stealth_replacement()
            }
        } else {
            None
        };
        Self {
            terrain: self.terrain.clone(),
            details: self.details.clone(),
            unit
        }
    }
}

impl<D: Direction> interfaces::map_interface::MapInterface for Map<D> {
    type Terrain = Terrain<D>;
    type Detail = Detail;
    type Unit = UnitType<D>;
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
            fog_mode: FogMode::DarkRegular(0.try_into().unwrap(), (players.len() as u8 * 2).try_into().unwrap(), (players.len() as u8 * 2 + 1).try_into().unwrap()),
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
