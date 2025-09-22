use std::fmt::{Debug, Display};

use crate::commander::Commander;
use crate::commander::commander_type::CommanderType;
use crate::config::editor_tag_config::TagEditorVisibility;
use crate::config::environment::Environment;
use crate::config::config::Config;
use crate::map::board::BoardView;
use crate::map::direction::Direction;
use crate::map::map::Map;
use crate::tags::{TagBag, TagValue};
use crate::units::hero::HeroType;
use crate::{player::*, VERSION};

use super::fog::FogMode;
use interfaces::map_interface::GameSettingsInterface;
use interfaces::{PlayerMeta, RandomFn};
use semver::Version;
use uniform_smart_pointer::Urc;
use zipper::*;
use zipper_derive::Zippable;


#[derive(Debug, Clone)]
pub struct GameConfig<D: Direction> {
    pub fog_mode: FogMode,
    pub tags: TagBag<D>,
    pub players: Vec<PlayerConfig<D>>,
}

impl<D: Direction> GameConfig<D> {
    pub fn build(&self, player_selections: &[PlayerSelectedOptions], random: &RandomFn) -> GameSettings {
        GameSettings {
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
            hero: None,
        }).collect();
        let random: RandomFn = Urc::new(|| 0.);
        Self::build(&self, &player_selections, &random)
    }

    pub(crate) fn check_player_setting(&self, config: &Config, player_index: usize, bytes: Vec<u8>) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        if player_index >= self.players.len() {
            return Err(Box::new(PlayerSettingError::PlayerIndex(player_index, self.players.len())));
        }
        let options = &self.players[player_index].options;
        let selected = PlayerSelectedOptions::parse(bytes, config)?;
        if let Some(commander) = &selected.commander {
            if !options.commanders.contains(commander) {
                return Err(Box::new(PlayerSettingError::Commander(player_index, *commander)));
            }
        }
        Ok(selected.pack(config))
    }

    pub fn import(map: &Map<D>, bytes: Vec<u8>) -> Result<Self, ZipperError> {
        let mut unzipper = Unzipper::new(bytes, Version::parse(VERSION).unwrap());
        let fog_mode = FogMode::unzip(&mut unzipper)?;
        let tags = TagBag::import(&mut unzipper, map.environment())?;
        let mut players = Vec::new();
        for _ in 0..unzipper.read_u8(bits_needed_for_max_value(map.environment().config.max_player_count() as u32 - 1))? + 1 {
            players.push(PlayerConfig::import(&mut unzipper, map.environment())?);
        }
        unzipper.finish()?;
        Ok(Self {
            fog_mode,
            tags,
            players,
        })
    }

    pub fn export(&self, map: &Map<D>) -> Vec<u8> {
        let mut zipper = Zipper::new();
        self.fog_mode.zip(&mut zipper);
        self.tags.export(&mut zipper, map.environment());
        zipper.write_u8((self.players.len() - 1) as u8, bits_needed_for_max_value(map.environment().config.max_player_count() as u32 - 1));
        for p in &self.players {
            p.export(&mut zipper, map.environment());
        }
        zipper.finish()
    }
}

impl<D: Direction> PartialEq for GameConfig<D> {
    fn eq(&self, other: &Self) -> bool {
        self.fog_mode == other.fog_mode &&
        self.tags == other.tags &&
        self.players == other.players
    }
}

#[derive(Debug, Clone)]
pub struct GameSettings {
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
    pub fn import(unzipper: &mut Unzipper, config: Urc<Config>) -> Result<Self, ZipperError> {
        let fog_mode = FogMode::unzip(unzipper)?;
        let mut players = Vec::new();
        for _ in 0..unzipper.read_u8(bits_needed_for_max_value(config.max_player_count() as u32 - 1))? + 1 {
            players.push(PlayerSettings::import(unzipper, &config)?);
        }
        Ok(Self {
            fog_mode,
            players,
        })
    }

    pub fn export(&self, zipper: &mut Zipper, config: &Config) {
        self.fog_mode.zip(zipper);
        zipper.write_u8((self.players.len() - 1) as u8, bits_needed_for_max_value(config.max_player_count() as u32 - 1));
        for p in &self.players {
            p.export(zipper, config);
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

impl<D: Direction> GameSettingsInterface for GameConfig<D> {
    fn players(&self) -> Vec<PlayerMeta> {
        self.players.iter().map(|player| PlayerMeta {
            color_id: player.get_owner_id() as u8,
            team: player.get_team(),
        }).collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerOptions {
    commanders: Vec<CommanderType>,
    heroes: Vec<HeroType>,
}

impl SupportedZippable<&Environment> for PlayerOptions {
    fn export(&self, zipper: &mut Zipper, environment: &Environment) {
        for option in environment.config.commander_types() {
            zipper.write_bool(self.commanders.contains(&option));
        }
        for option in environment.config.hero_types() {
            zipper.write_bool(self.heroes.contains(&option));
        }
    }
    fn import(unzipper: &mut Unzipper, environment: &Environment) -> Result<Self, ZipperError> {
        let mut commanders = Vec::new();
        for option in environment.config.commander_types() {
            if unzipper.read_bool()? {
                commanders.push(option);
            }
        }
        let mut heroes = Vec::new();
        for option in environment.config.hero_types() {
            if unzipper.read_bool()? {
                heroes.push(option);
            }
        }
        Ok(Self {
            commanders,
            heroes,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Zippable)]
#[zippable(support_ref = Environment)]
pub struct PlayerConfig<D: Direction> {
    pub options: PlayerOptions,
    tags: TagBag<D>,
    team: Team,
    owner_id: Owner,
}

impl<D: Direction> PlayerConfig<D> {
    pub fn new(owner_id: u8, map: &Map<D>, random: &RandomFn) -> Self {
        let mut tags = TagBag::new();
        for tag in 0..map.environment().config.tag_count() {
            if map.environment().config.tag_config(tag).player >= TagEditorVisibility::Normal {
                tags.set_tag(map.environment(), tag, TagValue::default_value(map, tag, random()));
            }
        }
        Self {
            options: PlayerOptions {
                commanders: map.environment().config.commander_types().to_vec(),
                heroes: map.environment().config.hero_types().to_vec(),
            },
            tags,
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

    pub fn get_hero_options(&self) -> &[HeroType] {
        &self.options.heroes
    }
    pub fn set_hero_options(&mut self, heroes: Vec<HeroType>) {
        self.options.heroes = heroes;
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

    pub fn get_tag_bag(&self) -> &TagBag<D> {
        &self.tags
    }
    pub fn get_tag_bag_mut(&mut self) -> &mut TagBag<D> {
        &mut self.tags
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
        let hero = player_selection.hero.unwrap_or_else(|| {
            if self.options.heroes.len() == 0 {
                HeroType(0)
            } else {
                let index = (self.options.heroes.len() as f32 * random()).floor() as usize;
                self.options.heroes[index]
            }
        });
        PlayerSettings {
            commander,
            hero,
            team: self.team,
            owner_id: self.owner_id,
        }
    }

    pub fn build_player(&self, environment: &Environment, settings: &PlayerSettings) -> Player<D> {
        Player::new(self.owner_id.0 as u8, self.tags.clone(), Commander::new(environment, settings.commander))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Zippable)]
#[zippable(support_ref = Config)]
pub struct PlayerSelectedOptions {
    pub commander: Option<CommanderType>,
    pub hero: Option<HeroType>,
}

impl PlayerSelectedOptions {
    pub fn default(_options: &PlayerOptions) -> Self {
        Self {
            commander: None,
            hero: None,
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

/// Information about Players that don't change during a game.
/// Makes it easier to access some information without needing access to the actual Players.
#[derive(Debug, Clone, PartialEq, Eq, Zippable)]
#[zippable(support_ref = Config)]
pub struct PlayerSettings {
    commander: CommanderType,
    hero: HeroType,
    team: Team,
    owner_id: Owner,
}

impl PlayerSettings {
    pub fn new(owner_id: u8, commander: CommanderType, hero: HeroType) -> Self {
        Self {
            commander,
            hero,
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

    pub fn get_hero(&self) -> HeroType {
        self.hero
    }
    pub fn set_hero(&mut self, hero: HeroType) {
        self.hero = hero
    }

    pub fn get_owner_id(&self) -> i8 {
        self.owner_id.0
    }

    pub fn get_team(&self) -> u8 {
        self.team.0
    }
}

#[cfg(test)]
mod tests {
    use interfaces::RandomFn;
    use semver::Version;
    use uniform_smart_pointer::Urc;
    use zipper::{SupportedZippable, Unzipper, Zipper};

    use crate::commander::commander_type::CommanderType;
    use crate::config::config::Config;
    use crate::config::environment::Environment;
    use crate::game::fog::{FogMode, FogSetting};
    use crate::map::board::BoardView;
    use crate::map::direction::Direction4;
    use crate::map::map::Map;
    use crate::map::point_map::{MapSize, PointMap};
    use crate::map::wrapping_map::WMBuilder;
    use crate::tags::{TagBag, TagValue};
    use crate::units::hero::HeroType;
    use crate::VERSION;

    use super::{GameConfig, GameSettings, PlayerOptions, PlayerSettings, PlayerConfig};

    #[test]
    fn export_commander_options() {
        let config = Urc::new(Config::default());
        let environment = Environment::new_map(config.clone(), MapSize::new(4, 5));
        let options = PlayerOptions{
            commanders: vec![CommanderType(0)],
            heroes: vec![HeroType(0)],
        };
        let co = CommanderType(0);
        let mut zipper = Zipper::new();
        options.export(&mut zipper, &environment);
        co.export(&mut zipper, &config);
        zipper.write_u8(1, 1);
        let data = zipper.finish();
        crate::debug!("export_commander_options: {data:?}");
        let mut unzipper = Unzipper::new(data, Version::parse(VERSION).unwrap());
        assert_eq!(Ok(options), PlayerOptions::import(&mut unzipper, &environment));
        assert_eq!(Ok(co), CommanderType::import(&mut unzipper, &config));
        assert_eq!(1, unzipper.read_u8(1).unwrap());
    }

    #[test]
    fn export_game_config() {
        let config = Urc::new(Config::default());
        let map = PointMap::new(8, 8, false);
        let map = WMBuilder::<Direction4>::new(map);
        let map = Map::new(map.build(), &config);
        let environment = map.environment().clone();
        let mut tags = TagBag::new();
        tags.set_tag(&environment, 12, TagValue::Direction(Direction4::D90));
        let random: RandomFn = Urc::new(|| 0.5);
        let setting = GameConfig {
            fog_mode: FogMode::Constant(FogSetting::Sharp(2)),
            tags: TagBag::new(),
            players: vec![
                PlayerConfig::new(0, &map, &random),
                PlayerConfig {
                    options: PlayerOptions {
                        commanders: Vec::new(),
                        heroes: Vec::new(),
                    },
                    team: 1.into(),
                    owner_id: 3.into(),
                    tags,
                },
            ],
        };
        let bytes = setting.export(&map);
        assert_eq!(Ok(setting), GameConfig::import(&map, bytes));
    }

    #[test]
    fn export_game_settings() {
        let config = Urc::new(Config::default());
        let setting = GameSettings {
            fog_mode: FogMode::Constant(FogSetting::Sharp(2)),
            players: vec![
                PlayerSettings::new(0, CommanderType::Celerity, HeroType::CRYSTAL),
                PlayerSettings::new(3, CommanderType(0), HeroType(0)),
            ],
        };
        let mut zipper = Zipper::new();
        setting.export(&mut zipper, &config);
        zipper.write_u8(8, 4);
        let data = zipper.finish();
        let mut unzipper = Unzipper::new(data, Version::parse(VERSION).unwrap());
        assert_eq!(setting, GameSettings::import(&mut unzipper, config.clone()).unwrap());
        assert_eq!(8, unzipper.read_u8(4).unwrap())
    }
}
