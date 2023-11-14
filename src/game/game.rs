use std::collections::{HashMap, HashSet};

use zipper::*;
use interfaces::game_interface::{self, Events, ClientPerspective};
use interfaces::game_interface::GameInterface;

use crate::map::map::*;
use crate::map::direction::*;
use crate::game::settings;
use crate::game::events;
use crate::game::fog::*;
use crate::map::point::Point;
use crate::player::*;
use crate::terrain::Terrain;
use crate::units::UnitType;
use crate::units::mercenary::MercenaryOption;
use crate::units::movement::Path;

use super::{event_handler, commands};

#[derive(Debug, Clone, PartialEq)]
pub struct Game<D: Direction> {
    map: Map<D>,
    pub current_turn: u32,
    ended: bool,
    pub players: LVec<Player, 16>,
    fog_mode: FogMode,
    fog: HashMap<ClientPerspective, HashMap<Point, FogIntensity>>,
}

impl<D: Direction> Game<D> {
    fn new(map: Map<D>, settings: &settings::GameSettings) -> Self {
        let players: Vec<Player> = settings.players.iter()
            .map(|player| player.build())
            .collect();
        Game {
            fog: create_base_fog(&map, &players),
            current_turn: 0,
            ended: false,
            players: players.try_into().unwrap(),
            map,
            fog_mode: settings.fog_mode.clone(),
        }
    }

    pub fn new_server<R: 'static + Fn() -> f32>(map: Map<D>, settings: &settings::GameSettings, random: R) -> (Self, Events<Self>) {
        let mut this = Self::new(map, settings);
        let events = this.start_server(random);
        (this, events)
    }

    pub fn new_client(map: Map<D>, settings: &settings::GameSettings, events: &Vec<events::Event<D>>) -> Self {
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

    pub fn recalculate_fog(&self, perspective: Perspective) -> HashMap<Point, FogIntensity> {
        let mut fog = HashMap::new();
        let strongest_intensity = self.fog_mode.fog_setting(self.current_turn as usize, self.players.len()).intensity();
        for p in self.get_map().all_points() {
            fog.insert(p, strongest_intensity);
        }
        if !self.is_foggy() {
            return fog;
        }
        for p in self.get_map().all_points() {
            for (p, v) in self.get_map().get_terrain(p).unwrap().get_vision(self, p, perspective) {
                fog.insert(p, v.min(fog.get(&p).clone().unwrap().clone()));
            }
            if let Some(unit) = self.get_map().get_unit(p) {
                if perspective.is_some() && perspective == unit.get_owner().and_then(|owner| self.get_owning_player(owner)).and_then(|player| Some(player.team)) {
                    for (p, v) in unit.get_vision(self, p) {
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

    pub fn get_teams(&self) -> HashSet<Team> {
        let mut result = HashSet::new();
        for p in self.players.iter() {
            result.insert(p.team);
        }
        result
    }

    pub fn get_living_teams(&self) -> HashSet<Team> {
        let mut result = HashSet::new();
        for p in self.players.iter() {
            if !p.dead {
                result.insert(p.team);
            }
        }
        result
    }

    pub fn is_team_alive(&self, team: &Team) -> bool {
        self.get_living_teams().contains(team)
    }

    pub fn has_ended(&self) -> bool {
        self.ended
    }

    pub fn get_owning_player(&self, owner: Owner) -> Option<&Player> {
        self.players.iter().find(|player| player.owner_id == owner)
    }

    pub fn get_owning_player_mut(&mut self, owner: Owner) -> Option<&mut Player> {
        self.players.iter_mut().find(|player| player.owner_id == owner)
    }

    pub fn get_team(&self, owner: Option<Owner>) -> ClientPerspective {
        owner.and_then(|o| self.get_owning_player(o)).and_then(|p| Some(ClientPerspective::Team(*p.team as u8))).unwrap_or(ClientPerspective::Neutral)
    }

    pub fn is_foggy(&self) -> bool {
        self.fog_mode.is_foggy(self.current_turn as usize, self.players.len())
    }

    pub fn will_be_foggy(&self, turns_later: usize) -> bool {
        self.fog_mode.is_foggy(self.current_turn as usize + turns_later, self.players.len())
    }

    pub fn get_fog(&self) -> &HashMap<ClientPerspective, HashMap<Point, FogIntensity>> {
        &self.fog
    }

    pub fn get_fog_at(&self, team: ClientPerspective, position: Point) -> FogIntensity {
        self.fog.get(&team).and_then(|fog| fog.get(&position)).cloned().unwrap_or(FogIntensity::TrueSight)
    }

    pub fn can_see_unit_at(&self, team: ClientPerspective, position: Point, unit: &UnitType<D>, accept_unknowns: bool) -> bool {
        match unit.fog_replacement(self.map.get_terrain(position).expect(&format!("No terrain at {position:?}")), self.get_fog_at(team, position)) {
            None => false,
            Some(UnitType::Unknown) => accept_unknowns,
            Some(_) => true,
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
    
    pub fn available_mercs(&self, player: &Player) -> Vec<MercenaryOption> {
        let mut used = HashSet::new();
        for p in self.map.all_points() {
            if let Some(unit) = self.map.get_unit(p) {
                if unit.get_owner() == Some(player.owner_id) {
                    unit.update_used_mercs(&mut used);
                }
            }
        }
        vec![MercenaryOption::EarlGrey]
        .into_iter()
        .filter(|m| !used.contains(m))
        .collect()
    }
    
    pub fn can_buy_merc_at(&self, player: &Player, pos: Point) -> bool {
        if self.map.get_terrain(pos) == Some(&Terrain::Tavern) {
            for p in self.map.all_points() {
                if let Some(unit) = self.map.get_unit(p) {
                    if unit.get_owner() == Some(player.owner_id) {
                        // check if unit is mercenary or transports a mercenary
                        match unit {
                            UnitType::Normal(unit) => {
                                if unit.data.mercenary.get_origin() == Some(pos) {
                                    return false;
                                }
                            }
                            _ => {}
                        }
                        for unit in unit.get_boarded() {
                            if unit.data.mercenary.get_origin() == Some(pos) {
                                return false;
                            }
                        }
                    }
                }
            }
            true
        } else {
            false
        }
    }

    pub fn undo(&mut self, events: &Vec<events::Event<D>>) {
        for event in events.iter().rev() {
            event.undo(self);
        }
    }

    pub fn find_visible_threats(&self, pos: Point, threatened: &UnitType<D>, team: ClientPerspective) -> HashSet<Point> {
        let mut result = HashSet::new();
        for p in self.map.all_points() {
            if let Some(unit) = self.map.get_unit(p) {
                if self.can_see_unit_at(team, p, unit, false) && unit.threatens(self, threatened, pos) && unit.shortest_path_to_attack(self, &Path::new(p), pos).is_some() {
                    result.insert(p);
                }
            }
        }
        result
    }
}

fn export_fog(zipper: &mut Zipper, points: &Vec<Point>, fog: &HashMap<Point, FogIntensity>) {
    for p in points {
        let intensity = fog.get(&p).cloned().unwrap_or(FogIntensity::TrueSight);
        intensity.export(zipper);
    }
}

fn import_fog(unzipper: &mut Unzipper, points: &Vec<Point>) -> Result<HashMap<Point, FogIntensity>, ZipperError> {
    let mut result = HashMap::new();
    for p in points {
        let intensity = FogIntensity::import(unzipper)?;
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
        if !fog.contains_key(&ClientPerspective::Team(*player.team as u8)) {
            fog.insert(ClientPerspective::Team(*player.team as u8), neutral_fog.clone());
        }
    }
    fog.insert(ClientPerspective::Neutral, neutral_fog);
    fog
}

fn import_game_base<D: Direction>(unzipper: &mut Unzipper, is_server: bool) -> Result<Game<D>, ZipperError> {
    let map = Map::<D>::import_from_unzipper(unzipper)?;
    let current_turn = unzipper.read_u32(32)?;
    let ended = unzipper.read_bool()?;
    let fog_mode = FogMode::import(unzipper)?;
    let player_len = unzipper.read_u8(4)? + 1;
    let mut players = vec![];
    for _ in 0..player_len {
        players.push(Player::import(unzipper, !is_server && fog_mode.is_foggy(current_turn as usize, player_len as usize))?);
    }
    Ok(Game {
        fog: create_base_fog(&map, &players),
        map,
        current_turn,
        ended,
        fog_mode,
        players: players.try_into().unwrap(),
    })
}

impl<D: Direction> game_interface::GameInterface for Game<D> {
    type Event = events::Event<D>;
    type Command = commands::Command<D>;
    type CommandError = commands::CommandError;
    type ImportError = ZipperError;

    fn import_server(data: game_interface::ExportedGame) -> Result<Box<Self>, ZipperError> {
        if let Some(mut hidden_data) = data.hidden {
            let mut unzipper = Unzipper::new(hidden_data.server);
            let mut game = import_game_base(&mut unzipper, true)?;

            let points = game.map.all_points();
            game.fog.insert(ClientPerspective::Neutral, import_fog(&mut unzipper, &points)?);
            
            for team in game.get_living_teams() {
                if let Some(data) = hidden_data.teams.remove(&(*team as u8)) {
                    let mut unzipper = Unzipper::new(data);
                    game.fog.insert(ClientPerspective::Team(*team as u8), import_fog(&mut unzipper, &points)?);
                }
            }

            Ok(Box::new(game))
        } else {
            let mut unzipper = Unzipper::new(data.public);
            let game = import_game_base(&mut unzipper, true)?;
            Ok(Box::new(game))
        }
    }

    fn import_client(public: Vec<u8>, team_view: Option<(u8, Vec<u8>)>) -> Result<Box<Game<D>>, ZipperError> {
        let mut unzipper = Unzipper::new(public);
        let mut game = import_game_base(&mut unzipper, false)?;
        let points = game.map.all_points();
        let neutral_fog = if game.is_foggy() {
            import_fog(&mut unzipper, &points)?
        } else {
            HashMap::new()
        };
        if let Some((team, team_view)) = team_view {
            let mut unzipper = Unzipper::new(team_view);
            let fog = import_fog(&mut unzipper, &points)?;
            for p in &points {
                if fog.get(p).cloned().unwrap_or(FogIntensity::TrueSight) < neutral_fog.get(p).cloned().unwrap_or(FogIntensity::TrueSight) {
                    let field_data = FieldData::import(&mut unzipper)?;
                    game.map.set_terrain(p.clone(), field_data.terrain);
                    game.map.set_details(p.clone(), field_data.details.to_vec());
                    game.map.set_unit(p.clone(), field_data.unit);
                }
            }
            game.fog.insert(ClientPerspective::Team(team), fog);
            let mut players: Vec<Player> = vec![];
            for player in game.players.iter() {
                players.push(if *player.team as u8 == team {
                    Player::import(&mut unzipper, false)?
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
        match command.convert(&mut handler) {
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
        self.map.zip(&mut zipper, None);
        zipper.write_u32(self.current_turn, 32);
        zipper.write_bool(self.ended);
        self.fog_mode.export(&mut zipper);
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
            self.map.zip(&mut zipper, Some(neutral_fog));
            zipper.write_u32(self.current_turn, 32);
            zipper.write_bool(self.ended);
            self.fog_mode.export(&mut zipper);
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
                if let Some(fog) = self.fog.get(&ClientPerspective::Team(*team as u8)) {
                    let mut zipper = Zipper::new();
                    export_fog(&mut zipper, &points, fog);
                    for p in &points {
                        let fog_intensity = fog.get(p).cloned().unwrap_or(FogIntensity::TrueSight);
                        if fog_intensity < neutral_fog.get(p).cloned().unwrap_or(FogIntensity::TrueSight) {
                            self.map.export_field(&mut zipper, *p, fog_intensity);
                        }
                    }
                    for player in self.players.iter() {
                        if player.team == team {
                            player.export(&mut zipper, false);
                        }
                    }
                    teams.insert(*team as u8, zipper.finish());
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
        .map(|p| {
            game_interface::PlayerData {
                color_id: p.color_id,
                team: *p.team as u8,
                dead: p.dead,
            }
        }).collect()
    }
    fn current_turn(&self) -> usize {
        self.current_turn as usize
    }
    fn current_player(&self) -> game_interface::PlayerData {
        let player = self.current_player();
        game_interface::PlayerData {
            color_id: player.color_id,
            team: *player.team as u8,
            dead: player.dead,
        }
    }
}

