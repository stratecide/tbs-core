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


#[derive(Debug, Clone, Eq)]
pub struct GameSettings {
    pub name: String, // should name even be part of the settings?
    pub fog_mode: FogMode,
    pub players: Vec<PlayerSettings>,
}

impl PartialEq for GameSettings {
    fn eq(&self, other: &Self) -> bool {
        self.fog_mode == other.fog_mode &&
        self.players == other.players
    }
}

impl GameSettings {
    pub fn start(&self) -> Self {
        let mut result = self.clone();
        for p in &mut result.players {
            p.commander_options.0 = Vec::new();
        }
        result
    }

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

#[derive(Debug, Clone, PartialEq, Eq)]
struct CommanderOptions(Vec<CommanderType>);
impl<'a> SupportedZippable<ConfigStarted<'a>> for CommanderOptions {
    fn export(&self, zipper: &mut Zipper, (config, started): (&'a Config, bool)) {
        if started {
            return;
        }
        for option in config.commander_types() {
            // could be made more efficient by assuming that commander types are sorted in self
            zipper.write_bool(self.0.contains(option));
        }
    }
    fn import(unzipper: &mut Unzipper, (config, started): (&'a Config, bool)) -> Result<Self, ZipperError> {
        if started {
            return Ok(Self(Vec::new()));
        }
        let mut result = Vec::new();
        for option in config.commander_types() {
            if unzipper.read_bool()? {
                result.push(*option);
            }
        }
        Ok(Self(result))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Zippable)]
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
    pub fn new(config: &Config, owner_id: u8) -> Self {
        let commander_options = config.commander_types();
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
    pub fn set_team(&mut self, team: u8) {
        self.team = team.into();
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
    pub fn set_income(&mut self, income: i32) {
        self.income = income.into();
    }

    pub fn get_funds(&self) -> i32 {
        *self.funds
    }
    pub fn set_funds(&mut self, funds: i32) {
        self.funds = funds.into();
    }

    pub fn build(&self, environment: &Environment) -> Player {
        Player::new(self.owner_id.0 as u8, *self.funds, Commander::new(environment, self.commander))
    }
}

#[cfg(test)]
mod tests {
    use semver::Version;
    use zipper::{SupportedZippable, Unzipper, Zipper};

    use crate::commander::commander_type::CommanderType;
    use crate::config::config::Config;
    use crate::game::fog::{FogMode, FogSetting};
    use crate::VERSION;

    use super::{GameSettings, PlayerSettings, CommanderOptions};

    #[test]
    fn export_commander_options() {
        let config = Config::test_config();
        let options = CommanderOptions(vec![CommanderType::None]);
        let co = CommanderType::None;
        let mut zipper = Zipper::new();
        options.export(&mut zipper, (&config, false));
        co.export(&mut zipper, &config);
        zipper.write_u8(1, 1);
        let data = zipper.finish();
        println!("export_commander_options: {data:?}");
        let mut unzipper = Unzipper::new(data, Version::parse(VERSION).unwrap());
        assert_eq!(Ok(options), CommanderOptions::import(&mut unzipper, (&config, false)));
        assert_eq!(Ok(co), CommanderType::import(&mut unzipper, &config));
        assert_eq!(1, unzipper.read_u8(1).unwrap())
    }

    #[test]
    fn export_settings() {
        let config = Config::test_config();
        let name = String::new();
        let setting = GameSettings {
            fog_mode: FogMode::Constant(FogSetting::Sharp(2)),
            name: name.clone(),
            players: vec![
                PlayerSettings::new(&config, 0),
                PlayerSettings::new(&config, 3),
            ],
        };
        for started in [false, true] {
            let mut zipper = Zipper::new();
            setting.export(&mut zipper, &config, started);
            zipper.write_u8(8, 4);
            let data = zipper.finish();
            let setting = if started {
                let mut setting = setting.clone();
                for player in &mut setting.players {
                    player.commander_options = CommanderOptions(Vec::new());
                }
                setting
            } else {
                setting.clone()
            };
            println!("{started}: {data:?}");
            let mut unzipper = Unzipper::new(data, Version::parse(VERSION).unwrap());
            assert_eq!(setting, GameSettings::import(&mut unzipper, &config, name.clone(), started).unwrap());
            assert_eq!(8, unzipper.read_u8(4).unwrap())
        }
    }
}
