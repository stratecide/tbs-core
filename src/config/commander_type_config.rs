use std::error::Error;

use rustc_hash::FxHashMap as HashMap;

use crate::commander::commander_type::CommanderType;
use crate::config::parse::*;

use super::file_loader::{FileLoader, TableLine};
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

impl TableLine for CommanderTypeConfig {
    type Header = CommanderTypeConfigHeader;
    fn parse(data: &HashMap<Self::Header, &str>, loader: &mut FileLoader) -> Result<Self, Box<dyn Error>> {
        use CommanderTypeConfigHeader as H;
        use ConfigParseError as E;
        let get = |key| {
            data.get(&key).ok_or(E::MissingColumn(format!("{key:?}")))
        };
        let result = Self {
            id: parse(data, H::Id, loader)?,
            name: get(H::Name)?.to_string(),
            transport_capacity: parse_def(data, H::TransportCapacity, 0, loader)?,
            max_charge: parse(data, H::Charge, loader)?,
        };
        Ok(result)
    }

    fn simple_validation(&self) -> Result<(), Box<dyn Error>> {
        if self.name.trim().len() == 0 {
            return Err(Box::new(ConfigParseError::NameTooShort));
        }
        if self.max_charge > i32::MAX as u32 {
            return Err(Box::new(ConfigParseError::CommanderMaxChargeExceeded(i32::MAX as u32)));
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
