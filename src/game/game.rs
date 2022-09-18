use std::collections::{HashMap, HashSet};

use zipper::*;
use zipper::zipper_derive::*;

use crate::map::map::*;
use crate::map::direction::*;
use crate::game::settings;
use crate::game::events;
use crate::map::point::Point;
use crate::player::*;
use crate::units::UnitType;
use crate::units::combat::ArmorType;
use crate::units::movement::Path;

#[derive(Debug, Clone, PartialEq)]
pub struct Game<D: Direction> {
    map: Map<D>,
    pub current_turn: u32,
    ended: bool,
    pub players: LVec<Player, 16>,
    fog_mode: FogMode,
    fog: HashMap<Option<Team>, HashSet<Point>>,
}
impl<D: Direction> Game<D> {
    fn new(map: Map<D>, settings: &settings::GameSettings) -> Self {
        let players = map.get_players();
        let mut fog = HashMap::new();
        let neutral_fog = HashSet::new();
        for player in &players {
            fog.insert(Some(player.team.clone()), neutral_fog.clone());
        }
        fog.insert(None, neutral_fog);
        Game {
            current_turn: 0,
            ended: false,
            players: players.try_into().unwrap(),
            map,
            fog_mode: settings.fog_mode.clone(),
            fog,
        }
    }
    pub fn new_server<R: Fn() -> f32>(map: Map<D>, settings: &settings::GameSettings, random: R) -> (Self, HashMap<Option<Perspective>, Vec<events::Event<D>>>) {
        let mut this = Self::new(map, settings);
        let events = this.start_server(random);
        (this, events)
    }
    pub fn new_client(map: Map<D>, settings: &settings::GameSettings, events: &Vec<events::Event<D>>) -> Self {
        let mut this = Self::new(map, settings);
        this.handle_events(events);
        this
    }
    fn start_server<R: Fn() -> f32>(&mut self, _random: R) -> HashMap<Option<Perspective>, Vec<events::Event<D>>> {
        let mut handler = events::EventHandler::new(self);
        if handler.get_game().fog_mode.is_foggy(0) {
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
    pub fn flip_fog_state(&mut self) {
        match &mut self.fog_mode {
            FogMode::Random(value, _, _, _) => {
                *value = !*value;
            }
            _ => {}
        }
    }
    pub fn recalculate_fog(&self, perspective: Perspective) -> HashSet<Point> {
        let mut fog = HashSet::new();
        if self.is_foggy() {
            for p in self.get_map().all_points() {
                fog.insert(p);
            }
            for p in self.get_map().all_points() {
                for p in self.get_map().get_terrain(&p).unwrap().get_vision(self, &p, perspective) {
                    fog.remove(&p);
                }
                if let Some(unit) = self.get_map().get_unit(&p) {
                    if perspective.is_some() && perspective == unit.get_owner().and_then(|owner| self.get_owning_player(owner)).and_then(|player| Some(player.team)) {
                        for p in unit.get_vision(self, &p) {
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
    pub fn current_turn(&self) -> u32 {
        self.current_turn
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
    pub fn get_owning_player(&self, owner: &Owner) -> Option<&Player> {
        self.players.iter().find(|player| &player.owner_id == owner)
    }
    pub fn get_owning_player_mut(&mut self, owner: &Owner) -> Option<&mut Player> {
        self.players.iter_mut().find(|player| &player.owner_id == owner)
    }
    pub fn get_team(&self, owner: Option<&Owner>) -> Option<Team> {
        owner.and_then(|o| self.get_owning_player(o)).and_then(|p| Some(p.team))
    }

    pub fn is_foggy(&self) -> bool {
        self.fog_mode.is_foggy(self.current_turn)
    }
    pub fn get_fog(&self) -> &HashMap<Option<Team>, HashSet<Point>> {
        &self.fog
    }
    pub fn get_fog_mut(&mut self) -> &mut HashMap<Option<Team>, HashSet<Point>> {
        &mut self.fog
    }
    pub fn has_vision_at(&self, team: Option<Team>, at: &Point) -> bool {
        !self.fog.get(&team).unwrap().contains(at)
    }

    pub fn handle_command<R: Fn() -> f32>(&mut self, command: events::Command<D>, random: R) -> Result<HashMap<Option<Perspective>, Vec<events::Event<D>>>, events::CommandError> {
        let mut handler = events::EventHandler::new(self);
        match command.convert(&mut handler, random) {
            Ok(()) => Ok(handler.accept()),
            Err(err) => {
                handler.cancel();
                Err(err)
            }
        }
    }
    pub fn handle_events(&mut self, events: &Vec<events::Event<D>>) {
        for event in events {
            event.apply(self);
        }
    }
    pub fn undo(&mut self, events: &Vec<events::Event<D>>) {
        for event in events.iter().rev() {
            event.undo(self);
        }
    }
    pub fn find_visible_threats(&self, pos: Point, threatened: &UnitType<D>, team: Team) -> HashSet<Point> {
        let mut result = HashSet::new();
        for p in self.map.all_points().into_iter().filter(|p| self.has_vision_at(Some(team), &p)) {
            if let Some(unit) = self.map.get_unit(&p) {
                if unit.threatens(self, threatened) && unit.shortest_path_to_attack(self, &Path::new(p), &pos).is_some() {
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

pub fn import_server<D: Direction>(data: interfaces::ExportedGame) -> Result<Game<D>, ZipperError> {
    if let Some(mut hidden_data) = data.hidden {
        let mut unzipper = Unzipper::new(hidden_data.server);
        let mut game = import_game_base(&mut unzipper, true)?;

        let points = game.map.all_points();
        game.fog.insert(None, import_fog(&mut unzipper, &points)?);
        
        for team in game.get_living_teams() {
            if let Some(data) = hidden_data.teams.remove(&*team) {
                let mut unzipper = Unzipper::new(data);
                game.fog.insert(Some(team), import_fog(&mut unzipper, &points)?);
            }
        }

        Ok(game)
    } else {
        let mut unzipper = Unzipper::new(data.public);
        let game = import_game_base(&mut unzipper, true)?;
        Ok(game)
    }
}

pub fn import_client<D: Direction>(public: Vec<u8>, team_view: Option<(Team, Vec<u8>)>) -> Result<Game<D>, ZipperError> {
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
        game.fog.insert(Some(team), fog);
        let mut players: Vec<Player> = vec![];
        for player in game.players.iter() {
            players.push(if player.team == team {
                Player::import(&mut unzipper, false)?
            } else {
                player.clone()
            });
        }
        game.players = players.try_into().unwrap();
    } else {
        game.fog.insert(None, neutral_fog);
    }
    Ok(game)
}

fn import_game_base<D: Direction>(unzipper: &mut Unzipper, is_server: bool) -> Result<Game<D>, ZipperError> {
    let map = Map::<D>::import_from_unzipper(unzipper)?;
    let current_turn = unzipper.read_u32(32)?;
    let ended = unzipper.read_bool()?;
    let fog_mode = FogMode::import(unzipper)?;
    let player_len = unzipper.read_u8(4)? + 1;
    let mut players = vec![];
    for _ in 0..player_len {
        players.push(Player::import(unzipper, !is_server && fog_mode.is_foggy(current_turn))?);
    }
    let mut fog = HashMap::new();
    let neutral_fog: HashSet<Point> = HashSet::new(); //map.all_points().into_iter().collect();
    for player in &players {
        fog.insert(Some(player.team.clone()), neutral_fog.clone());
    }
    fog.insert(None, neutral_fog);
    Ok(Game {
        map,
        current_turn,
        ended,
        fog_mode,
        players: players.try_into().unwrap(),
        fog,
    })
}

impl<D: Direction> interfaces::Game for Game<D> {
    fn export(&self) -> interfaces::ExportedGame {
        // server perspective
        let mut zipper = Zipper::new();
        self.map.export(&mut zipper, None);
        zipper.write_u32(self.current_turn, 32);
        zipper.write_bool(self.ended);
        self.fog_mode.export(&mut zipper);
        zipper.write_u8(self.players.len() as u8 - 1, 4);
        for player in self.players.iter() {
            player.export(&mut zipper, false);
        }

        if self.is_foggy() {

            let points = self.map.all_points();
            let neutral_fog = self.fog.get(&None).unwrap();
            // no need to export the teams' fog since it's exported below anyway.
            export_fog(&mut zipper, &points, neutral_fog);
            let server = zipper.finish();

            // "None" perspective, visible to all
            let mut zipper = Zipper::new();
            self.map.export(&mut zipper, Some(neutral_fog));
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
                if let Some(fog) = self.fog.get(&Some(team)) {
                    let mut zipper = Zipper::new();
                    export_fog(&mut zipper, &points, fog);
                    for p in &points {
                        if neutral_fog.contains(p) && !fog.contains(p) {
                            self.map.export_field(&mut zipper, p, false);
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

            interfaces::ExportedGame {
                public,
                hidden: Some(interfaces::ExportedGameHidden {
                    server,
                    teams,
                }),
            }
        } else {
            // no need to add fog info to the export
            let public = zipper.finish();
            interfaces::ExportedGame {
                public,
                hidden: None,
            }
        }
    }
}


#[derive(Debug, Clone, PartialEq, Zippable)]
#[zippable(bits = 3)]
pub enum FogMode {
    Never,
    Always,
    DarkRegular(U8::<255>, U8::<255>, U8::<255>),
    BrightRegular(U8::<255>, U8::<255>, U8::<255>),
    Random(bool, U8::<255>, U8::<240>, U8::<240>),
}
impl FogMode {
    pub fn is_foggy(&self, turn: u32) -> bool {
        match self {
            Self::Never => false,
            Self::Always => true,
            Self::DarkRegular(offset, bright, dark) => {
                if **offset as u32 > turn {
                    true
                } else {
                    ((turn - **offset as u32) % (**bright + **dark) as u32) >= **bright as u32
                }
            }
            Self::BrightRegular(offset, dark, bright) => {
                if **offset as u32 > turn {
                    false
                } else {
                    ((turn - **offset as u32) % (**dark + **bright) as u32) < **dark as u32
                }
            }
            Self::Random(value, _, _, _) => *value,
        }
    }
}
