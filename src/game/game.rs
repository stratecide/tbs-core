use std::collections::{HashMap, HashSet};
use std::fmt::Display;

use zipper::*;
use zipper::zipper_derive::*;
use interfaces::game_interface::{self, Events, ClientPerspective};
use interfaces::game_interface::GameInterface;

use crate::map::map::*;
use crate::map::direction::*;
use crate::game::settings;
use crate::game::events;
use crate::map::point::Point;
use crate::player::*;
use crate::terrain::Terrain;
use crate::units::UnitType;
use crate::units::mercenary::MercenaryOption;
use crate::units::movement::Path;

#[derive(Debug, Clone, PartialEq)]
pub struct Game<D: Direction> {
    map: Map<D>,
    pub current_turn: u32,
    ended: bool,
    pub players: LVec<Player, 16>,
    fog_mode: FogMode,
    fog: HashMap<ClientPerspective, HashSet<Point>>,
}
impl<D: Direction> Game<D> {
    fn new(map: Map<D>, settings: &settings::GameSettings) -> Self {
        let players: Vec<Player> = settings.players.iter()
            .map(|player| player.build())
            .collect();
        let mut fog = HashMap::new();
        let neutral_fog = HashSet::new();
        for player in &players {
            if !fog.contains_key(&ClientPerspective::Team(*player.team)) {
                fog.insert(ClientPerspective::Team(*player.team), neutral_fog.clone());
            }
        }
        fog.insert(ClientPerspective::Neutral, neutral_fog);
        Game {
            current_turn: 0,
            ended: false,
            players: players.try_into().unwrap(),
            map,
            fog_mode: settings.fog_mode.clone(),
            fog,
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
        let mut handler = events::EventHandler::new(self, Box::new(random));
        FogMode::forecast(&mut handler);
        if handler.get_game().is_foggy() {
            // TODO: this is duplicated code from EndTurn in events
            let mut events: Vec<events::Event<D>> = vec![];
            for player in handler.get_game().players.iter() {
                events.push(events::Event::PureHideFunds(player.owner_id));
            }
            for event in events {
                handler.add_event(event);
            }
        }
        handler.start_turn();
        handler.accept()
    }
    pub fn get_fog_mode(&self) -> &FogMode {
        &self.fog_mode
    }
    pub fn get_fog_mode_mut(&mut self) -> &mut FogMode {
        &mut self.fog_mode
    }
    pub fn recalculate_fog(&self, perspective: Perspective) -> HashSet<Point> {
        let mut fog = HashSet::new();
        if self.is_foggy() {
            for p in self.get_map().all_points() {
                fog.insert(p);
            }
            for p in self.get_map().all_points() {
                for p in self.get_map().get_terrain(p).unwrap().get_vision(self, p, perspective) {
                    fog.remove(&p);
                }
                if let Some(unit) = self.get_map().get_unit(p) {
                    if perspective.is_some() && perspective == unit.get_owner().and_then(|owner| self.get_owning_player(owner)).and_then(|player| Some(player.team)) {
                        for p in unit.get_vision(self, p) {
                            fog.remove(&p);
                        }
                    }
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
        owner.and_then(|o| self.get_owning_player(o)).and_then(|p| Some(ClientPerspective::Team(*p.team))).unwrap_or(ClientPerspective::Neutral)
    }

    pub fn is_foggy(&self) -> bool {
        self.fog_mode.is_foggy(self.current_turn, 0).expect("the game should always know whether it's currently foggy")
    }
    pub fn will_be_foggy(&self, turns_later: usize) -> Option<bool> {
        self.fog_mode.is_foggy(self.current_turn, turns_later)
    }
    pub fn get_fog(&self) -> &HashMap<ClientPerspective, HashSet<Point>> {
        &self.fog
    }
    pub fn get_fog_mut(&mut self) -> &mut HashMap<ClientPerspective, HashSet<Point>> {
        &mut self.fog
    }
    pub fn has_vision_at(&self, team: ClientPerspective, at: Point) -> bool {
        !self.fog.get(&team).unwrap().contains(&at)
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
    pub fn find_visible_threats(&self, pos: Point, threatened: &UnitType<D>, team: Team) -> HashSet<Point> {
        let mut result = HashSet::new();
        for p in self.map.all_points().into_iter().filter(|p| self.has_vision_at(ClientPerspective::Team(*team), *p)) {
            if let Some(unit) = self.map.get_unit(p) {
                if unit.threatens(self, threatened) && unit.shortest_path_to_attack(self, &Path::new(p), pos).is_some() {
                    result.insert(p);
                }
            }
        }
        result
    }
}

fn export_fog(zipper: &mut Zipper, points: &Vec<Point>, fog: &HashSet<Point>) {
    for p in points {
        zipper.write_bool(fog.contains(p));
    }
}

fn import_fog(unzipper: &mut Unzipper, points: &Vec<Point>) -> Result<HashSet<Point>, ZipperError> {
    let mut result = HashSet::new();
    for p in points {
        if unzipper.read_bool()? {
            result.insert(p.clone());
        }
    }
    Ok(result)
}

fn import_game_base<D: Direction>(unzipper: &mut Unzipper, is_server: bool) -> Result<Game<D>, ZipperError> {
    let map = Map::<D>::import_from_unzipper(unzipper)?;
    let current_turn = unzipper.read_u32(32)?;
    let ended = unzipper.read_bool()?;
    let fog_mode = FogMode::import(unzipper)?;
    let player_len = unzipper.read_u8(4)? + 1;
    let mut players = vec![];
    for _ in 0..player_len {
        players.push(Player::import(unzipper, !is_server && fog_mode.is_foggy(current_turn, 0).ok_or(ZipperError::InconsistentData)?)?);
    }
    let mut fog = HashMap::new();
    let neutral_fog: HashSet<Point> = HashSet::new(); //map.all_points().into_iter().collect();
    for player in &players {
        fog.insert(ClientPerspective::Team(*player.team.clone()), neutral_fog.clone());
    }
    fog.insert(ClientPerspective::Neutral, neutral_fog);
    Ok(Game {
        map,
        current_turn,
        ended,
        fog_mode,
        players: players.try_into().unwrap(),
        fog,
    })
}

impl<D: Direction> game_interface::GameInterface for Game<D> {
    type Event = events::Event<D>;
    type Command = events::Command<D>;
    type CommandError = events::CommandError;
    type ImportError = ZipperError;

    fn import_server(data: game_interface::ExportedGame) -> Result<Box<Self>, ZipperError> {
        if let Some(mut hidden_data) = data.hidden {
            let mut unzipper = Unzipper::new(hidden_data.server);
            let mut game = import_game_base(&mut unzipper, true)?;

            let points = game.map.all_points();
            game.fog.insert(ClientPerspective::Neutral, import_fog(&mut unzipper, &points)?);
            
            for team in game.get_living_teams() {
                if let Some(data) = hidden_data.teams.remove(&*team) {
                    let mut unzipper = Unzipper::new(data);
                    game.fog.insert(ClientPerspective::Team(*team), import_fog(&mut unzipper, &points)?);
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
            let fog = import_fog(&mut unzipper, &points)?;
            fog
        } else {
            HashSet::new()
        };

        if let Some((team, team_view)) = team_view {
            let mut unzipper = Unzipper::new(team_view);
            let fog = import_fog(&mut unzipper, &points)?;
            for p in &points {
                if neutral_fog.contains(p) && !fog.contains(p) {
                    let field_data = FieldData::import(&mut unzipper)?;
                    game.map.set_terrain(p.clone(), field_data.terrain);
                    game.map.set_details(p.clone(), field_data.details.to_vec());
                    game.map.set_unit(p.clone(), field_data.unit);
                }
            }
            game.fog.insert(ClientPerspective::Team(team), fog);
            let mut players: Vec<Player> = vec![];
            for player in game.players.iter() {
                players.push(if *player.team == team {
                    Player::import(&mut unzipper, false)?
                } else {
                    player.clone()
                });
            }
            game.players = players.try_into().unwrap();
        } else {
            game.fog.insert(ClientPerspective::Neutral, neutral_fog);
        }
        Ok(Box::new(game))
    }

    fn handle_command<R: 'static + Fn() -> f32>(&mut self, command: events::Command<D>, random: R) -> Result<Events<Self>, events::CommandError> {
        let mut handler = events::EventHandler::new(self, Box::new(random));
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
            let neutral_fog = self.fog.get(&ClientPerspective::Neutral).unwrap();
            // no need to export the teams' fog since it's exported below anyway.
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
            
            let mut teams = HashMap::new();
            for team in self.get_living_teams() {
                // team perspective, one per team
                if let Some(fog) = self.fog.get(&ClientPerspective::Team(*team)) {
                    let mut zipper = Zipper::new();
                    export_fog(&mut zipper, &points, fog);
                    for p in &points {
                        if neutral_fog.contains(p) && !fog.contains(p) {
                            self.map.export_field(&mut zipper, *p, false);
                        }
                    }
                    for player in self.players.iter() {
                        if player.team == team {
                            player.export(&mut zipper, false);
                        }
                    }
                    teams.insert(*team, zipper.finish());
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
                team: *p.team,
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
            team: *player.team,
            dead: player.dead,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Zippable)]
pub struct FogChangeChance (U8::<240>);
impl FogChangeChance {
    pub fn new(chance: f32) -> Self {
        Self (
            U8::new((chance.max(0.).min(1.) * 240.) as u8)
        )
    }
    pub fn check(&self, value: f32) -> bool {
        value < self.get_chance()
    }
    pub fn get_chance(&self) -> f32 {
        (*self.0 as f32) / 240.
    }
}
impl Display for FogChangeChance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&format!("{} %", (self.get_chance() * 100.).round() as u8))
    }
}

pub const MAX_FOG_FORECAST_TURNS: u32 = 255 + 33;

#[derive(Debug, Clone, PartialEq, Zippable)]
#[zippable(bits = 3)]
pub enum FogMode {
    Never,
    Always,
    DarkRegular(U8::<255>, U8::<255>, U8::<255>),
    BrightRegular(U8::<255>, U8::<255>, U8::<255>),
    Random(FogChangeChance, FogChangeChance, U8::<255>, LVec::<bool, {MAX_FOG_FORECAST_TURNS}>),
}

impl Display for FogMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Always => write!(f, "Night Only"),
            Self::Never => write!(f, "Day Only"),
            Self::Random(_, _, _, _) => write!(f, "Random"),
            Self::BrightRegular(_, _, _) => write!(f, "Regular (Day)"),
            Self::DarkRegular(_, _, _) => write!(f, "Regular (Night)"),
        }
    }
}

impl FogMode {
    pub fn forecast<D: Direction>(handler: &mut events::EventHandler<D>) {
        loop {
            match &handler.get_game().fog_mode {
                FogMode::Random(to_bright_chance, to_dark_chance, _, forecast) => {
                    if forecast.len() >= handler.get_game().players.len() * 2 + 1 {
                        break;
                    }
                    let next_value = if *forecast.last().unwrap_or(&false) {
                        to_bright_chance.check(handler.rng())
                    } else {
                        to_dark_chance.check(handler.rng())
                    };
                    handler.add_event(events::Event::RandomFogForecast(next_value));
                }
                _ => break,
            }
        }
    }

    fn is_foggy(&self, current_turn: u32, additional_turns: usize) -> Option<bool> {
        let turn = current_turn + additional_turns as u32;
        match self {
            Self::Never => Some(false),
            Self::Always => Some(true),
            Self::DarkRegular(offset, bright, dark) => {
                if **offset as u32 > turn {
                    Some(true)
                } else {
                    Some(((turn - **offset as u32) % (**bright + **dark) as u32) >= **bright as u32)
                }
            }
            Self::BrightRegular(offset, dark, bright) => {
                if **offset as u32 > turn {
                    Some(false)
                } else {
                    Some(((turn - **offset as u32) % (**dark + **bright) as u32) < **dark as u32)
                }
            }
            Self::Random(_, _, _, forecast) => forecast.get(additional_turns).cloned(),
        }
    }
}
