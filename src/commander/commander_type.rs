use serde::Deserialize;
use zipper::*;
use std::str::FromStr;

use crate::config::config::Config;
use crate::config::environment::Environment;
use crate::config::ConfigParseError;

crate::listable_enum! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize)]
    pub enum CommanderType {
        None,
        Vampire,
        Zombie,
    }
}

impl SupportedZippable<&Config> for CommanderType {
    fn export(&self, zipper: &mut Zipper, support: &Config) {
        let index = support.commander_types().iter().position(|t| t == self).unwrap();
        let bits = bits_needed_for_max_value(support.commander_count() as u32 - 1);
        zipper.write_u32(index as u32, bits);
    }
    fn import(unzipper: &mut Unzipper, support: &Config) -> Result<Self, ZipperError> {
        let bits = bits_needed_for_max_value(support.commander_count() as u32 - 1);
        let index = unzipper.read_u32(bits)? as usize;
        if index < support.commander_count() {
            Ok(support.commander_types()[index])
        } else {
            Err(ZipperError::EnumOutOfBounds(format!("CommanderType index {}", index)))
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CommanderChargeChange(pub i32);

impl SupportedZippable<&Environment> for CommanderChargeChange {
    fn export(&self, zipper: &mut Zipper, support: &Environment) {
        let max = support.config.max_commander_charge() as i32;
        zipper.write_u32((self.0 + max) as u32, bits_needed_for_max_value(max as u32 * 2));
    }
    fn import(unzipper: &mut Unzipper, support: &Environment) -> Result<Self, ZipperError> {
        let max = support.config.max_commander_charge() as i32;
        Ok(Self(unzipper.read_u32(bits_needed_for_max_value(max as u32 * 2))? as i32 - max))
    }
}

impl From<i32> for CommanderChargeChange {
    fn from(value: i32) -> Self {
        Self(value)
    }
}
