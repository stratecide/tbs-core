use std::error::Error;

use rustc_hash::FxHashMap as HashMap;

use crate::config::parse::*;
use crate::script::custom_action::CustomAction;
use crate::units::hero::HeroType;

use super::file_loader::{FileLoader, TableLine};
use super::ConfigParseError;

#[derive(Debug)]
pub struct HeroPowerConfig {
    pub(super) hero: HeroType,
    pub(crate) name: String, // of the ability
    pub(crate) usable_from_power: Vec<u8>,
    pub(crate) next_power: u8, // at the start of the player's turn, this index is automatically switched to if possible (e.g. hero has enough charge)
    pub(crate) required_charge: u8,
    pub(super) aura_range: i8,
    pub(super) aura_range_transported: i8,
    pub(crate) script: Option<CustomAction>,
    pub(super) prevents_charging: bool,
}

impl TableLine for HeroPowerConfig {
    type Header = HeroPowerConfigHeader;
    fn parse(data: &HashMap<Self::Header, &str>, loader: &mut FileLoader) -> Result<Self, Box<dyn Error>> {
        use HeroPowerConfigHeader as H;
        use ConfigParseError as E;
        let get = |key| {
            data.get(&key).ok_or(E::MissingColumn(format!("{key:?}")))
        };
        let script = match data.get(&H::Script) {
            Some(s) if s.len() > 0 => {
                let exe = loader.rhai_function(s, 0..=1)?;
                let input = if exe.parameters.len() > 0 {
                    Some(loader.rhai_function(&format!("{s}_input"), 0..=0)?.index)
                } else {
                    None
                };
                Some((input, exe.index))
            }
            _ => None,
        };
        let result = Self {
            hero: parse(data, H::Hero, loader)?,
            name: get(H::Name)?.trim().to_string(),
            usable_from_power: parse_vec_def(data, H::UsableFromPowers, Vec::new(), loader)?,
            next_power: parse_def(data, H::NextPower, 0, loader)?,
            required_charge: parse_def(data, H::RequiredCharge, 0, loader)?,
            aura_range: parse_def(data, H::AuraRange, 0, loader)?,
            aura_range_transported: parse_def(data, H::AuraRange, -9, loader)?,
            script,
            prevents_charging: parse_def(data, H::PreventsCharging, false, loader)?,
        };
        Ok(result)
    }

    fn simple_validation(&self) -> Result<(), Box<dyn Error>> {
        /*if self.name.len() == 0 {
            return Err(ConfigParseError::NameTooShort);
        }*/
        Ok(())
    }
}

impl HeroPowerConfig {
    pub fn get_name(&self) -> &str {
        &self.name
    }

    pub fn get_script(&self) -> Option<CustomAction> {
        self.script
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
        AuraRangeTransported,
        Script,
        PreventsCharging,
    }
}
