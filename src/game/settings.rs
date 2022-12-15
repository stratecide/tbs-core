use crate::commanders::*;
use crate::player::*;

use super::game::FogMode;
use zipper::*;
use zipper::{zipper_derive::*, LVec};


#[derive(Debug, Clone, Zippable)]
pub struct GameSettings {
    pub fog_mode: FogMode,
    pub players: LVec::<PlayerSettings, 16>,
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
