use std::collections::HashSet;

use serde::Deserialize;
use num_rational::Rational32;

use crate::commander::commander_type::CommanderType;
use crate::script::attack::AttackScript;
use crate::script::kill::KillScript;
use crate::script::unit::UnitScript;
use crate::units::attributes::*;
use crate::units::combat::*;
use crate::units::movement::MovementType;

use super::unit_filter::UnitFilter;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub(super) struct CommanderPowerUnitConfig {
    pub(super) commander: CommanderType,
    #[serde(default)]
    pub(super) commander_power_id: Option<u8>,
    #[serde(default)]
    pub(super) affects: Vec<UnitFilter>,
    #[serde(default)]
    pub(super) visibility: Option<UnitVisibility>,
    #[serde(default)]
    pub(super) movement_type: Option<MovementType>,
    #[serde(default)]
    pub(super) water_movement_type: Option<MovementType>,
    pub(super) bonus_movement_points: Rational32,
    #[serde(default)]
    pub(super) bonus_vision: usize,
    #[serde(default)]
    pub(super) bonus_attack: Rational32,
    #[serde(default)]
    pub(super) bonus_defense: Rational32,
    #[serde(default)]
    pub(super) bonus_counter_attack: Rational32,
    #[serde(default)]
    pub(super) bonus_counter_defense: Rational32,
    #[serde(default)]
    pub(super) bonus_true_vision: usize,
    #[serde(default)]
    pub(super) stealthy: Option<bool>,
    #[serde(default)]
    pub(super) attack_targets: Option<AttackTargeting>,
    #[serde(default)]
    pub(super) splash_damage: Vec<Rational32>, // doesn't override if empty. contains factor per additional distance
    #[serde(default)]
    pub(super) cost_factor: Option<Rational32>,
    #[serde(default)]
    pub(super) extra_cost: Option<i32>,
    #[serde(default)]
    pub(super) displacement: Option<Displacement>, // implies that attack_pattern is Adjacent or Straight
    #[serde(default)]
    pub(super) displacement_distance: Option<i8>, // can only be 0 if Displacement::None
    #[serde(default)]
    pub(super) can_be_displaced: Option<bool>,
    #[serde(default)]
    pub(super) heal_transported: Option<i8>,
    #[serde(default)]
    pub(super) build_overrides: HashSet<AttributeOverride>,
    #[serde(default)]
    pub(super) on_death: Vec<UnitScript>,
    #[serde(default)]
    pub(super) on_attack: Vec<AttackScript>,
    #[serde(default)]
    pub(super) on_kill: Vec<KillScript>,
}
