use std::collections::HashSet;

use serde::Deserialize;
use num_rational::Rational32;

use crate::game::fog::VisionMode;
use crate::script::unit::UnitScript;
use crate::units::attributes::*;
use crate::units::combat::*;
use crate::units::movement::MovementType;
use crate::units::hero::*;

use super::movement_type_config::MovementPattern;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct HeroTypeConfig {
    pub(super) id: HeroType,
    pub(super) name: String,
    pub(super) price: u16,
    pub(super) relative_price: Rational32,
    pub(super) aura_range: u8,
    pub(super) charge: u8,
    #[serde(default)]
    pub(super) visibility: Option<UnitVisibility>,
    #[serde(default)]
    pub(super) transport_capacity: u8,

    pub(super) movement_points: Rational32,
    pub(super) aura_movement_points: Rational32,
    pub(super) power_movement_points: Rational32,
    pub(super) power_aura_movement_points: Rational32,

    pub(super) attack: Rational32,
    pub(super) aura_attack: Rational32,
    pub(super) power_attack: Rational32,
    pub(super) power_aura_attack: Rational32,

    pub(super) defense: Rational32,
    pub(super) aura_defense: Rational32,
    pub(super) power_defense: Rational32,
    pub(super) power_aura_defense: Rational32,
}
