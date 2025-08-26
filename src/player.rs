use interfaces::ClientPerspective;
use zipper::*;
use zipper_derive::Zippable;

use crate::commander::Commander;
use crate::config::config::Config;
use crate::config::environment::Environment;
use crate::map::direction::Direction;
use crate::tags::{TagBag, TagValue};
use crate::units::UnitVisibility;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Owner(pub i8);

impl SupportedZippable<&Config> for Owner {
    fn export(&self, zipper: &mut Zipper, support: &Config) {
        zipper.write_u8((self.0 + 1) as u8, bits_needed_for_max_value(support.max_player_count() as u32));
    }
    fn import(unzipper: &mut Unzipper, support: &Config) -> Result<Self, ZipperError> {
        Ok(Self(unzipper.read_u8(bits_needed_for_max_value(support.max_player_count() as u32))? as i8 - 1))
    }
}
impl SupportedZippable<&Environment> for Owner {
    fn export(&self, zipper: &mut Zipper, support: &Environment) {
        self.export(zipper, &*support.config)
    }
    fn import(unzipper: &mut Unzipper, support: &Environment) -> Result<Self, ZipperError> {
        Self::import(unzipper, &*support.config)
    }
}

impl From<i8> for Owner {
    fn from(value: i8) -> Self {
        Self(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Team(pub u8);

impl SupportedZippable<&Config> for Team {
    fn export(&self, zipper: &mut Zipper, support: &Config) {
        zipper.write_u8(self.0, bits_needed_for_max_value(support.max_player_count() as u32));
    }
    fn import(unzipper: &mut Unzipper, support: &Config) -> Result<Self, ZipperError> {
        Ok(Self(unzipper.read_u8(bits_needed_for_max_value(support.max_player_count() as u32))?))
    }
}
impl SupportedZippable<&Environment> for Team {
    fn export(&self, zipper: &mut Zipper, support: &Environment) {
        self.export(zipper, &*support.config)
    }
    fn import(unzipper: &mut Unzipper, support: &Environment) -> Result<Self, ZipperError> {
        Self::import(unzipper, &*support.config)
    }
}

impl From<u8> for Team {
    fn from(value: u8) -> Self {
        Self(value)
    }
}

pub type Perspective = Option<Team>;

pub fn from_client_perspective(value: ClientPerspective) -> Perspective {
    match value {
        ClientPerspective::Neutral => None,
        ClientPerspective::Team(team) => Some(team.into()),
    }
}

pub fn to_client_perspective(value: &Perspective) -> ClientPerspective {
    match value {
        None => ClientPerspective::Neutral,
        Some(team) => ClientPerspective::Team(team.0),
    }
}

#[derive(Debug, Clone, PartialEq, Zippable)]
#[zippable(support_ref = Environment)]
pub struct Player<D: Direction> {
    owner_id: Owner,
    pub dead: bool,
    pub commander: Commander,
    tags: TagBag<D>,
}

impl<D: Direction> Player<D> {
    pub fn new(owner_id: u8, tags: TagBag<D>, commander: Commander) -> Self {
        Self {
            owner_id: Owner(owner_id as i8),
            dead: false,
            commander,
            tags,
        }
    }

    pub fn get_owner_id(&self) -> i8 {
        self.owner_id.0
    }

    pub fn get_team(&self) -> ClientPerspective {
        self.commander.environment().get_team(self.get_owner_id())
    }

    pub fn has_flag(&self, key: usize) -> bool {
        self.tags.has_flag(key)
    }
    pub fn set_flag(&mut self, environment: &Environment, key: usize) {
        self.tags.set_flag(environment, key);
    }
    pub fn remove_flag(&mut self, key: usize) {
        self.tags.remove_flag(key);
    }
    pub fn flip_flag(&mut self, environment: &Environment, key: usize) {
        if self.has_flag(key) {
            self.remove_flag(key);
        } else {
            self.set_flag(environment, key);
        }
    }

    pub fn get_tag(&self, key: usize) -> Option<TagValue<D>> {
        self.tags.get_tag(key)
    }
    pub fn set_tag(&mut self, environment: &Environment, key: usize, value: TagValue<D>) {
        self.tags.set_tag(environment, key, value);
    }
    pub fn remove_tag(&mut self, key: usize) {
        self.tags.remove_tag(key);
    }

    pub fn get_tag_bag(&self) -> &TagBag<D> {
        &self.tags
    }

    pub fn fog_replacement(&self) -> Self {
        Self {
            owner_id: self.owner_id,
            dead: self.dead,
            commander: self.commander.clone(),
            tags: self.tags.fog_replacement(self.commander.environment(), UnitVisibility::Normal),
        }
    }
}
