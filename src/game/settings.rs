use std::fmt::{Debug, Display};
use std::sync::Arc;

use crate::commander::Commander;
use crate::commander::commander_type::CommanderType;
use crate::config::environment::Environment;
use crate::config::config::Config;
use crate::{player::*, VERSION};

use super::fog::FogMode;
use interfaces::map_interface::GameSettingsInterface;
use interfaces::{PlayerMeta, RandomFn};
use semver::Version;
use zipper::*;
use zipper_derive::Zippable;


#[derive(Clone)]
pub struct GameConfig {
    pub config: Arc<Config>,
    pub fog_mode: FogMode,
    pub players: Vec<PlayerConfig>,
}

impl GameConfig {
    pub fn build(&self, player_selections: &[PlayerSelectedOptions], random: &RandomFn) -> GameSettings {
        GameSettings {
            config: self.config.clone(),
            fog_mode: self.fog_mode.clone(),
            players: self.players.iter()
                .enumerate()
                .map(|(i, p)| p.build(&player_selections[i], random))
                .collect(),
        }
    }

    pub fn build_default(&self) -> GameSettings {
        let player_selections: Vec<_> = (0..self.players.len()).map(|_| PlayerSelectedOptions {
            commander: None,
        }).collect();
        let random: RandomFn = Arc::new(|| 0.);
        Self::build(&self, &player_selections, &random)
    }

    pub fn import(config: Arc<Config>, bytes: Vec<u8>) -> Result<Self, ZipperError> {
        let mut unzipper = Unzipper::new(bytes, Version::parse(VERSION).unwrap());
        let fog_mode = FogMode::unzip(&mut unzipper)?;
        let mut players = Vec::new();
        for _ in 0..unzipper.read_u8(bits_needed_for_max_value(config.max_player_count() as u32 - 1))? + 1 {
            players.push(PlayerConfig::import(&mut unzipper, &config)?);
        }
        Ok(Self {
            config,
            fog_mode,
            players,
        })
    }
}

impl Debug for GameConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GameConfig")
        .field("fog_mode", &self.fog_mode)
        .field("players", &self.players)
        .finish()
    }
}

impl PartialEq for GameConfig {
    fn eq(&self, other: &Self) -> bool {
        self.fog_mode == other.fog_mode &&
        self.players == other.players
    }
}

#[derive(Clone)]
pub struct GameSettings {
    pub config: Arc<Config>,
    pub fog_mode: FogMode,
    pub players: Vec<PlayerSettings>,
}

impl Debug for GameSettings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GameSettings")
        .field("fog_mode", &self.fog_mode)
        .field("players", &self.players)
        .finish()
    }
}

impl PartialEq for GameSettings {
    fn eq(&self, other: &Self) -> bool {
        self.fog_mode == other.fog_mode &&
        self.players == other.players
    }
}

impl GameSettings {
    pub fn import(unzipper: &mut Unzipper, config: Arc<Config>) -> Result<Self, ZipperError> {
        let fog_mode = FogMode::unzip(unzipper)?;
        let mut players = Vec::new();
        for _ in 0..unzipper.read_u8(bits_needed_for_max_value(config.max_player_count() as u32 - 1))? + 1 {
            players.push(PlayerSettings::import(unzipper, &config)?);
        }
        Ok(Self {
            config,
            fog_mode,
            players,
        })
    }

    pub fn export(&self, zipper: &mut Zipper) {
        self.fog_mode.zip(zipper);
        zipper.write_u8((self.players.len() - 1) as u8, bits_needed_for_max_value(self.config.max_player_count() as u32 - 1));
        for p in &self.players {
            p.export(zipper, &self.config);
        }
    }
}

#[derive(Debug)]
pub enum PlayerSettingError {
    PlayerIndex(usize, usize),
    Commander(usize, CommanderType),
    PlayerCount(usize, usize),
}

impl Display for PlayerSettingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PlayerIndex(index, player_count) => write!(f, "Only {player_count} player slots exist, can't join as player {}", index + 1),
            Self::Commander(index, commander) => write!(f, "Player {} can't take {commander:?}", index + 1),
            Self::PlayerCount(slots, joined) => write!(f, "{slots} player slots exist, but {joined} players joined"),
        }
    }
}

impl std::error::Error for PlayerSettingError {}

impl GameSettingsInterface for GameConfig {
    fn players(&self) -> Vec<PlayerMeta> {
        self.players.iter().map(|player| PlayerMeta {
            color_id: player.get_owner_id() as u8,
            team: player.get_team(),
        }).collect()
    }

    fn check_player_setting(&self, player_index: usize, bytes: Vec<u8>) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        if player_index >= self.players.len() {
            return Err(Box::new(PlayerSettingError::PlayerIndex(player_index, self.players.len())));
        }
        let options = &self.players[player_index].options;
        let selected = PlayerSelectedOptions::parse(bytes, &self.config)?;
        if let Some(commander) = &selected.commander {
            if !options.commanders.contains(commander) {
                return Err(Box::new(PlayerSettingError::Commander(player_index, *commander)));
            }
        }
        Ok(selected.pack(&self.config))
    }

    fn export(&self) -> Vec<u8> {
        let mut zipper = Zipper::new();
        self.fog_mode.zip(&mut zipper);
        zipper.write_u8((self.players.len() - 1) as u8, bits_needed_for_max_value(self.config.max_player_count() as u32 - 1));
        for p in &self.players {
            p.export(&mut zipper, &self.config);
        }
        zipper.finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerOptions {
    commanders: Vec<CommanderType>,
}

impl SupportedZippable<&Config> for PlayerOptions {
    fn export(&self, zipper: &mut Zipper, config: &Config) {
        for option in config.commander_types() {
            zipper.write_bool(self.commanders.contains(&option));
        }
    }
    fn import(unzipper: &mut Unzipper, config: &Config) -> Result<Self, ZipperError> {
        let mut commanders = Vec::new();
        for option in config.commander_types() {
            if unzipper.read_bool()? {
                commanders.push(option);
            }
        }
        Ok(Self {
            commanders,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Zippable)]
#[zippable(support_ref = Config)]
pub struct PlayerConfig {
    pub options: PlayerOptions,
    funds: Funds,
    income: Income,
    team: Team,
    owner_id: Owner,
}

impl PlayerConfig {
    pub fn new(owner_id: u8, config: &Config) -> Self {
        Self {
            options: PlayerOptions {
                commanders: config.commander_types().to_vec(),
            },
            income: 100.into(),
            funds: 0.into(),
            team: Team(owner_id),
            owner_id: Owner(owner_id as i8),
        }
    }

    pub fn get_commander_options(&self) -> &[CommanderType] {
        &self.options.commanders
    }
    pub fn set_commander_options(&mut self, commanders: Vec<CommanderType>) {
        self.options.commanders = commanders;
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

    pub fn build(&self, player_selection: &PlayerSelectedOptions, random: &RandomFn) -> PlayerSettings {
        let commander = player_selection.commander.unwrap_or_else(|| {
            if self.options.commanders.len() == 0 {
                CommanderType(0)
            } else {
                let index = (self.options.commanders.len() as f32 * random()).floor() as usize;
                self.options.commanders[index]
            }
        });
        PlayerSettings {
            commander,
            funds: self.funds,
            income: self.income,
            team: self.team,
            owner_id: self.owner_id,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Zippable)]
#[zippable(support_ref = Config)]
pub struct PlayerSelectedOptions {
    pub commander: Option<CommanderType>,
}

impl PlayerSelectedOptions {
    pub fn default(_options: &PlayerOptions) -> Self {
        Self {
            commander: None,
        }
    }

    pub fn parse(bytes: Vec<u8>, config: &Config) -> Result<Self, ZipperError> {
        let mut unzipper = Unzipper::new(bytes, Version::parse(VERSION).unwrap());
        Self::import(&mut unzipper, config)
    }

    pub fn pack(&self, config: &Config) -> Vec<u8> {
        let mut zipper = Zipper::new();
        self.export(&mut zipper, config);
        zipper.finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Zippable)]
#[zippable(support_ref = Config)]
pub struct PlayerSettings {
    commander: CommanderType,
    funds: Funds,
    income: Income,
    team: Team,
    owner_id: Owner,
}

impl PlayerSettings {
    pub fn new(owner_id: u8, commander: CommanderType) -> Self {
        Self {
            commander,
            income: 100.into(),
            funds: 0.into(),
            team: Team(owner_id),
            owner_id: Owner(owner_id as i8),
        }
    }

    pub fn get_commander(&self) -> CommanderType {
        self.commander
    }
    pub fn set_commander(&mut self, commander: CommanderType) {
        self.commander = commander
    }

    pub fn get_owner_id(&self) -> i8 {
        self.owner_id.0
    }

    pub fn get_team(&self) -> u8 {
        self.team.0
    }

    pub fn get_income(&self) -> i32 {
        *self.income
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
    use std::sync::Arc;

    use interfaces::GameSettingsInterface;
    use semver::Version;
    use zipper::{SupportedZippable, Unzipper, Zipper};

    use crate::commander::commander_type::CommanderType;
    use crate::config::config::Config;
    use crate::game::fog::{FogMode, FogSetting};
    use crate::VERSION;

    use super::{GameConfig, GameSettings, PlayerOptions, PlayerSettings, PlayerConfig};

    #[test_log::test]
    fn export_commander_options() {
        let config = Config::default();
        let options = PlayerOptions{
            commanders: vec![CommanderType(0)]
        };
        let co = CommanderType(0);
        let mut zipper = Zipper::new();
        options.export(&mut zipper, &config);
        co.export(&mut zipper, &config);
        zipper.write_u8(1, 1);
        let data = zipper.finish();
        tracing::debug!("export_commander_options: {data:?}");
        let mut unzipper = Unzipper::new(data, Version::parse(VERSION).unwrap());
        assert_eq!(Ok(options), PlayerOptions::import(&mut unzipper, &config));
        assert_eq!(Ok(co), CommanderType::import(&mut unzipper, &config));
        assert_eq!(1, unzipper.read_u8(1).unwrap());
    }

    #[test_log::test]
    fn export_game_config() {
        let config = Arc::new(Config::default());
        let setting = GameConfig {
            config: config.clone(),
            fog_mode: FogMode::Constant(FogSetting::Sharp(2)),
            players: vec![
                PlayerConfig::new(0, &config),
                PlayerConfig {
                    options: PlayerOptions {
                        commanders: Vec::new(),
                    },
                    funds: 23.into(),
                    income: (-198).into(),
                    team: 1.into(),
                    owner_id: 3.into(),
                },
            ],
        };
        let bytes = setting.export();
        assert_eq!(Ok(setting), GameConfig::import(config, bytes));
    }

    #[test_log::test]
    fn export_game_settings() {
        let config = Arc::new(Config::default());
        let setting = GameSettings {
            config: config.clone(),
            fog_mode: FogMode::Constant(FogSetting::Sharp(2)),
            players: vec![
                PlayerSettings::new(0, CommanderType::Celerity),
                PlayerSettings::new(3, CommanderType(0)),
            ],
        };
        let mut zipper = Zipper::new();
        setting.export(&mut zipper);
        zipper.write_u8(8, 4);
        let data = zipper.finish();
        let mut unzipper = Unzipper::new(data, Version::parse(VERSION).unwrap());
        assert_eq!(setting, GameSettings::import(&mut unzipper, config.clone()).unwrap());
        assert_eq!(8, unzipper.read_u8(4).unwrap())
    }
}
