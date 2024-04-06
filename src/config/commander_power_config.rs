use std::collections::HashMap;

use crate::commander::commander_type::CommanderType;
use crate::config::parse::*;
use crate::script::custom_power::CustomPower;

use super::ConfigParseError;

#[derive(Debug)]
pub struct CommanderPowerConfig {
    pub(super) id: CommanderType,
    pub(crate) name: String, // of the ability
    pub(crate) usable_from_power: Vec<u8>,
    pub(crate) next_power: u8, // at the start of the player's turn, this index is automatically switched to if possible (e.g. player has enough charge)
    pub(crate) required_charge: u32,
    pub(crate) script: CustomPower,
    pub(super) prevents_charging: bool,
}

impl CommanderPowerConfig {
    pub fn parse(data: &HashMap<CommanderPowerConfigHeader, &str>) -> Result<Self, ConfigParseError> {
        use CommanderPowerConfigHeader as H;
        use ConfigParseError as E;
        let get = |key| {
            data.get(&key).ok_or(E::MissingColumn(format!("{key:?}")))
        };
        let result = Self {
            id: parse(data, H::Id)?,
            name: get(H::Name)?.to_string(),
            usable_from_power: parse_vec_def(data, H::UsableFromPowers, Vec::new())?,
            next_power: parse_def(data, H::NextPower, 0)?,
            required_charge: parse_def(data, H::RequiredCharge, 0)?,
            script: parse_def(data, H::Script, CustomPower::None)?,
            prevents_charging: parse_def(data, H::PreventsCharging, false)?,
        };
        result.simple_validation()?;
        Ok(result)
    }

    pub fn simple_validation(&self) -> Result<(), ConfigParseError> {
        /*if self.name.trim().len() == 0 {
            return Err(ConfigParseError::NameTooShort);
        }*/
        Ok(())
    }
}

crate::listable_enum! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum CommanderPowerConfigHeader {
        Id,
        Name,
        UsableFromPowers,
        NextPower,
        RequiredCharge,
        Script,
        PreventsCharging,
    }
}
