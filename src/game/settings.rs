use std::collections::HashMap;

use crate::commanders::*;
use crate::map::direction::Direction;
use crate::map::map::Map;
use crate::player::*;

use super::game::{FogMode, Game};
use super::events::Event;
use interfaces::map_interface::GameSettingsInterface;
use zipper::*;
use zipper::{zipper_derive::*, LVec};
use interfaces::game_interface;


#[derive(Debug, Clone, Zippable)]
pub struct GameSettings {
    pub fog_mode: FogMode,
    pub players: LVec::<PlayerSettings, 16>,
}
impl GameSettingsInterface for GameSettings {
    fn players(&self) -> Vec<game_interface::PlayerData> {
        self.players.iter()
        .map(|p| {
            game_interface::PlayerData {
                color_id: *p.color_id,
                team: *p.team,
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

#[derive(Debug, Clone, Zippable)]
pub struct PlayerSettings {
    commander_options: LVec::<Commander, 255>,
    commander: Commander,
    pub funds: Funds,
    pub income: Income,
    pub team: Team,
    pub owner_id: Owner,
    pub color_id: U8::<15>,
}
impl PlayerSettings {
    pub fn new(owner_id: Owner) -> Self {
        // TODO: validate input after importing. commander_options shouldn't contain the same commander multiple times
        let commander_options = Commander::list_all();
        let commander = commander_options.get(0).cloned().unwrap_or(Commander::None);
        Self {
            commander_options: commander_options.try_into().unwrap(),
            commander,
            income: Income::new(100),
            funds: Funds::new(0),
            team: Team::new(*owner_id),
            owner_id,
            color_id: owner_id,
        }
    }
    pub fn get_commander_options(&self) -> &LVec<Commander, 255> {
        &self.commander_options
    }
    /*pub fn set_commander_options(&mut self, options: Vec<CommanderOption>) {
        // TODO
    }*/
    pub fn get_commander(&self) -> &Commander {
        &self.commander
    }
    pub fn set_commander(&mut self, co: Commander) {
        self.commander = co;
    }
    pub fn build(&self) -> Player {
        Player {
            commander: self.commander.clone(),
            funds: self.funds,
            income: self.income,
            team: self.team,
            dead: false,
            color_id: *self.color_id,
            owner_id: self.owner_id,
        }
    }
}
