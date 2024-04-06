use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use zipper::*;
use interfaces::game_interface::{self, Events, ClientPerspective};
use interfaces::game_interface::GameInterface;
use semver::Version;

use crate::config::environment::Environment;
use crate::config::config::Config;
use crate::details::Detail;
use crate::map::map::*;
use crate::map::direction::*;
use crate::game::settings;
use crate::game::events;
use crate::game::fog::*;
use crate::map::map_view::MapView;
use crate::map::point::Point;
use crate::map::point_map::MapSize;
use crate::map::wrapping_map::WrappingMap;
use crate::terrain::terrain::Terrain;
use crate::{player::*, VERSION};
use crate::units::attributes::AttributeKey;
use crate::units::hero::*;
use crate::units::movement::Path;
use crate::units::unit::*;
use crate::units::unit_types::UnitType;

use super::events::Event;
use super::game_view::GameView;
use super::settings::GameSettings;
use super::{event_handler, commands};

#[derive(Debug, Clone, PartialEq)]
pub struct Game<D: Direction> {
    environment: Environment,
    map: Map<D>,
    pub current_turn: u32,
    ended: bool,
    pub players: LVec<Player, 16>,
    fog_mode: FogMode,
    fog: HashMap<ClientPerspective, HashMap<Point, FogIntensity>>,
}

impl<D: Direction> Game<D> {
    fn new(mut map: Map<D>, settings: &settings::GameSettings) -> Self {
        let settings = settings.start();
        map.start_game(&Arc::new(settings));
        let settings = map.environment().settings.as_ref().unwrap();
        let fog_mode = settings.fog_mode.clone();
        let players: Vec<Player> = settings.players.iter()
            .map(|player| player.build(map.environment()))
            .collect();
        Game {
            environment: map.environment().clone(),
            fog: create_base_fog(&map, &players),
            current_turn: 0,
            ended: false,
            players: players.try_into().unwrap(),
            map,
            fog_mode,
        }
    }

    pub fn new_server<R: 'static + Fn() -> f32>(map: Map<D>, settings: &settings::GameSettings, random: R) -> (Self, Events<Self>) {
        let mut this = Self::new(map, settings);
        let events = this.start_server(random);
        (this, events)
    }

    pub fn new_client(map: Map<D>, settings: &settings::GameSettings, events: &[events::Event<D>]) -> Self {
        let mut this = Self::new(map, settings);
        for e in events {
            this.handle_event(e);
        }
        this
    }

    fn start_server<R: 'static + Fn() -> f32>(&mut self, random: R) -> Events<Self> {
        let mut handler = event_handler::EventHandler::new(self, Box::new(random));
        handler.start_turn(None);
        handler.accept()
    }

    pub fn get_fog_mode(&self) -> &FogMode {
        &self.fog_mode
    }

    pub fn get_fog_setting(&self) -> FogSetting {
        self.get_fog_mode().fog_setting(self.current_turn(), self.players.len())
    }

    pub fn recalculate_fog(&self, perspective: ClientPerspective) -> HashMap<Point, FogIntensity> {
        let mut fog = HashMap::new();
        let strongest_intensity = self.fog_mode.fog_setting(self.current_turn as usize, self.players.len()).intensity();
        for p in self.get_map().all_points() {
            fog.insert(p, strongest_intensity);
        }
        if !self.is_foggy() {
            return fog;
        }
        let heroes = Hero::map_influence(self, -1);
        for p in self.get_map().all_points() {
            let terrain = self.get_map().get_terrain(p).unwrap();
            let terrain_heroes = if terrain.get_team() != ClientPerspective::Neutral {
                heroes.get(&(p, terrain.get_owner_id())).map(|h| h.as_slice()).unwrap_or(&[])
            } else {
                &[]
            };
            for (p, v) in terrain.get_vision(self, p, terrain_heroes, perspective) {
                fog.insert(p, v.min(fog.get(&p).clone().unwrap().clone()));
            }
            if let Some(unit) = self.get_map().get_unit(p) {
                if perspective != ClientPerspective::Neutral && perspective == unit.get_team() {
                    let heroes = heroes.get(&(p, unit.get_owner_id())).map(|h| h.as_slice()).unwrap_or(&[]);
                    for (p, v) in unit.get_vision(self, p, heroes) {
                        fog.insert(p, v.min(fog.get(&p).clone().unwrap().clone()));
                    }
                }
            }
            for det in self.get_map().get_details(p) {
                for (p, v) in det.get_vision(self, p, perspective) {
                    fog.insert(p, v.min(fog.get(&p).clone().unwrap().clone()));
                }
            }
        }
        fog
    }
    
    pub fn get_map(&self) -> &Map<D> {
        &self.map
    }

    pub fn get_map_mut(&mut self) -> &mut Map<D> {
        &mut self.map
    }

    pub fn current_player(&self) -> &Player {
        &self.players[self.current_turn as usize % self.players.len()]
    }

    pub fn get_teams(&self) -> HashSet<u8> {
        let mut result = HashSet::new();
        for p in self.players.iter() {
            match p.get_team() {
                ClientPerspective::Team(team) => {
                    result.insert(team);
                }
                _ => panic!("player should not be neutral"),
            }
        }
        result
    }

    pub fn get_living_teams(&self) -> HashSet<u8> {
        let mut result = HashSet::new();
        for p in self.players.iter()
        .filter(|p| !p.dead) {
            match p.get_team() {
                ClientPerspective::Team(team) => {
                    result.insert(team);
                }
                _ => panic!("player should not be neutral"),
            }
        }
        result
    }

    pub fn is_team_alive(&self, team: u8) -> bool {
        self.get_living_teams().contains(&team)
    }

    pub fn set_ended(&mut self, ended: bool) {
        self.ended = ended;
    }

    pub fn get_owning_player_mut(&mut self, owner: i8) -> Option<&mut Player> {
        self.players.iter_mut().find(|player| player.get_owner_id() == owner)
    }

    pub fn get_team(&self, owner: Option<i8>) -> ClientPerspective {
        owner.and_then(|o| self.get_owning_player(o)).and_then(|p| Some(p.get_team())).unwrap_or(ClientPerspective::Neutral)
    }

    pub fn will_be_foggy(&self, turns_later: usize) -> bool {
        self.fog_mode.is_foggy(self.current_turn as usize + turns_later, self.players.len())
    }

    pub fn get_fog(&self) -> &HashMap<ClientPerspective, HashMap<Point, FogIntensity>> {
        &self.fog
    }

    pub fn can_see_unit_at(&self, team: ClientPerspective, position: Point, unit: &Unit<D>, accept_unknowns: bool) -> bool {
        match unit.fog_replacement(self, position, self.get_fog_at(team, position)) {
            None => false,
            Some(unit) => accept_unknowns || unit.typ() != UnitType::Unknown,
        }
    }

    pub fn set_fog(&mut self, team: ClientPerspective, pos: Point, intensity: FogIntensity) {
        let fog = self.fog.get_mut(&team).expect(&format!("attempted to set fog for {:?} at {:?}: {:?}", team, pos, intensity));
        if intensity == FogIntensity::TrueSight {
            fog.remove(&pos);
        } else {
            fog.insert(pos, intensity);
        }
    }
    
    pub fn undo(&mut self, events: &[events::Event<D>]) {
        for event in events.iter().rev() {
            event.undo(self);
        }
    }

    pub fn find_visible_threats(&self, pos: Point, threatened: &Unit<D>, team: ClientPerspective) -> HashSet<Point> {
        let mut result = HashSet::new();
        for p in self.map.all_points() {
            if let Some(unit) = self.map.get_unit(p) {
                if self.can_see_unit_at(team, p, unit, false) && unit.threatens(threatened) && unit.shortest_path_to_attack(self, &Path::new(p), None, pos).is_some() {
                    result.insert(p);
                }
                // TODO: also check transported units
            }
        }
        result
    }

    pub fn visible_unit_with_attribute(&self, team: ClientPerspective, pos: Point, attribute: AttributeKey) -> bool {
        self.get_map().get_unit(pos).unwrap()
        .fog_replacement(self, pos, self.get_fog_at(team, pos))
        .and_then(|u| Some(u.has_attribute(attribute))).unwrap_or(false)
    }

    pub fn export_field(&self, zipper: &mut Zipper, p: Point, fog_intensity: FogIntensity) {
        let fd = self.map.get_field_data(p).fog_replacement(self, p, fog_intensity);
        fd.export(zipper, &self.environment);
    }

    pub fn zip(&self, zipper: &mut Zipper, fog: Option<&HashMap<Point, FogIntensity>>) {
        zipper.write_bool(D::is_hex());
        self.environment.settings.as_ref().unwrap().export(zipper, &self.environment.config, true);
        self.map.wrapping_logic().zip(zipper);
        for p in self.map.all_points() {
            self.export_field(zipper, p, fog.and_then(|fog| fog.get(&p).cloned()).unwrap_or(FogIntensity::TrueSight));
        }
    }

}

impl<D: Direction> MapView<D> for Game<D> {
    fn environment(&self) -> &Environment {
        &self.environment
    }

    fn wrapping_logic(&self) -> &WrappingMap<D> {
        self.map.wrapping_logic()
    }

    fn all_points(&self) -> Vec<Point> {
        self.map.all_points()
    }

    fn get_terrain(&self, p: Point) -> Option<&Terrain> {
        self.map.get_terrain(p)
    }

    fn get_details(&self, p: Point) -> &[Detail<D>] {
        self.map.get_details(p)
    }

    fn get_unit(&self, p: Point) -> Option<&Unit<D>> {
        self.map.get_unit(p)
    }
}

impl<D: Direction> GameView<D> for Game<D> {
    fn get_owning_player(&self, owner: i8) -> Option<&Player> {
        self.players.iter().find(|player| player.get_owner_id() == owner)
    }

    fn fog_intensity(&self) -> FogIntensity {
        self.fog_mode.fog_setting(self.current_turn as usize, self.players.len()).intensity()
    }

    fn get_fog_at(&self, team: ClientPerspective, position: Point) -> FogIntensity {
        self.fog.get(&team).and_then(|fog| fog.get(&position)).cloned().unwrap_or(FogIntensity::TrueSight)
    }

    fn get_visible_unit(&self, team: ClientPerspective, p: Point) -> Option<Unit<D>> {
        self.get_unit(p)
        .and_then(|u| {
            // use base's fog instead of self.get_fog_at
            // when the server verifies a unit's available actions, units invisible to the player shouldn't have an influence
            // but maybe it should be possible to predict the fog
            u.fog_replacement(self, p, self.get_fog_at(team, p))
        })
    }
}

fn export_fog(zipper: &mut Zipper, points: &Vec<Point>, fog: &HashMap<Point, FogIntensity>) {
    for p in points {
        let intensity = fog.get(&p).cloned().unwrap_or(FogIntensity::TrueSight);
        intensity.zip(zipper);
    }
}

fn import_fog(unzipper: &mut Unzipper, points: &Vec<Point>) -> Result<HashMap<Point, FogIntensity>, ZipperError> {
    let mut result = HashMap::new();
    for p in points {
        let intensity = FogIntensity::unzip(unzipper)?;
        if intensity != FogIntensity::TrueSight {
            result.insert(*p, intensity);
        }
    }
    Ok(result)
}

fn create_base_fog<D: Direction>(_map: &Map<D>, players: &[Player]) -> HashMap<ClientPerspective, HashMap<Point, FogIntensity>> {
    let mut fog = HashMap::new();
    let neutral_fog: HashMap<Point, FogIntensity> = HashMap::new();
    for player in players {
        // TODO: maybe fog-maps should only be added for visible teams
        // (so all for the server but only your team's for client)
        if !fog.contains_key(&player.get_team()) {
            fog.insert(player.get_team(), neutral_fog.clone());
        }
    }
    fog.insert(ClientPerspective::Neutral, neutral_fog);
    fog
}

fn import_game_base<D: Direction>(unzipper: &mut Unzipper, config: &Arc<Config>, name: String, is_server: bool) -> Result<Game<D>, ZipperError> {
    // is_hex: skip because at this point we already know
    unzipper.read_bool()?;
    let mut environment = Environment {
        config: config.clone(),
        map_size: MapSize::new(0, 0),
        settings: Some(Arc::new(GameSettings::import(unzipper, config, name, true)?)),
    };
    let map = Map::<D>::import_from_unzipper(unzipper, &mut environment)?;
    let current_turn = unzipper.read_u32(32)?;
    let ended = unzipper.read_bool()?;
    let fog_mode = FogMode::unzip(unzipper)?;
    let player_len = unzipper.read_u8(4)? + 1;
    let mut players = vec![];
    for _ in 0..player_len {
        players.push(Player::import(unzipper, &environment, !is_server && fog_mode.is_foggy(current_turn as usize, player_len as usize))?);
    }
    Ok(Game {
        fog: create_base_fog(&map, &players),
        map,
        current_turn,
        ended,
        fog_mode,
        players: players.try_into().unwrap(),
        environment,
    })
}

impl<D: Direction> game_interface::GameInterface for Game<D> {
    type Event = events::Event<D>;
    type Command = commands::Command<D>;
    type CommandError = commands::CommandError;
    type ImportError = ZipperError;
    type Config = Arc<Config>;

    fn import_server(data: game_interface::ExportedGame, config: &Arc<Config>, name: String, version: Version) -> Result<Box<Self>, ZipperError> {
        if let Some(mut hidden_data) = data.hidden {
            let mut unzipper = Unzipper::new(hidden_data.server, version.clone());
            let mut game = import_game_base(&mut unzipper, config, name, true)?;

            let points = game.map.all_points();
            game.fog.insert(ClientPerspective::Neutral, import_fog(&mut unzipper, &points)?);
            
            for team in game.get_living_teams() {
                if let Some(data) = hidden_data.teams.remove(&(team)) {
                    let mut unzipper = Unzipper::new(data, version.clone());
                    game.fog.insert(ClientPerspective::Team(team), import_fog(&mut unzipper, &points)?);
                }
            }

            Ok(Box::new(game))
        } else {
            let mut unzipper = Unzipper::new(data.public, version);
            let game = import_game_base(&mut unzipper, config, name, true)?;
            Ok(Box::new(game))
        }
    }

    fn import_client(public: Vec<u8>, team_view: Option<(u8, Vec<u8>)>, config: &Arc<Config>, name: String, version: Version) -> Result<Box<Game<D>>, ZipperError> {
        let mut unzipper = Unzipper::new(public, version.clone());
        let mut game = import_game_base(&mut unzipper, config, name, false)?;
        let points = game.map.all_points();
        let neutral_fog = if game.is_foggy() {
            import_fog(&mut unzipper, &points)?
        } else {
            HashMap::new()
        };
        if let Some((team, team_view)) = team_view {
            let mut unzipper = Unzipper::new(team_view, version);
            let fog = import_fog(&mut unzipper, &points)?;
            for p in &points {
                if fog.get(p).cloned().unwrap_or(FogIntensity::TrueSight) < neutral_fog.get(p).cloned().unwrap_or(FogIntensity::TrueSight) {
                    let field_data = FieldData::import(&mut unzipper, &game.environment)?;
                    game.map.set_terrain(p.clone(), field_data.terrain);
                    game.map.set_details(p.clone(), field_data.details.to_vec());
                    game.map.set_unit(p.clone(), field_data.unit);
                }
            }
            game.fog.insert(ClientPerspective::Team(team), fog);
            let mut players: Vec<Player> = vec![];
            for player in game.players.iter() {
                players.push(if player.get_team() == ClientPerspective::Team(team) {
                    Player::import(&mut unzipper, &game.environment, false)?
                } else {
                    player.clone()
                });
            }
            game.players = players.try_into().unwrap();
        } else {
            // teams don't receive events for neutral, so neutral fog wouldn't be consistent
            game.fog.insert(ClientPerspective::Neutral, neutral_fog);
        }
        Ok(Box::new(game))
    }

    fn handle_command<R: 'static + Fn() -> f32>(&mut self, command: commands::Command<D>, random: R) -> Result<Events<Self>, commands::CommandError> {
        let mut handler = event_handler::EventHandler::new(self, Box::new(random));
        match command.execute(&mut handler) {
            Ok(()) => Ok(handler.accept()),
            Err(err) => {
                handler.cancel();
                Err(err)
            }
        }
    }

    fn handle_event(&mut self, event: &events::Event<D>) {
        event.apply(self);
    }

    fn undo_event(&mut self, event: &Self::Event) {
        event.undo(self)
    }

    fn has_secrets(&self) -> bool {
        self.is_foggy()
    }

    fn export(&self) -> game_interface::ExportedGame {
        // server perspective
        let mut zipper = Zipper::new();
        self.zip(&mut zipper, None);
        zipper.write_u32(self.current_turn, 32);
        zipper.write_bool(self.ended);
        self.fog_mode.zip(&mut zipper);
        zipper.write_u8(self.players.len() as u8 - 1, 4);
        for player in self.players.iter() {
            player.export(&mut zipper, false);
        }
        if self.is_foggy() {
            let points = self.map.all_points();
            // Server-perspective. only needs neutral fog, the teams' vision is exported later
            let neutral_fog = self.fog.get(&ClientPerspective::Neutral).unwrap();
            export_fog(&mut zipper, &points, neutral_fog);
            let server = zipper.finish();
            // "None" perspective, visible to all
            let mut zipper = Zipper::new();
            self.zip(&mut zipper, Some(neutral_fog));
            zipper.write_u32(self.current_turn, 32);
            zipper.write_bool(self.ended);
            self.fog_mode.zip(&mut zipper);
            zipper.write_u8(self.players.len() as u8 - 1, 4);
            for player in self.players.iter() {
                player.export(&mut zipper, true);
            }
            export_fog(&mut zipper, &points, neutral_fog);
            let public = zipper.finish();
            // team perspectives
            let mut teams = HashMap::new();
            for team in self.get_living_teams() {
                // team perspective, one per team
                if let Some(fog) = self.fog.get(&ClientPerspective::Team(team)) {
                    let mut zipper = Zipper::new();
                    export_fog(&mut zipper, &points, fog);
                    for p in &points {
                        let fog_intensity = fog.get(p).cloned().unwrap_or(FogIntensity::TrueSight);
                        if fog_intensity < neutral_fog.get(p).cloned().unwrap_or(FogIntensity::TrueSight) {
                            self.export_field(&mut zipper, *p, fog_intensity);
                        }
                    }
                    for player in self.players.iter() {
                        if player.get_team() == ClientPerspective::Team(team) {
                            player.export(&mut zipper, false);
                        }
                    }
                    teams.insert(team, zipper.finish());
                }
            }

            game_interface::ExportedGame {
                public,
                hidden: Some(game_interface::ExportedGameHidden {
                    server,
                    teams,
                }),
            }
        } else {
            // no need to add fog info to the export
            let public = zipper.finish();
            game_interface::ExportedGame {
                public,
                hidden: None,
            }
        }
    }
    fn players(&self) -> Vec<game_interface::PlayerData> {
        self.players.iter()
        .map(|player| {
            game_interface::PlayerData {
                color_id: player.get_owner_id() as u8,
                team: match player.get_team() {
                    ClientPerspective::Team(team) => team,
                    _ => panic!("player should not be neutral"),
                },
                dead: player.dead,
            }
        }).collect()
    }
    fn current_turn(&self) -> usize {
        self.current_turn as usize
    }
    fn current_player(&self) -> game_interface::PlayerData {
        let player = self.current_player();
        game_interface::PlayerData {
            color_id: player.get_owner_id() as u8,
            team: match player.get_team() {
                ClientPerspective::Team(team) => team,
                _ => panic!("player should not be neutral"),
            },
            dead: player.dead,
        }
    }
    
    fn import_events(&self, bytes: Vec<u8>, version: Version) -> Result<Vec<Self::Event>, Self::ImportError> {
        Event::import_list(bytes, &self.environment, version)
    }

    fn get_config(&self) -> &Self::Config {
        &self.environment.config
    }

    fn get_name(&self) -> &str {
        self.environment.config.name()
    }

    fn get_version() -> Version {
        Version::parse(VERSION).expect(&format!("Cargo version has invalid format: {}", VERSION))
    }

    fn has_ended(&self) -> bool {
        self.ended
    }
}

