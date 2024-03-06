use std::collections::HashMap;

use crate::config::parse::*;
use crate::script::custom_action::CustomAction;
use crate::units::hero::HeroType;

use super::ConfigParseError;

#[derive(Debug)]
pub struct HeroPowerConfig {
    pub(super) hero: HeroType,
    pub(crate) name: String, // of the ability
    pub(crate) usable_from_power: Vec<u8>,
    pub(crate) next_power: u8, // at the start of the player's turn, this index is automatically switched to if possible (e.g. hero has enough charge)
    pub(crate) required_charge: u8,
    pub(super) aura_range: u8,
    pub(crate) script: CustomAction,
}

impl HeroPowerConfig {
    pub fn parse(data: &HashMap<HeroPowerConfigHeader, &str>) -> Result<Self, ConfigParseError> {
        use HeroPowerConfigHeader as H;
        use ConfigParseError as E;
        let get = |key| {
            data.get(&key).ok_or(E::MissingColumn(format!("{key:?}")))
        };
        let result = Self {
            hero: parse(data, H::Hero)?,
            name: get(H::Name)?.trim().to_string(),
            usable_from_power: parse_vec_def(data, H::UsableFromPowers, Vec::new())?,
            next_power: parse_def(data, H::NextPower, 0)?,
            required_charge: parse_def(data, H::RequiredCharge, 0)?,
            aura_range: parse_def(data, H::AuraRange, 0)?,
            script: parse_def(data, H::Script, CustomAction::None)?,
        };
        result.simple_validation()?;
        Ok(result)
    }

    pub fn simple_validation(&self) -> Result<(), ConfigParseError> {
        /*if self.name.len() == 0 {
            return Err(ConfigParseError::NameTooShort);
        }*/
        Ok(())
    }
}

crate::listable_enum! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum HeroPowerConfigHeader {
        Hero,
        Name,
        UsableFromPowers,
        NextPower,
        RequiredCharge,
        AuraRange,
        Script,
    }
}
