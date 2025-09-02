use num_rational::Rational32;
use rustc_hash::{FxHashMap as HashMap, FxHashSet as HashSet};
use std::error::Error;
use std::fmt::{Debug, Display};

use interfaces::*;
use semver::Version;
use zipper::*;
use zipper_derive::Zippable;

use crate::config::config::Config;
use crate::config::environment::Environment;
use crate::game::game_view::GameView;
use crate::game::rhai_board::SharedGameView;
use crate::game::settings::{self, GameConfig, GameSettings, PlayerConfig, PlayerSelectedOptions, PlayerSettingError};
use crate::game::game::*;
use crate::game::fog::*;
use crate::handle::Handle;
use crate::map::wrapping_map::*;
use crate::map::direction::*;
use crate::map::point::*;
use uniform_smart_pointer::{ReadGuard, Urc};
use crate::player::Player;
use crate::tags::TagBag;
use crate::units::hero::HeroMap;
use crate::VERSION;
use crate::tokens;
use crate::terrain::terrain::Terrain;
use crate::tokens::token::Token;
use crate::units::unit::Unit;

use super::point_map::MapSize;
use super::pipe::PipeState;

#[derive(Clone, PartialEq)]
pub struct Map<D>
where D: Direction
{
    environment: Environment,
    wrapping_logic: WrappingMap<D>,
    tags: TagBag<D>,
    pipes: HashMap<Point, Vec<PipeState<D>>>,
    terrain: HashMap<Point, Terrain<D>>,
    units: HashMap<Point, Unit<D>>,
    tokens: HashMap<Point, Vec<Token<D>>>,
}

impl<D: Direction> Debug for Map<D> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{:?}", self.environment)?;
        writeln!(f, "{:?}", self.wrapping_logic)?;
        self.tags.debug(f, &self.environment)?;
        for p in self.all_points() {
            write!(f, "{},{}: {:?}", p.x, p.y, self.terrain.get(&p).unwrap())?;
            if let Some(tokens) = self.tokens.get(&p) {
                write!(f, " +")?;
                for token in tokens {
                    write!(f, " {token:?}")?;
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
    pub fn new_handled(wrapping_logic: WrappingMap<D>, config: &Urc<Config>) -> Handle<Self> {
        Handle::new(Self::new(wrapping_logic, config))
    }

    pub fn new(wrapping_logic: WrappingMap<D>, config: &Urc<Config>) -> Self {
        let environment = Environment::new_map(config.clone(), wrapping_logic.pointmap().size());
        let mut terrain = HashMap::default();
        for p in wrapping_logic.pointmap().get_valid_points() {
            terrain.insert(p, environment.default_terrain());
        }
        Map {
            environment,
            wrapping_logic,
            tags: TagBag::new(),
            pipes: HashMap::default(),
            terrain,
            units: HashMap::default(),
            tokens: HashMap::default(),
        }
    }

    pub fn new2(wrapping_logic: WrappingMap<D>, environment: &Environment) -> Self {
        let mut terrain = HashMap::default();
        for p in wrapping_logic.pointmap().get_valid_points() {
            terrain.insert(p, environment.default_terrain());
        }
        Map {
            environment: environment.clone(),
            wrapping_logic,
            tags: TagBag::new(),
            pipes: HashMap::default(),
            terrain,
            units: HashMap::default(),
            tokens: HashMap::default(),
        }
    }

    pub fn environment(&self) -> &Environment {
        &self.environment
    }

    pub fn get_tag_bag(&self) -> &TagBag<D> {
        &self.tags
    }
    pub fn get_tag_bag_mut(&mut self) -> &mut TagBag<D> {
        &mut self.tags
    }

    pub fn wrapping_logic(&self) -> &WrappingMap<D> {
        &self.wrapping_logic
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

    pub fn all_points(&self) -> Vec<Point> {
        self.wrapping_logic.pointmap().get_valid_points()
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

    pub fn get_pipes(&self, p: Point) -> &[PipeState<D>] {
        self.pipes.get(&p).map(|pipes| pipes.as_slice()).unwrap_or(&[])
    }
    pub fn set_pipes(&mut self, p: Point, pipes: Vec<PipeState<D>>) {
        if pipes.len() == 0 {
            self.pipes.remove(&p);
        } else {
            let mut used_directions = Vec::new();
            let mut pips = Vec::new();
            'outer: for pipe in pipes {
                for d in pipe.directions() {
                    if used_directions.contains(&d) {
                        break 'outer;
                    }
                    used_directions.push(d);
                }
                pips.push(pipe);
            }
            self.pipes.insert(p, pips);
        }
    }

    pub fn get_terrain(&self, p: Point) -> Option<&Terrain<D>> {
        self.terrain.get(&p)
    }
    pub fn get_terrain_mut(&mut self, p: Point) -> Option<&mut Terrain<D>> {
        self.terrain.get_mut(&p)
    }
    pub fn set_terrain(&mut self, p: Point, t: Terrain<D>) {
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

    pub fn get_tokens(&self, p: Point) -> &[Token<D>] {
        self.tokens.get(&p).map(|v| v.as_slice()).unwrap_or(&[])
    }
    pub fn set_tokens(&mut self, p: Point, value: Vec<Token<D>>) {
        if self.is_point_valid(p) {
            let value = Token::correct_stack(value);
            if value.len() > 0 {
                self.tokens.insert(p, value.try_into().unwrap());
            } else {
                self.tokens.remove(&p);
            }
        }
    }
    pub fn add_token(&mut self, p: Point, value: Token<D>) {
        let mut list = self.get_tokens(p).to_vec();
        list.push(value);
        self.set_tokens(p, list);
    }
    pub fn insert_token(&mut self, p: Point, index: usize, value: Token<D>) {
        let mut list = self.get_tokens(p).to_vec();
        if index <= list.len() {
            list.insert(index, value);
            self.set_tokens(p, list);
        }
    }
    pub fn remove_token(&mut self, p: Point, index: usize) -> Option<Token<D>> {
        if let Some(list) = self.tokens.get_mut(&p) {
            if list.len() > index {
                return Some(list.remove(index));
            }
        }
        None
    }
    
    /**
     * checks the pipe at dp.point for whether it can be entered by dp.direction and if true, returns the position of the next pipe tile
     * returns None if no pipe is at the given location, for example because the previous pipe tile was an exit
     */
    pub fn next_pipe_tile(&self, point: Point, direction: D) -> Option<(Point, Distortion<D>)> {
        if let Some(disto) = self.pipes.get(&point)
        .and_then(|pipes| pipes.iter().find_map(|pipe_state| pipe_state.distortion(direction))) {
            self.wrapping_logic().get_neighbor(point, disto.update_direction(direction))
            .and_then(|(p, d)| Some((p, disto + d)))
        } else {
            None
        }
    }

    /**
     * the returned Distortion has to be applied to 'd' in order to
     * keep moving in the same direction
     */
    pub fn get_neighbor(&self, p: Point, d: D) -> Option<(Point, Distortion<D>)> {
        if let Some((point, mut distortion)) = self.wrapping_logic().get_neighbor(p, d) {
            // look for pipe to enter
            if self.pipes.get(&point)
            .map(|pipes| pipes.iter().any(|pipe_state| pipe_state.distortion(distortion.update_direction(d)).is_some()))
            .unwrap_or(false) {
                // check if pipe can be entered from here (meaning it isn't connected to a pipe at 'p')
                // this should prevent infinite loops
                if self.pipes.get(&p)
                .map(|pipes| !pipes.iter().any(|pipe_state| pipe_state.distortion(d.opposite_direction()).is_some()))
                .unwrap_or(true) {
                    // follow pipe to its end
                    let mut current = point;
                    while let Some((next, disto)) = self.next_pipe_tile(current, distortion.update_direction(d)) {
                        current = next;
                        distortion += disto;
                        if current == point {
                            // infinite loop, shouldn't happen after the above tests!
                            panic!("encountered infinite pipe loop at {p:?} in direction {d}");
                        }
                    }
                    return Some((current, distortion));
                }
            }
            Some((point, distortion))
        } else {
            None
        }
    }

    pub fn get_neighbors(&self, p: Point, mode: NeighborMode) -> Vec<OrientedPoint<D>> {
        let mut result = vec![];
        for d in D::list() {
            match mode {
                NeighborMode::Direct => {
                    if let Some((p, distortion)) = self.wrapping_logic().get_neighbor(p, d) {
                        result.push(OrientedPoint::new(p, distortion.is_mirrored(), distortion.update_direction(d)));
                    }
                }
                NeighborMode::FollowPipes => {
                    if let Some((p, distortion)) = self.get_neighbor(p, d) {
                        result.push(OrientedPoint::new(p, distortion.is_mirrored(), distortion.update_direction(d)));
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
        let mut distortion = Distortion::neutral();
        while result.len() < length {
            let current = result.get(result.len() - 1).unwrap();
            let next = match mode {
                NeighborMode::Direct => self.wrapping_logic().get_neighbor(current.point, distortion.update_direction(d)),
                NeighborMode::FollowPipes => self.get_neighbor(current.point, distortion.update_direction(d)),
            };
            if let Some((p, disto)) = next {
                distortion += disto;
                result.push(OrientedPoint::new(p, distortion.is_mirrored(), distortion.update_direction(d)));
            } else {
                break;
            }
        }
        result
    }

    pub fn width_search(&self, start: Point, mut f: Box<&mut dyn FnMut(Point) -> bool>) -> HashSet<Point> {
        let mut result = HashSet::default();
        let mut rejected = HashSet::default();
        let mut to_check = HashSet::default();
        to_check.insert(start);
        while to_check.len() > 0 {
            let mut next = HashSet::default();
            for p in to_check {
                if f(p) {
                    result.insert(p);
                    for p in self.get_neighbors(p, NeighborMode::Direct) {
                        if !result.contains(&p.point) && !rejected.contains(&p.point) {
                            next.insert(p.point);
                        }
                    }
                } else {
                    rejected.insert(p);
                }
            }
            to_check = next;
        }
        result
    }

    pub fn range_in_layers(&self, center: Point, range: usize) -> Vec<HashSet<Point>> {
        if range == 0 {
            return Vec::new();
        }
        let mut result = Vec::new();
        let mut layer: HashSet<(Point, D, Option<D>)> = HashSet::default();
        for dp in self.get_neighbors(center, NeighborMode::FollowPipes) {
            layer.insert((dp.point, dp.direction, None));
        }
        for _ in 1..range {
            let previous_layer = layer;
            layer = HashSet::default();
            let mut result_layer = HashSet::default();
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
        let mut result_layer = HashSet::default();
        for (p, _, _) in layer {
            result_layer.insert(p);
        }
        result.push(result_layer);
        result
    }

    pub fn fix_errors_tokens(&self) -> HashMap<Point, Vec<Token<D>>> {
        let mut corrected = HashMap::default();
        for p in self.all_points() {
            let stack = Token::correct_stack(self.get_tokens(p).to_vec());
            if *self.tokens.get(&p).unwrap_or(&stack) != stack {
                corrected.insert(p, stack);
            }
        }
        corrected
    }

    pub fn get_viable_player_ids(&self, _game: &impl GameView<D>) -> Vec<u8> {
        let mut owners = HashSet::default();
        for p in self.all_points() {
            if let Some(unit) = self.get_unit(p) {
                if unit.get_owner_id() >= 0 {
                    owners.insert(unit.get_owner_id() as u8);
                }
            }
            let t = self.get_terrain(p).unwrap();
            if t.get_owner_id() >= 0 && self.environment.config.terrain_owner_is_playable(t.typ()) {
                owners.insert(t.get_owner_id() as u8);
            }
            for token in self.get_tokens(p) {
                if token.get_owner_id() >= 0 && self.environment.config.token_owner_is_playable(token.typ()) {
                    owners.insert(token.get_owner_id() as u8);
                }
            }
        }
        let mut owners: Vec<u8> = owners.into_iter().collect();
        owners.sort();
        owners
    }

    pub fn get_field_data(&self, p: Point) -> FieldData<D> {
        FieldData {
            pipes: self.pipes.get(&p).map(|pipes| pipes.clone().try_into().unwrap()).unwrap_or(LVec::new()),
            terrain: self.terrain.get(&p).unwrap().clone(),
            tokens: self.tokens.get(&p).cloned().map(|v| v.try_into().unwrap()).unwrap_or(LVec::new()),
            unit: self.units.get(&p).cloned(),
        }
    }

    pub fn import_from_unzipper(unzipper: &mut Unzipper, environment: &mut Environment) -> Result<Self, ZipperError> {
        let wrapping_logic = WrappingMap::unzip(unzipper)?;
        environment.map_size = wrapping_logic.pointmap().size();
        let tags = TagBag::import(unzipper, environment)?;
        let mut pipes = HashMap::default();
        let mut terrain = HashMap::default();
        let mut units = HashMap::default();
        let mut tokens = HashMap::default();
        for p in wrapping_logic.pointmap().get_valid_points() {
            let fd = FieldData::import(unzipper, environment)?;
            if fd.pipes.len() > 0 {
                pipes.insert(p, fd.pipes.into_inner());
            }
            terrain.insert(p, fd.terrain);
            if fd.tokens.len() > 0 {
                tokens.insert(p, fd.tokens.into_inner());
            }
            if let Some(unit) = fd.unit {
                units.insert(p, unit);
            }
        }
        Ok(Self {
            environment: environment.clone(),
            wrapping_logic,
            tags,
            pipes,
            terrain,
            units,
            tokens,
        })
    }

    pub(crate) fn start_game(&mut self, settings: &Urc<GameSettings>) {
        self.environment.start_game(settings);
        for p in self.all_points() {
            self.terrain.get_mut(&p).unwrap().start_game(settings);
            if let Some(tokens) = self.tokens.get_mut(&p) {
                for token in tokens {
                    token.start_game(settings);
                }
            }
            if let Some(unit) = self.units.get_mut(&p) {
                unit.start_game(settings);
            }
        }
    }

    pub fn settings(&self) -> Result<GameConfig<D>, NotPlayable> {
        let as_view = Handle::new(self.clone());
        let owners = self.get_viable_player_ids(&as_view);
        if owners.len() < 2 {
            return Err(NotPlayable::TooFewPlayers);
        }
        let random: RandomFn = Urc::new(|| 0.);
        let players:Vec<PlayerConfig<D>> = owners.into_iter()
            .map(|owner| PlayerConfig::new(owner, self, &random))
            .collect();
        Ok(settings::GameConfig {
            fog_mode: FogMode::Constant(FogSetting::Light(0)),
            tags: self.tags.clone(),
            players: players.try_into().unwrap(),
        })
    }

    #[cfg(feature = "rendering")]
    pub(crate) fn preview(&self) -> MapPreview {
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

impl<D: Direction> GameView<D> for Handle<Map<D>> {
    fn environment(&self) -> Environment {
        self.with(|map| map.environment.clone())
    }

    fn all_points(&self) -> Vec<Point> {
        self.with(|map| map.all_points())
    }

    fn get_pipes(&self, p: Point) -> Vec<PipeState<D>> {
        self.with(|map| map.get_pipes(p).to_vec())
    }

    fn get_terrain(&self, p: Point) -> Option<Terrain<D>> {
        self.with(|map| map.get_terrain(p).cloned())
    }

    fn get_tokens(&self, p: Point) -> Vec<Token<D>> {
        self.with(|map| map.get_tokens(p).to_vec())
    }

    fn get_unit(&self, p: Point) -> Option<Unit<D>> {
        self.with(|map| map.get_unit(p).cloned())
    }

    fn as_shared(&self) -> SharedGameView<D> {
        SharedGameView(Urc::new(self.cloned()))
    }

    fn wrapping_logic(&self) -> ReadGuard<'_, WrappingMap<D>> {
        self.borrow(|map| map.wrapping_logic())
    }

    fn next_pipe_tile(&self, point: Point, direction: D) -> Option<(Point, Distortion<D>)> {
        self.with(|map| map.next_pipe_tile(point, direction))
    }

    fn get_neighbor(&self, p: Point, d: D) -> Option<(Point, Distortion<D>)> {
        self.with(|map| map.get_neighbor(p, d))
    }

    fn get_neighbors(&self, p: Point, mode: NeighborMode) -> Vec<OrientedPoint<D>> {
        self.with(|map| map.get_neighbors(p, mode))
    }

    fn width_search(&self, start: Point, f: Box<&mut dyn FnMut(Point) -> bool>) -> HashSet<Point> {
        self.with(|map| map.width_search(start, f))
    }

    fn range_in_layers(&self, center: Point, range: usize) -> Vec<HashSet<Point>> {
        self.with(|map| map.range_in_layers(center, range))
    }

    fn get_line(&self, start: Point, d: D, length: usize, mode: NeighborMode) -> Vec<OrientedPoint<D>> {
        self.with(|map| map.get_line(start, d, length, mode))
    }

    fn current_owner(&self) -> i8 {
        -1
    }

    fn get_owning_player(&self, _: i8) -> Option<Player<D>> {
        None
    }

    fn get_team(&self, _owner: i8) -> ClientPerspective {
        // could be useful to give each owner a team
        ClientPerspective::Neutral
    }

    fn get_fog_setting(&self) -> FogSetting {
        FogSetting::None
    }

    fn get_fog_at(&self, _: ClientPerspective, _: Point) -> FogIntensity {
        FogIntensity::TrueSight
    }

    fn get_visible_unit(&self, _: ClientPerspective, p: Point) -> Option<Unit<D>> {
        self.with(|map| map.get_unit(p).cloned())
    }

    fn get_attack_config_limit(&self) -> Option<usize> {
        Handle::get_attack_config_limit(self)
    }
    fn set_attack_config_limit(&self, limit: Option<usize>) {
        Handle::set_attack_config_limit(self, limit);
    }
    fn get_unit_config_limit(&self) -> Option<usize> {
        Handle::get_unit_config_limit(self)
    }
    fn set_unit_config_limit(&self, limit: Option<usize>) {
        Handle::set_unit_config_limit(self, limit);
    }
    fn get_terrain_config_limit(&self) -> Option<usize> {
        Handle::get_terrain_config_limit(self)
    }
    fn set_terrain_config_limit(&self, limit: Option<usize>) {
        Handle::set_terrain_config_limit(self, limit);
    }
}

pub enum MapType {
    Square(Map<Direction4>),
    Hex(Map<Direction6>),
}

pub fn import_map(config: &Urc<Config>, bytes: Vec<u8>, version: Version) -> Result<MapType, ZipperError> {
    let mut environment = Environment::new_map(config.clone(), MapSize::new(0, 0));
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
    pub pipes: LVec<PipeState<D>, 3>,
    pub terrain: Terrain<D>,
    pub tokens: LVec<Token<D>, {tokens::MAX_STACK_SIZE}>,
    pub unit: Option<Unit<D>>,
}

impl<D: Direction> FieldData<D> {
    pub fn game_field(game: &Handle<Game<D>>, p: Point) -> Self {
        Self {
            pipes: game.get_pipes(p).try_into().unwrap(),
            terrain: game.get_terrain(p).unwrap(),
            tokens: game.get_tokens(p).try_into().unwrap(),
            unit: game.get_unit(p),
        }
    }

    pub fn fog_replacement(self, game: &Handle<Game<D>>, pos: Point, intensity: FogIntensity) -> Self {
        let tokens: Vec<_> = self.tokens.into_iter()
        .filter_map(|d| d.fog_replacement(intensity))
        .collect();
        Self {
            pipes: self.pipes.clone(),
            unit: self.unit.and_then(|unit| unit.fog_replacement(game, pos, intensity)),
            tokens: tokens.try_into().expect("Detail list shouldn't become longer after filtering"),
            terrain: self.terrain.fog_replacement(intensity),
        }
    }
}

impl<D: Direction> MapInterface for Handle<Map<D>> {
    fn export(&self) -> Vec<u8> {
        let mut zipper = Zipper::new();
        zipper.write_bool(D::is_hex());
        self.with(|map| {
            map.wrapping_logic.zip(&mut zipper);
            map.tags.export(&mut zipper, &map.environment);
            for p in map.all_points() {
                map.get_field_data(p).export(&mut zipper, &map.environment);
            }
        });
        zipper.finish()
    }

    fn width(&self) -> usize {
        self.with(|map| map.width()) as usize
    }

    fn height(&self) -> usize {
        self.with(|map| map.height()) as usize
    }

    fn player_count(&self) -> u16 {
        self.with(|map| map.get_viable_player_ids(self).len()) as u16
    }

    // TODO: add metrics. metrics maybe should be floats instead of integers
    fn metrics(&self) -> std::collections::HashMap<String, i32> {
        let result = std::collections::HashMap::default();
        /*let mut income = 0;
        self.with(|map| {
            for t in map.terrain.values() {
                income += t.income_factor();
            }
        });
        result.insert("Total Income".to_string(), income);*/
        result
    }

    fn default_settings(&self) -> Result<Box<dyn GameSettingsInterface>, Box<dyn Error>> {
        let owners = self.with(|map| map.get_viable_player_ids(self));
        if owners.len() < 2 {
            return Err(Box::new(NotPlayable::TooFewPlayers));
        }
        let random: RandomFn = Urc::new(|| 0.);
        let players:Vec<PlayerConfig<D>> = self.with(|map| {
            owners.into_iter()
            .map(|owner| PlayerConfig::new(owner, map, &random))
            .collect()
        });
        Ok(Box::new(settings::GameConfig {
            fog_mode: FogMode::Constant(FogSetting::Light(0)),
            tags: self.with(|map| map.tags.clone()),
            players: players.try_into().unwrap(),
        }))
    }

    fn parse_settings(&self, bytes: Vec<u8>) -> Result<Box<dyn GameSettingsInterface>, Box<dyn Error>> {
        let settings = self.with(|map| {
            GameConfig::import(map, bytes)
        })?;
        Ok(Box::new(settings))
    }

    fn check_player_setting(&self, game_settings: Vec<u8>, player_index: usize, bytes: Vec<u8>) -> Result<Vec<u8>, Box<dyn Error>> {
        self.with(|map| {
            let settings = GameConfig::import(map, game_settings)?;
            settings.check_player_setting(&map.environment.config, player_index, bytes)
        })
    }

    fn game_creator(self: Box<Self>, settings: Vec<u8>, player_settings: Vec<Vec<u8>>) -> Result<Box<dyn GameCreationInterface>, Box<dyn Error>> {
        let settings = self.with(|map| {
            GameConfig::import(map, settings)
        })?;
        if player_settings.len() != settings.players.len() {
            return Err(Box::new(PlayerSettingError::PlayerCount(settings.players.len(), player_settings.len())));
        }
        let mut player_selection = Vec::with_capacity(player_settings.len());
        for bytes in player_settings {
            let mut unzipper = Unzipper::new(bytes, Version::parse(VERSION).unwrap());
            player_selection.push(PlayerSelectedOptions::import(&mut unzipper, &self.environment().config)?);
        }
        Ok(Box::new(GameCreation {
            map: self.with(|map| map.clone()),
            settings,
            player_selection,
        }))
    }

    #[cfg(feature = "rendering")]
    fn preview(&self) -> MapPreview {
        self.with(|map| {
            map.preview()
        })
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
    pub settings: settings::GameConfig<D>,
    pub player_selection: Vec<settings::PlayerSelectedOptions>,
}

impl<D: Direction> GameCreationInterface for GameCreation<D> {
    fn server(self: Box<Self>, random: RandomFn) -> (Box<dyn GameInterface>, Events) {
        let settings = self.settings.build(&self.player_selection, &random);
        let (server, events) = Game::new_server(self.map, &self.settings, settings, random);
        let events = server.with(|s| events.export(s.environment()));
        (server, events)
    }

    fn server_and_client(self: Box<Self>, client_perspective: ClientPerspective, random: RandomFn) -> (Box<dyn GameInterface>, Box<dyn GameInterface>, Events) {
        let settings = self.settings.build(&self.player_selection, &random);
        let (server, events) = Game::new_server(self.map.clone(), &self.settings, settings.clone(), random);
        let client = Game::new_client(self.map, &self.settings, settings, events.get(&client_perspective.into()).unwrap_or(&[]));
        let events = server.with(|s| events.export(s.environment()));
        (server, client, events)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NeighborMode {
    Direct,
    FollowPipes,
}

pub fn get_unit<D: Direction>(map: &impl GameView<D>, p: Point, unload_index: Option<usize>) -> Option<Unit<D>> {
    let mut unit = map.get_unit(p)?;
    match unload_index {
        Some(index) => {
            let mut transported = unit.get_transported_mut();
            if transported.len() > index {
                Some(transported.swap_remove(index))
            } else {
                None
            }
        }
        None => Some(unit),
    }
}

/**
 * returns (up to) range+1 Points reached by moving from 'start' in direction 'd' in a straight line
 * the Distortions already include previous distortions
 * the result may be shorter than range+1 if not enough points could be found
 */
pub fn get_line<D: Direction>(map: &impl GameView<D>, start: Point, d: D, range: usize, mode: NeighborMode) -> Vec<(Point, Distortion<D>)> {
    let mut result = vec![(start, Distortion::neutral())];
    let wrapping_logic: ReadGuard<'_, WrappingMap<D>>;
    let get_next: Box<dyn Fn(Point, D) -> Option<(Point, Distortion<D>)>> = match mode {
        NeighborMode::Direct => {
            wrapping_logic = map.wrapping_logic();
            Box::new(|p, d| wrapping_logic.get_neighbor(p, d))
        }
        NeighborMode::FollowPipes => Box::new(|p, d| map.get_neighbor(p, d))
    };
    for i in 0..range {
        if let Some((p, distortion)) = get_next(result[i].0, result[i].1.update_direction(d)) {
            result.push((p, result[i].1 + distortion));
        } else {
            break;
        }
    }
    result
}

/**
 * returns points that can be reached by moving to neighbors while only moving outwards from the center
 * returns range+1 sets. the same point can be in multiple sets
 * when searching in all directions, set diagonal_directions to D::list()
 * setting diagonal_directions to Direction4::0 only searches towards the top-right
 */
pub fn range_in_layers<D: Direction>(map: &impl GameView<D>, center: Point, range: usize, diagonal_directions: &[D]) -> Vec<HashSet<(Point, Distortion<D>)>> {
    let mut result = Vec::new();
    for _ in 0..=range {
        result.push(HashSet::default());
    }
    result[0].insert((center, Distortion::neutral()));
    for d in diagonal_directions {
        let d2 = d.rotate(true);
        let mut previous_layer = result[0].clone();
        let mut layer = HashSet::default();
        for i in 1..=range {
            for (p, distortion) in previous_layer {
                for d in [*d, d2] {
                    if let Some((p, new_distortion)) = map.get_neighbor(p, distortion.update_direction(d)) {
                        layer.insert((p, distortion + new_distortion));
                    }
                }
            }
            result[i].extend(layer.iter().cloned());
            previous_layer = layer;
            layer = HashSet::default();
        }
    }
    result
}

/**
 * returns points that can be reached by moving to neighbors while only moving along allowed directions and their diagonals
 * returns range+1 sets. the same point can be in multiple sets
 * when searching in all directions, set directions to D::list()
 * setting directions to Direction4::0 only searches towards the right
 */
pub fn cannon_range_in_layers<D: Direction>(map: &impl GameView<D>, center: Point, range: usize, directions: &[D]) -> Vec<HashSet<(Point, Distortion<D>)>> {
    let mut result = Vec::new();
    for _ in 0..=range {
        result.push(HashSet::default());
    }
    result[0].insert((center, Distortion::neutral()));
    if D::is_hex() {
        for d in directions {
            let d2 = d.rotate(true);
            let d3 = d.rotate(false);
            let mut previous_back = result[0].clone();
            let mut back = HashSet::default();
            let mut previous_forward: HashMap<(Point, Distortion<D>), u8> = HashMap::default();
            let mut forward: HashMap<(Point, Distortion<D>), u8> = HashMap::default();
            for i in 1..=range {
                for (p, distortion) in previous_back {
                    if let Some((p, new_distortion)) = map.get_neighbor(p, distortion.update_direction(*d)) {
                        let distortion = distortion + new_distortion;
                        // move forward
                        back.insert((p, distortion));
                        // move sideways
                        for d in [d2, d3] {
                            if let Some((p, new_distortion)) = map.get_neighbor(p, distortion.update_direction(d)) {
                                let key = (p, distortion + new_distortion);
                                let old_value = forward.remove(&key).unwrap_or(0u8);
                                forward.insert(key, old_value + 1);
                            }
                        }
                    }
                }
                for ((p, distortion), strength) in previous_forward {
                    if let Some((p, new_distortion)) = map.get_neighbor(p, distortion.update_direction(*d)) {
                        let distortion = distortion + new_distortion;
                        // move forward
                        forward.insert((p, distortion), 2);
                    }
                    if strength >= 2 {
                        // move sideways
                        for d in [d2, d3] {
                            if let Some((p, new_distortion)) = map.get_neighbor(p, distortion.update_direction(d)) {
                                back.insert((p, distortion + new_distortion));
                            }
                        }
                    }
                }
                result[i].extend(back.iter().cloned());
                result[i].extend(forward.keys().cloned());
                previous_back = back;
                previous_forward = forward;
                back = HashSet::default();
                forward = HashMap::default();
            }
        }
    } else {
        for d in directions {
            let d2 = d.rotate(true);
            let d3 = d.rotate(false);
            let mut previous_layer = result[0].clone();
            let mut layer = HashSet::default();
            for i in 1..=range {
                for (p, distortion) in previous_layer {
                    if let Some((p, new_distortion)) = map.get_neighbor(p, distortion.update_direction(*d)) {
                        let distortion = distortion + new_distortion;
                        // move forward
                        layer.insert((p, distortion));
                        // move sideways
                        for d in [d2, d3] {
                            if let Some((p, new_distortion)) = map.get_neighbor(p, distortion.update_direction(d)) {
                                layer.insert((p, distortion + new_distortion));
                            }
                        }
                    }
                }
                result[i].extend(layer.iter().cloned());
                previous_layer = layer;
                layer = HashSet::default();
            }
        }
    }
    result
}

pub fn get_income_factor<D: Direction>(map: &impl GameView<D>, owner_id: i8) -> Rational32 {
    // income from properties
    let mut income_factor = Rational32::from_integer(0);
    let hero_map = HeroMap::new(map, Some(owner_id));
    for p in map.all_points() {
        let t = map.get_terrain(p).unwrap();
        if t.get_owner_id() == owner_id {
            income_factor += t.income_factor(map, p, hero_map.get(p, owner_id));
        }
    }
    income_factor
}
