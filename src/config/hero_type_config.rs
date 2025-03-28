use std::error::Error;

use rustc_hash::FxHashMap as HashMap;

use crate::config::parse::*;

use super::file_loader::{FileLoader, TableLine};
use super::hero_power_config::HeroPowerConfig;
use super::number_modification::NumberMod;
use super::ConfigParseError;

#[derive(Debug)]
pub struct HeroTypeConfig {
    pub(super) name: String,
    pub(super) price: NumberMod<i32>,
    pub(super) max_charge: u32,
    // gets added to the unit's transport_capacity
    // if reducing the transport_capacity of a unit should be allowed,
    // the unit's current transported.len() has to be below the result in order to pick the hero
    pub(super) transport_capacity: u8,
    pub(super) powers: Vec<HeroPowerConfig>,
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
            name: get(H::Id)?.to_string(),
            price: parse_def(data, H::Price, NumberMod::Keep, loader)?,
            max_charge: parse_def(data, H::Charge, 0, loader)?,
            transport_capacity: parse_def(data, H::TransportCapacity, 0, loader)?,
            powers: Vec::new(),
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
        Price,
        Charge,
        TransportCapacity,
    }
}
