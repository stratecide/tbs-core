use crate::map::map::*;
use crate::map::direction::*;
use crate::game::settings;
use crate::game::events;
use crate::map::point::Point;
use crate::player::*;

pub struct Game<D: Direction> {
    map: Map<D>,
    current_turn: u32,
    ended: bool,
    players: Vec<Player>,
}
impl<D: Direction> Game<D> {
    pub fn new_server(map: Map<D>, _settings: &settings::GameSettings) -> (Self, Vec<events::Event>) {
        let mut this = Game {
            current_turn: 0,
            ended: false,
            players: map.get_players(),
            map,
        };
        let events = this.start_server();
        (this, events)
    }
    fn start_server(&mut self) -> Vec<events::Event> {
        vec![]
    }
    pub fn new_client(map: Map<D>, events: &Vec<events::Event>) -> Self {
        let mut this = Game {
            current_turn: 0,
            ended: false,
            players: map.get_players(),
            map,
        };
        this.handle_events(events);
        this
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
    pub fn has_ended(&self) -> bool {
        self.ended
    }
    pub fn get_owning_player(&self, owner: &Owner) -> Option<&Player> {
        self.players.iter().find(|player| &player.owner_id == owner)
    }
    pub fn has_vision_at(&self, _player: &Player, _at: &Point) -> bool {
        // todo
        true
    }

    pub fn handle_command(&mut self, command: events::Command) -> Result<Vec<events::Event>, events::CommandError> {
        command.check_validity(self)?;
        let events = command.apply(self);
        self.handle_events(&events);
        Ok(events)
    }
    pub fn handle_events(&mut self, events: &Vec<events::Event>) {
        for event in events {
            event.apply(self);
        }
    }
    pub fn undo(&mut self, events: &Vec<events::Event>) {
        for event in events.iter().rev() {
            event.undo(self);
        }
    }
}

