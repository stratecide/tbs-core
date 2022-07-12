use std::collections::{HashMap, HashSet};

use crate::map::map::*;
use crate::map::direction::*;
use crate::game::settings;
use crate::game::events;
use crate::map::point::Point;
use crate::player::*;

#[derive(Clone)]
pub struct Game<D: Direction> {
    map: Map<D>,
    pub current_turn: u32,
    ended: bool,
    pub players: Vec<Player>,
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
            players,
            map,
            fog_mode: settings.fog_mode.clone(),
            fog,
        }
    }
    pub fn new_server(map: Map<D>, settings: &settings::GameSettings) -> (Self, HashMap<Option<Perspective>, Vec<events::Event<D>>>) {
        let mut this = Self::new(map, settings);
        let events = this.start_server();
        (this, events)
    }
    pub fn new_client(map: Map<D>, settings: &settings::GameSettings, events: &Vec<events::Event<D>>) -> Self {
        let mut this = Self::new(map, settings);
        this.handle_events(events);
        this
    }
    fn start_server(&mut self) -> HashMap<Option<Perspective>, Vec<events::Event<D>>> {
        let mut handler = events::EventHandler::new(self);
        if handler.get_game().is_foggy() {
            handler.recalculate_fog(false);
        }
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
            for p in self.get_map().wrapping_logic().pointmap().get_valid_points() {
                fog.insert(p);
            }
            for p in self.get_map().wrapping_logic().pointmap().get_valid_points() {
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
        for p in &self.players {
            result.insert(p.team.clone());
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

    pub fn handle_command(&mut self, command: events::Command<D>) -> Result<HashMap<Option<Perspective>, Vec<events::Event<D>>>, events::CommandError> {
        let mut handler = events::EventHandler::new(self);
        match command.convert(&mut handler) {
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
}


#[derive(Debug, Clone)]
pub enum FogMode {
    Never,
    Always,
    DarkRegular(u8, u8, u8),
    BrightRegular(u8, u8, u8),
    Random(bool, u8, f32, f32),
}
impl FogMode {
    pub fn is_foggy(&self, turn: u32) -> bool {
        match self {
            Self::Never => false,
            Self::Always => true,
            Self::DarkRegular(offset, bright, dark) => {
                if *offset as u32 > turn {
                    true
                } else {
                    ((turn - *offset as u32) % (bright + dark) as u32) >= *bright as u32
                }
            }
            Self::BrightRegular(offset, dark, bright) => {
                if *offset as u32 > turn {
                    false
                } else {
                    ((turn - *offset as u32) % (dark + bright) as u32) < *dark as u32
                }
            }
            Self::Random(value, _, _, _) => *value,
        }
    }
}
