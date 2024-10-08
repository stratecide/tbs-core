use std::error::Error;

use rustc_hash::FxHashMap as HashMap;

use crate::config::parse::*;
use crate::units::hero::*;

use super::file_loader::{FileLoader, TableLine};
use super::number_modification::NumberMod;
use super::ConfigParseError;

#[derive(Debug)]
pub struct HeroTypeConfig {
    pub(super) id: HeroType,
    pub(super) name: String,
    pub(super) price: NumberMod<i32>,
    pub(super) charge: u8,
    // gets added to the unit's transport_capacity
    // if reducing the transport_capacity of a unit should be allowed,
    // the unit's current transported.len() has to be below the result in order to pick the hero
    pub(super) transport_capacity: u8,
}

impl TableLine for HeroTypeConfig {
    type Header = HeroTypeConfigHeader;
    fn parse(data: &HashMap<Self::Header, &str>, loader: &mut FileLoader) -> Result<Self, Box<dyn Error>> {
        use HeroTypeConfigHeader as H;
        use ConfigParseError as E;
        let get = |key| {
            data.get(&key).ok_or(E::MissingColumn(format!("{key:?}")))
        };
        let result = Self {
            id: parse(data, H::Id, loader)?,
            name: get(H::Name)?.to_string(),
            price: parse_def(data, H::Price, NumberMod::Keep, loader)?,
            charge: parse_def(data, H::Charge, 0, loader)?,
            transport_capacity: parse_def(data, H::TransportCapacity, 0, loader)?,
        };
        Ok(result)
    }

    fn simple_validation(&self) -> Result<(), Box<dyn Error>> {
        // TODO
        Ok(())
    }
}

crate::listable_enum! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum HeroTypeConfigHeader {
        Id,
        Name,
        Price,
        Charge,
        TransportCapacity,
    }
}
