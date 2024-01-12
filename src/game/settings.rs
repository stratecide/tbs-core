use std::sync::Arc;

use crate::commander::Commander;
use crate::commander::commander_type::CommanderType;
use crate::config::environment::Environment;
use crate::config::config::Config;
use crate::map::direction::Direction;
use crate::player::*;

use super::fog::FogMode;
use super::game::Game;
use interfaces::map_interface::GameSettingsInterface;
use semver::Version;
use zipper::*;
use interfaces::game_interface;
use zipper_derive::Zippable;


#[derive(Debug, Clone)]
pub struct GameSettings {
    pub name: String,
    pub fog_mode: FogMode,
    pub players: Vec<PlayerSettings>,
}

impl GameSettings {
    pub fn import(unzipper: &mut Unzipper, config: &Config, name: String, started: bool) -> Result<Self, ZipperError> {
        let fog_mode = FogMode::unzip(unzipper)?;
        let mut players = Vec::new();
        for _ in 0..unzipper.read_u8(bits_needed_for_max_value(config.max_player_count() as u32 - 1))? + 1 {
            players.push(PlayerSettings::import(unzipper, (config, started))?);
        }
        Ok(Self {
            name,
            fog_mode,
            players,
        })
    }
    pub fn export(&self, zipper: &mut Zipper, config: &Config, started: bool) {
        self.fog_mode.zip(zipper);
        zipper.write_u8((self.players.len() - 1) as u8, bits_needed_for_max_value(config.max_player_count() as u32 - 1));
        for p in &self.players {
            p.export(zipper, (config, started));
        }
    }
}

impl<D: Direction> GameSettingsInterface<Game<D>> for GameSettings {
    fn players(&self) -> Vec<game_interface::PlayerData> {
        self.players.iter()
        .map(|p| {
            game_interface::PlayerData {
                color_id: p.owner_id.0 as u8,
                team: p.team.0,
                dead: false,
            }
        }).collect()
    }
    fn export(&self, config: &Arc<Config>) -> Vec<u8> {
        let mut zipper = Zipper::new();
        self.export(&mut zipper, config, false);
        zipper.finish()
    }
    fn import(data: Vec<u8>, config: &Arc<Config>, name: String, version: Version) -> Result<Self, ZipperError> {
        let mut unzipper = Unzipper::new(data, version);
        Self::import(&mut unzipper, config, name, false)
    }
}

type ConfigStarted<'a> = (&'a Config, bool);

#[derive(Debug, Clone)]
struct CommanderOptions(Vec<CommanderType>);
impl<'a> SupportedZippable<ConfigStarted<'a>> for CommanderOptions {
    fn export(&self, zipper: &mut Zipper, support: (&'a Config, bool)) {
        if support.1 {
            return;
        }
        for option in support.0.commander_types() {
            // could be made more efficient by assuming that commander types are sorted in self
            zipper.write_bool(self.0.contains(option));
        }
    }
    fn import(unzipper: &mut Unzipper, support: (&'a Config, bool)) -> Result<Self, ZipperError> {
        if support.1 {
            return Ok(Self(Vec::new()));
        }
        let mut result = Vec::new();
        for option in support.0.commander_types() {
            if unzipper.read_bool()? {
                result.push(*option);
            }
        }
        Ok(Self(result))
    }
}

#[derive(Debug, Clone, Zippable)]
#[zippable(support = ConfigStarted::<'_>)]
pub struct PlayerSettings {
    commander_options: CommanderOptions,
    #[supp(support.0)]
    commander: CommanderType,
    funds: Funds,
    income: Income,
    #[supp(support.0)]
    team: Team,
    #[supp(support.0)]
    owner_id: Owner,
}

impl PlayerSettings {
    pub fn new(owner_id: u8) -> Self {
        let commander_options = CommanderType::list();
        let commander = commander_options.get(0).cloned().unwrap_or(CommanderType::None);
        Self {
            commander_options: CommanderOptions(commander_options.try_into().unwrap()),
            commander,
            income: 100.into(),
            funds: 0.into(),
            team: Team(owner_id),
            owner_id: Owner(owner_id as i8),
        }
    }

    pub fn get_owner_id(&self) -> i8 {
        self.owner_id.0
    }

    pub fn get_team(&self) -> u8 {
        self.team.0
    }

    pub fn get_commander_options(&self) -> &[CommanderType] {
        &self.commander_options.0
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
        Player::new(self.owner_id.0 as u8, *self.funds, Commander::new(environment, self.commander))
    }
}
