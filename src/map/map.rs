use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fmt::{Debug, Display};
use std::sync::Arc;

use interfaces::*;
use semver::Version;
use zipper::*;
use zipper_derive::Zippable;

use crate::config::config::Config;
use crate::config::environment::Environment;
use crate::game::game_view::GameView;
use crate::game::settings::{self, GameConfig, GameSettings, PlayerConfig, PlayerSelectedOptions, PlayerSettingError};
use crate::game::game::*;
use crate::game::fog::*;
use crate::map::wrapping_map::*;
use crate::map::direction::*;
use crate::map::point::*;
use crate::player::Player;
use crate::{details::*, VERSION};
use crate::details;
use crate::terrain::terrain::Terrain;
use crate::units::unit::Unit;

use super::map_view::MapView;
use super::point_map::MapSize;

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

impl<D: Direction> Debug for Map<D> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{:?}", self.environment)?;
        writeln!(f, "{:?}", self.wrapping_logic)?;
        for p in self.all_points() {
            write!(f, "{},{}: {:?}", p.x, p.y, self.terrain.get(&p).unwrap())?;
            if let Some(details) = self.details.get(&p) {
                write!(f, " +")?;
                for detail in details {
                    write!(f, " {detail:?}")?;
                }
            }
            if let Some(unit) = self.units.get(&p) {
                write!(f, " - {unit:?}")?;
            }
            writeln!(f, "")?;
        }
        Ok(())
    }
}

impl<D: Direction> Map<D> {
    pub fn new(wrapping_logic: WrappingMap<D>, config: &Arc<Config>) -> Self {
        let environment = Environment {
            config: config.clone(),
            map_size: wrapping_logic.pointmap().size(),
            settings: None,
        };
        let mut terrain = HashMap::new();
        for p in wrapping_logic.pointmap().get_valid_points() {
            terrain.insert(p, environment.default_terrain());
        }
        Map {
            environment,
            wrapping_logic,
            terrain,
            units: HashMap::new(),
            details: HashMap::new(),
        }
    }

    pub fn new2(wrapping_logic: WrappingMap<D>, environment: &Environment) -> Self {
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

    pub fn odd_if_hex(&self) -> bool {
        self.wrapping_logic.pointmap().odd_if_hex()
    }

    pub fn width(&self) -> u8 {
        self.wrapping_logic.pointmap().width()
    }

    pub fn height(&self) -> u8 {
        self.wrapping_logic.pointmap().height()
    }

    pub fn is_point_valid(&self, point: Point) -> bool {
        self.wrapping_logic.pointmap().is_point_valid(point)
    }

    pub fn get_direction(&self, from: Point, to: Point) -> Option<D> {
        for d in D::list() {
            if let Some((p, _)) = self.get_neighbor(from, d) {
                if p == to {
                    return Some(d);
                }
            }
        }
        None
    }

    pub fn get_terrain_mut(&mut self, p: Point) -> Option<&mut Terrain> {
        self.terrain.get_mut(&p)
    }
    pub fn set_terrain(&mut self, p: Point, t: Terrain) {
        if self.is_point_valid(p) {
            self.terrain.insert(p, t);
        }
    }

    pub fn get_unit_mut(&mut self, p: Point) -> Option<&mut Unit<D>> {
        self.units.get_mut(&p)
    }
    pub fn set_unit(&mut self, p: Point, unit: Option<Unit<D>>) -> Option<Unit<D>> {
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
        let mut list = self.get_details(p).to_vec();
        list.push(value);
        self.set_details(p, list);
    }
    pub fn insert_detail(&mut self, p: Point, index: usize, value: Detail<D>) {
        let mut list = self.get_details(p).to_vec();
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
                if let Some((point, distortion)) = self.get_neighbor(p, dir) {
                    let dir_change = match (distortion.is_mirrored(), dir_change) {
                        (_, None) => None,
                        (true, Some(angle)) => Some(angle.mirror_vertically()),
                        (false, Some(angle)) => Some(angle),
                    };
                    layer.insert((point, distortion.update_direction(dir), dir_change));
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
                for mut dir_change in dir_changes {
                    if let Some((point, distortion)) = self.get_neighbor(p, dir.rotate_by(dir_change)) {
                        if distortion.is_mirrored() {
                            dir_change = dir_change.mirror_vertically();
                        }
                        layer.insert((point, distortion.update_direction(dir), Some(dir_change)));
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
            let stack = Detail::correct_stack(self.get_details(p).to_vec(), &self.environment);
            if *self.details.get(&p).unwrap_or(&stack) != stack {
                corrected.insert(p, stack);
            }
        }
        // fix_self can depend on surrounding details
        // so Detail::correct_stack which can remove details has to be in a separate loop before this one
        for p in self.all_points() {
            let stack = corrected.remove(&p).unwrap_or(self.get_details(p).to_vec())
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
            if t.get_owner_id() >= 0 && t.can_build(self, p, &[]) {
                owners.insert(t.get_owner_id() as u8);
            }
            for detail in self.get_details(p) {
                match detail {
                    Detail::Bubble(owner, _) => {
                        if owner.0 >= 0 {
                            owners.insert(owner.0 as u8);
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
            details: self.details.get(&p).cloned().map(|v| v.try_into().unwrap()).unwrap_or(LVec::new()),
            unit: self.units.get(&p).cloned(),
        }
    }

    pub fn import_from_unzipper(unzipper: &mut Unzipper, environment: &mut Environment) -> Result<Self, ZipperError> {
        let wrapping_logic = WrappingMap::unzip(unzipper)?;
        environment.map_size = wrapping_logic.pointmap().size();
        let mut terrain = HashMap::new();
        let mut units = HashMap::new();
        let mut details = HashMap::new();
        for p in wrapping_logic.pointmap().get_valid_points() {
            terrain.insert(p, Terrain::import(unzipper, environment)?);
            let det = LVec::<Detail<D>, MAX_STACK_SIZE>::import(unzipper, environment)?;
            if det.len() > 0 {
                details.insert(p, det.into());
            }
            // could be more memory-efficient by returning Option<Unit> from import and removing this read_bool
            if unzipper.read_bool()? {
                units.insert(p, Unit::unzip(unzipper, environment, None)?);
            }
        }
        Ok(Self {
            environment: environment.clone(),
            wrapping_logic,
            terrain,
            units,
            details,
        })
    }

    pub(crate) fn start_game(&mut self, settings: &Arc<GameSettings>) {
        self.environment.start_game(settings);
        for p in self.all_points() {
            self.terrain.get_mut(&p).unwrap().start_game(settings);
            if let Some(unit) = self.units.get_mut(&p) {
                unit.start_game(settings);
            }
        }
    }

    pub fn settings(&self) -> Result<GameConfig, NotPlayable> {
        let owners = self.get_viable_player_ids();
        if owners.len() < 2 {
            return Err(NotPlayable::TooFewPlayers);
        }
        let players:Vec<PlayerConfig> = owners.into_iter()
            .map(|owner| PlayerConfig::new(owner, &self.environment.config))
            .collect();
        Ok(settings::GameConfig {
            config: self.environment.config.clone(),
            fog_mode: FogMode::Constant(FogSetting::Light(0)),
            players: players.try_into().unwrap(),
        })
    }
}

impl<D: Direction> MapView<D> for Map<D> {
    fn environment(&self) -> &Environment {
        &self.environment
    }

    fn wrapping_logic(&self) -> &WrappingMap<D> {
        &self.wrapping_logic
    }

    fn all_points(&self) -> Vec<Point> {
        self.wrapping_logic.pointmap().get_valid_points()
    }

    fn get_terrain(&self, p: Point) -> Option<&Terrain> {
        self.terrain.get(&p)
    }

    fn get_details(&self, p: Point) -> &[Detail<D>] {
        self.details.get(&p).map(|v| v.as_slice()).unwrap_or(&[])
    }

    fn get_unit(&self, p: Point) -> Option<&Unit<D>> {
        self.units.get(&p)
    }
}

impl<D: Direction> GameView<D> for Map<D> {
    fn get_owning_player(&self, _owner: i8) -> Option<&Player> {
        None
    }

    fn fog_intensity(&self) -> FogIntensity {
        FogIntensity::TrueSight
    }

    fn get_fog_at(&self, _team: ClientPerspective, _position: Point) -> FogIntensity {
        FogIntensity::TrueSight
    }

    fn get_visible_unit(&self, _team: ClientPerspective, p: Point) -> Option<Unit<D>> {
        self.get_unit(p).cloned()
    }
}

pub enum MapType {
    Square(Map<Direction4>),
    Hex(Map<Direction6>),
}

pub fn import_map(config: &Arc<Config>, bytes: Vec<u8>, version: Version) -> Result<MapType, ZipperError> {
    let mut environment = Environment {
        config: config.clone(),
        map_size: MapSize::new(0, 0),
        settings: None,
    };
    let mut unzipper = Unzipper::new(bytes, version);
    if unzipper.read_bool()? {
        Ok(MapType::Hex(Map::import_from_unzipper(&mut unzipper, &mut environment)?))
    } else {
        Ok(MapType::Square(Map::import_from_unzipper(&mut unzipper, &mut environment)?))
    }
}

#[derive(Debug, Clone, PartialEq, Zippable)]
#[zippable(support_ref = Environment)]
pub struct FieldData<D: Direction> {
    pub terrain: Terrain,
    pub details: LVec<Detail<D>, {details::MAX_STACK_SIZE}>,
    pub unit: Option<Unit<D>>,
}

impl<D: Direction> FieldData<D> {
    pub fn fog_replacement(self, game: &Game<D>, pos: Point, intensity: FogIntensity) -> Self {
        let details: Vec<_> = self.details.into_iter()
        .filter_map(|d| d.fog_replacement(intensity))
        .collect();
        Self {
            unit: self.unit.and_then(|unit| unit.fog_replacement(game, pos, intensity)),
            details: details.try_into().expect("Detail list shouldn't become longer after filtering"),
            terrain: self.terrain.fog_replacement(intensity),
        }
    }
}

impl<D: Direction> MapInterface for Map<D> {
    fn export(&self) -> Vec<u8> {
        let mut zipper = Zipper::new();
        zipper.write_bool(D::is_hex());
        self.wrapping_logic.zip(&mut zipper);
        for p in self.all_points() {
            self.get_field_data(p).export(&mut zipper, &self.environment);
        }
        zipper.finish()
    }

    fn width(&self) -> usize {
        self.width() as usize
    }

    fn height(&self) -> usize {
        self.height() as usize
    }

    fn player_count(&self) -> u16 {
        self.get_viable_player_ids().len() as u16
    }

    fn metrics(&self) -> HashMap<String, i32> {
        let mut result = HashMap::new();
        let mut income = 0;
        for t in self.terrain.values() {
            income += t.income_factor();
        }
        result.insert("Income".to_string(), income);
        result
    }

    fn default_settings(&self) -> Result<Box<dyn GameSettingsInterface>, Box<dyn Error>> {
        match self.settings() {
            Ok(s) => Ok(Box::new(s)),
            Err(e) => Err(Box::new(e)),
        }
    }

    fn parse_settings(&self, bytes: Vec<u8>) -> Result<Box<dyn GameSettingsInterface>, Box<dyn Error>> {
        let settings = GameConfig::import(self.environment.config.clone(), bytes)?;
        Ok(Box::new(settings))
    }

    fn game_creator(self: Box<Self>, settings: Vec<u8>, player_settings: Vec<Vec<u8>>) -> Result<Box<dyn GameCreationInterface>, Box<dyn Error>> {
        let settings = GameConfig::import(self.environment.config.clone(), settings)?;
        if player_settings.len() != settings.players.len() {
            return Err(Box::new(PlayerSettingError::PlayerCount(settings.players.len(), player_settings.len())));
        }
        let mut player_selection = Vec::with_capacity(player_settings.len());
        for bytes in player_settings {
            let mut unzipper = Unzipper::new(bytes, Version::parse(VERSION).unwrap());
            player_selection.push(PlayerSelectedOptions::import(&mut unzipper, &settings.config)?);
        }
        Ok(Box::new(GameCreation {
            map: *self,
            settings,
            player_selection,
        }))
    }

    #[cfg(feature = "rendering")]
    fn preview(&self) -> MapPreview {
        let mut base = Vec::new();
        let base_x = if self.odd_if_hex() {
            2
        } else {
            0
        };
        for (p, terrain) in &self.terrain {
            let (x, y) = D::T::between(&GlobalPoint::ZERO, &GlobalPoint::new(p.x as i16, p.y as i16), self.odd_if_hex()).screen_coordinates();
            let pos = PreviewPos {
                x: base_x + (x * 4.).round() as u16,
                y: (y * 4.).round() as u16,
            };
            for (shape, color) in self.environment.config.terrain_preview(terrain.typ(), terrain.get_owner_id()) {
                base.push(PreviewTile {
                    pos,
                    shape,
                    color,
                });
            }
        }
        MapPreview {
            base,
            frames: Vec::new(),
        }
    }
}

#[derive(Debug)]
pub enum NotPlayable {
    TooFewPlayers,
}

impl Display for NotPlayable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooFewPlayers => write!(f, "This map has less than 2 Players"),
        }
    }
}

impl Error for NotPlayable {}

pub struct GameCreation<D: Direction> {
    pub map: Map<D>,
    pub settings: settings::GameConfig,
    pub player_selection: Vec<settings::PlayerSelectedOptions>,
}

impl<D: Direction> GameCreationInterface for GameCreation<D> {
    fn server(self: Box<Self>, random: Box<dyn 'static + Fn() -> f32>) -> (Box<dyn GameInterface>, Events) {
        let settings = self.settings.build(&self.player_selection, &random);
        let (server, events) = Game::new_server(self.map, settings, random);
        let events = events.export(server.environment());
        (server, events)
    }

    fn server_and_client(self: Box<Self>, client_perspective: ClientPerspective, random: Box<dyn 'static + Fn() -> f32>) -> (Box<dyn GameInterface>, Box<dyn GameInterface>, Events) {
        let settings = self.settings.build(&self.player_selection, &random);
        let (server, events) = Game::new_server(self.map.clone(), settings.clone(), random);
        let client = Game::new_client(self.map, settings, events.get(&client_perspective.into()).unwrap_or(&[]));
        let events = events.export(server.environment());
        (server, client, events)
    }
}

#[derive(Debug, Clone, Copy)]
pub enum NeighborMode {
    Direct,
    FollowPipes,
}
