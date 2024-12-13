pub mod commands;
pub mod movement;
pub mod rhai_movement;
pub mod combat;
pub mod rhai_combat;
pub mod hero;
pub mod unit_types;
pub mod unit;
pub mod rhai_unit;
#[cfg(test)]
pub(crate) mod test;

use zipper::*;
use zipper_derive::Zippable;

use crate::{config::parse::FromConfig, game::fog::FogIntensity};

pub type Hp = U<100>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Zippable)]
#[zippable(bits = 2)]
pub enum UnitVisibility {
    Stealth,
    Normal,
    AlwaysVisible,
}

impl UnitVisibility {
    pub fn visible_in_fog(&self, fog_intensity: FogIntensity) -> bool {
        match self {
            Self::Stealth => fog_intensity == FogIntensity::TrueSight,
            Self::Normal => fog_intensity < FogIntensity::Dark,
            Self::AlwaysVisible => true,
        }
    }
}

impl FromConfig for UnitVisibility {
    fn from_conf<'a>(s: &'a str, _: &mut crate::config::file_loader::FileLoader) -> Result<(Self, &'a str), crate::config::ConfigParseError> {
        let (base, s) = crate::config::parse::string_base(s);
        match base {
            "Stealth" => Ok((Self::Stealth, s)),
            "Normal" => Ok((Self::Normal, s)),
            "Always" | "AlwaysVisible" => Ok((Self::AlwaysVisible, s)),
            _ => Err(crate::config::ConfigParseError::UnknownEnumMember(format!("Visibility::{base} - {s}")))
        }
    }
}

impl ToString for UnitVisibility {
    fn to_string(&self) -> String {
        match self {
            Self::Stealth => "Stealth".to_string(),
            Self::Normal => "Normal".to_string(),
            Self::AlwaysVisible => "AlwaysVisible".to_string(),
        }
    }
}
