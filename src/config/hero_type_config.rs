use std::collections::HashMap;

use num_rational::Rational32;

use crate::config::parse::*;
use crate::units::attributes::*;
use crate::units::hero::*;

use super::ConfigParseError;

#[derive(Debug)]
pub struct HeroTypeConfig {
    pub(super) id: HeroType,
    pub(super) name: String,
    pub(super) price: u16,
    pub(super) relative_price: Rational32,
    pub(super) aura_range: u8,
    pub(super) charge: u8,
    pub(super) visibility: Option<UnitVisibility>,
    pub(super) transport_capacity: u8,

    pub(super) movement_points: Rational32,
    pub(super) power_movement_points: Rational32,

    pub(super) attack: Rational32,
    pub(super) power_attack: Rational32,

    pub(super) defense: Rational32,
    pub(super) power_defense: Rational32,
}

impl HeroTypeConfig {
    pub fn parse(data: &HashMap<HeroTypeConfigHeader, &str>) -> Result<Self, ConfigParseError> {
        use HeroTypeConfigHeader as H;
        use ConfigParseError as E;
        let get = |key| {
            data.get(&key).ok_or(E::MissingColumn(format!("{key:?}")))
        };
        Ok(Self {
            id: get(H::Id)?.parse()?,
            name: get(H::Name)?.to_string(),
            price: parse_def(data, H::Price, 0)?,
            relative_price: parse_def(data, H::RelativePrice, Rational32::from_integer(0))?,
            aura_range: parse_def(data, H::AuraRange, 0)?,
            charge: parse_def(data, H::Charge, 0)?,
            visibility: match data.get(&H::Visibility) {
                Some(s) if s.len() > 0 => Some(s.parse()?),
                _ => None,
            },
            transport_capacity: parse_def(data, H::TransportCapacity, 0)?,
            movement_points: parse_def(data, H::MovementPoints, Rational32::from_integer(0))?,
            power_movement_points: parse_def(data, H::PowerMovementPoints, Rational32::from_integer(0))?,
            attack: parse_def(data, H::Attack, Rational32::from_integer(0))?,
            power_attack: parse_def(data, H::PowerAttack, Rational32::from_integer(0))?,
            defense: parse_def(data, H::Defense, Rational32::from_integer(0))?,
            power_defense: parse_def(data, H::PowerDefense, Rational32::from_integer(0))?,
        })
    }
}

crate::listable_enum! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum HeroTypeConfigHeader {
        Id,
        Name,
        Price,
        RelativePrice,
        AuraRange,
        Charge,
        Visibility,
        TransportCapacity,
        MovementPoints,
        PowerMovementPoints,
        Attack,
        PowerAttack,
        Defense,
        PowerDefense,
    }
}
