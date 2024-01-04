use crate::commander::Commander;
use crate::commander::commander_type::CommanderType;
use crate::config::Environment;
use crate::player::*;

use super::fog::FogMode;
use interfaces::map_interface::GameSettingsInterface;
use zipper::*;
use interfaces::game_interface;


#[derive(Debug, Clone)]
pub struct GameSettings {
    pub name: String,
    pub fog_mode: FogMode,
    pub players: Vec<PlayerSettings>,
}
impl GameSettingsInterface for GameSettings {
    fn players(&self) -> Vec<game_interface::PlayerData> {
        self.players.iter()
        .map(|p| {
            game_interface::PlayerData {
                color_id: p.owner_id,
                team: p.team,
                dead: false,
            }
        }).collect()
    }
    fn export(&self) -> Vec<u8> {
        let mut zipper = Zipper::new();
        Zippable::export(self, &mut zipper);
        zipper.finish()
    }
    fn import(data: Vec<u8>) -> Self {
        let mut unzipper = Unzipper::new(data);
        Zippable::import(&mut unzipper).unwrap() // TODO: remove unwrap! return Result instead
    }
}

#[derive(Debug, Clone)]
pub struct PlayerSettings {
    commander_options: Vec<CommanderType>,
    commander: CommanderType,
    funds: Funds,
    income: Income,
    team: u8,
    owner_id: u8,
}

impl PlayerSettings {
    pub fn new(owner_id: u8) -> Self {
        // TODO: validate input after importing. commander_options shouldn't contain the same commander multiple times
        let commander_options = CommanderType::list();
        let commander = commander_options.get(0).cloned().unwrap_or(CommanderType::None);
        Self {
            commander_options: commander_options.try_into().unwrap(),
            commander,
            income: 100.into(),
            funds: 0.into(),
            team: owner_id,
            owner_id,
        }
    }

    pub fn get_owner_id(&self) -> i8 {
        self.owner_id as i8
    }

    pub fn get_team(&self) -> u8 {
        self.team
    }

    pub fn get_commander_options(&self) -> &[CommanderType] {
        &self.commander_options
    }
    
    /*pub fn set_commander_options(&mut self, options: Vec<CommanderOption>) {
        // TODO
    }*/

    pub fn get_commander(&self) -> CommanderType {
        self.commander
    }

    pub fn set_commander(&mut self, commander: CommanderType) {
        self.commander = commander;
    }

    pub fn get_income(&self) -> i32 {
        *self.income
    }

    pub fn build(&self, environment: &Environment) -> Player {
        Player::new(self.owner_id, *self.funds, Commander::new(environment, self.commander))
    }
}
