use std::collections::HashMap;
use std::collections::HashSet;
use std::str::FromStr;

use serde::Deserialize;
use num_rational::Rational32;

use crate::commander::commander_type::CommanderType;
use crate::config::parse::*;
use crate::script::attack::AttackScript;
use crate::script::kill::KillScript;
use crate::script::unit::UnitScript;
use crate::units::attributes::*;
use crate::units::combat::*;
use crate::units::hero::HeroType;
use crate::units::movement::MovementType;

use super::ConfigParseError;
use super::unit_filter::UnitFilter;

#[derive(Debug)]
pub(super) struct CommanderPowerUnitConfig {
    pub(super) power: PowerRestriction,
    pub(super) affects: Vec<UnitFilter>,
    pub(super) bonus_attack: Rational32,
    pub(super) bonus_defense: Rational32,
    pub(super) bonus_counter_attack: Rational32,
    pub(super) bonus_counter_defense: Rational32,
    pub(super) visibility: Option<UnitVisibility>,
    pub(super) movement_type: Option<MovementType>,
    pub(super) water_movement_type: Option<MovementType>,
    pub(super) bonus_movement_points: Rational32,
    pub(super) bonus_vision: usize,
    pub(super) bonus_true_vision: usize,
    pub(super) stealthy: Option<bool>,
    pub(super) attack_targets: Option<AttackTargeting>,
    pub(super) splash_damage: Vec<Rational32>, // doesn't override if empty. contains factor per additional distance
    pub(super) cost_factor: Option<Rational32>,
    pub(super) extra_cost: Option<i32>,
    pub(super) displacement: Option<Displacement>, // implies that attack_pattern is Adjacent or Straight
    pub(super) displacement_distance: Option<i8>, // can only be 0 if Displacement::None
    pub(super) can_be_displaced: Option<bool>,
    pub(super) build_overrides: HashSet<AttributeOverride>,
    pub(super) on_start_turn: Vec<UnitScript>,
    pub(super) on_end_turn: Vec<UnitScript>,
    pub(super) on_attack: Vec<AttackScript>,
    pub(super) on_kill: Vec<KillScript>,
    pub(super) on_death: Vec<UnitScript>,
}

impl CommanderPowerUnitConfig {
    pub fn parse(data: &HashMap<CommanderPowerUnitConfigHeader, &str>) -> Result<Self, ConfigParseError> {
        use CommanderPowerUnitConfigHeader as H;
        use ConfigParseError as E;
        let get = |key| {
            data.get(&key).ok_or(E::MissingColumn(format!("{key:?}")))
        };
        let result = Self {
            power: match data.get(&H::Power) {
                Some(s) => s.parse()?,
                None => PowerRestriction::None,
            },
            affects: parse_vec_def(data, H::Affects, Vec::new())?,
            bonus_attack: parse_def(data, H::Attack, Rational32::from_integer(0))?,
            bonus_counter_attack: parse_def(data, H::CounterAttack, Rational32::from_integer(0))?,
            bonus_defense: parse_def(data, H::Defense, Rational32::from_integer(0))?,
            bonus_counter_defense: parse_def(data, H::CounterDefense, Rational32::from_integer(0))?,
            visibility: match data.get(&H::Visibility) {
                Some(s) => Some(s.parse()?),
                None => None,
            },
            movement_type: match data.get(&H::MovementType) {
                Some(s) => Some(s.parse()?),
                None => None,
            },
            water_movement_type: match data.get(&H::WaterMovementType) {
                Some(s) => Some(s.parse()?),
                None => None,
            },
            bonus_movement_points: parse_def(data, H::MovementPoints, Rational32::from_integer(0))?,
            bonus_vision: parse_def(data, H::Vision, 0)?,
            bonus_true_vision: parse_def(data, H::TrueVision, 0)?,
            stealthy: match data.get(&H::Stealthy) {
                Some(s) => Some(s.parse().map_err(|_| ConfigParseError::InvalidBool(s.to_string()))?),
                None => None,
            },
            attack_targets: match data.get(&H::AttackTargets) {
                Some(s) => Some(s.parse()?),
                None => None,
            },
            splash_damage: parse_vec_def(data, H::SplashDamage, Vec::new())?,
            cost_factor: match data.get(&H::CostFactor) {
                Some(s) => Some(s.parse().map_err(|_| ConfigParseError::InvalidRatio(s.to_string()))?),
                None => None,
            },
            extra_cost: match data.get(&H::ExtraCost) {
                Some(s) => Some(s.parse().map_err(|_| ConfigParseError::InvalidInteger(s.to_string()))?),
                None => None,
            },
            displacement: match data.get(&H::Displacement) {
                Some(s) => Some(s.parse()?),
                None => None,
            },
            displacement_distance: match data.get(&H::DisplacementDistance) {
                Some(s) => Some(s.parse().map_err(|_| ConfigParseError::InvalidInteger(s.to_string()))?),
                None => None,
            },
            can_be_displaced: match data.get(&H::CanBeDisplaced) {
                Some(s) => Some(s.parse().map_err(|_| ConfigParseError::InvalidBool(s.to_string()))?),
                None => None,
            },
            build_overrides: parse_vec_def(data, H::BuildOverrides, Vec::new())?.into_iter().collect(),
            on_start_turn: parse_vec_def(data, H::OnStartTurn, Vec::new())?,
            on_end_turn: parse_vec_def(data, H::OnEndTurn, Vec::new())?,
            on_kill: parse_vec_def(data, H::OnEndTurn, Vec::new())?,
            on_attack: parse_vec_def(data, H::OnDeath, Vec::new())?,
            on_death: parse_vec_def(data, H::OnStartTurn, Vec::new())?,
        };
        result.simple_validation()?;
        Ok(result)
    }

    pub fn simple_validation(&self) -> Result<(), ConfigParseError> {
        let mut overrides = HashSet::new();
        for key in self.build_overrides.iter().map(AttributeOverride::key) {
            if !overrides.insert(key) {
                // TODO: return error
            }
        }
        Ok(())
    }
}

crate::listable_enum! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum CommanderPowerUnitConfigHeader {
        Power,
        Affects,
        Attack,
        CounterAttack,
        Defense,
        CounterDefense,
        Visibility,
        MovementType,
        WaterMovementType,
        MovementPoints,
        Vision,
        TrueVision,
        Stealthy,
        CanBeMovedThrough,
        CanTake,
        CanBeTaken,
        Weapon,
        CanAttackAfterMoving,
        AttackPattern,
        AttackTargets,
        SplashDamage,
        CanBuildUnits,
        CostFactor,
        ExtraCost,
        Displacement,
        DisplacementDistance,
        CanBeDisplaced,
        TransportCapacity,
        BuildOverrides,
        OnStartTurn,
        OnEndTurn,
        OnDeath,
    }
}

#[derive(Debug)]
pub(super) enum PowerRestriction {
    None,
    Commander(CommanderType, Option<u8>),
    Hero(HeroType, Option<bool>),
}

impl FromStr for PowerRestriction {
    type Err = ConfigParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut it = s.split(&['(', ' ', '-', ')'])
        .map(str::trim);
        Ok(match it.next().unwrap() {
            "None" => Self::None,
            "Commander" | "Co" => {
                let commander: CommanderType = it.next().ok_or(ConfigParseError::NotEnoughValues(s.to_string()))?.parse()?;
                let power = if let Some(power) = it.next() {
                    Some(power.parse().map_err(|_| ConfigParseError::InvalidInteger(s.to_string()))?)
                } else {
                    None
                };
                Self::Commander(commander, power)
            }
            "Hero" | "He" => {
                let commander: HeroType = it.next().ok_or(ConfigParseError::NotEnoughValues(s.to_string()))?.parse()?;
                let power = if let Some(power) = it.next() {
                    Some(power.parse().map_err(|_| ConfigParseError::InvalidBool(s.to_string()))?)
                } else {
                    None
                };
                Self::Hero(commander, power)
            }
            invalid => return Err(ConfigParseError::UnknownEnumMember(invalid.to_string())),
        })
    }
}
