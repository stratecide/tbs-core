use std::collections::HashMap;

use crate::commander::commander_type::CommanderType;
use crate::config::parse::*;

use super::ConfigParseError;

/**
 * contains data that shouldn't change when using a different power
 */
#[derive(Debug)]
pub struct CommanderTypeConfig {
    pub(super) id: CommanderType,
    pub(super) name: String,
    pub(super) transport_capacity: u8,
    pub(super) max_charge: u32,
}

impl CommanderTypeConfig {
    pub fn parse(data: &HashMap<CommanderTypeConfigHeader, &str>) -> Result<Self, ConfigParseError> {
        use CommanderTypeConfigHeader as H;
        use ConfigParseError as E;
        let get = |key| {
            data.get(&key).ok_or(E::MissingColumn(format!("{key:?}")))
        };
        let result = Self {
            id: parse(data, H::Id)?,
            name: get(H::Name)?.to_string(),
            transport_capacity: parse_def(data, H::TransportCapacity, 0)?,
            max_charge: parse(data, H::Charge)?,
        };
        result.simple_validation()?;
        Ok(result)
    }

    pub fn simple_validation(&self) -> Result<(), ConfigParseError> {
        if self.name.trim().len() == 0 {
            return Err(ConfigParseError::NameTooShort);
        }
        if self.max_charge > i32::MAX as u32 {
            return Err(ConfigParseError::CommanderMaxChargeExceeded(i32::MAX as u32));
        }
        Ok(())
    }
}

crate::listable_enum! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum CommanderTypeConfigHeader {
        Id,
        Name,
        TransportCapacity,
        Charge,
    }
}
