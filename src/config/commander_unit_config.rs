use std::collections::HashMap;
use std::collections::HashSet;
use std::error::Error;
use std::str::FromStr;

use num_rational::Rational32;

use crate::commander::commander_type::CommanderType;
use crate::config::parse::*;
use crate::script::attack::AttackScript;
use crate::script::death::DeathScript;
use crate::script::defend::DefendScript;
use crate::script::kill::KillScript;
use crate::script::unit::UnitScript;
use crate::units::attributes::*;
use crate::units::combat::*;
use crate::units::movement::MovementType;

use super::number_modification::NumberMod;
use super::ConfigParseError;
use super::unit_filter::UnitFilter;

#[derive(Debug)]
pub(super) struct CommanderPowerUnitConfig {
    pub(super) power: PowerRestriction,
    pub(super) affects: Vec<UnitFilter>,
    pub(super) attack: NumberMod<Rational32>,
    pub(super) defense: NumberMod<Rational32>,
    pub(super) attack_reduced_by_damage: NumberMod<Rational32>,
    pub(super) min_range: NumberMod<u8>,
    pub(super) max_range: NumberMod<u8>,
    pub(super) visibility: Option<UnitVisibility>,
    pub(super) movement_type: Option<MovementType>,
    pub(super) water_movement_type: Option<MovementType>,
    pub(super) movement_points: NumberMod<Rational32>,
    pub(super) vision: NumberMod<u8>,
    pub(super) true_vision: NumberMod<u8>,
    pub(super) stealthy: Option<bool>,
    pub(super) attack_targets: Option<AttackTargeting>,
    pub(super) splash_damage: Vec<Rational32>, // doesn't override if empty. contains factor per additional distance
    pub(super) cost: NumberMod<i32>,
    pub(super) displacement: Option<Displacement>, // implies that attack_pattern is Adjacent or Straight
    pub(super) displacement_distance: NumberMod<i8>, // can only be 0 if Displacement::None
    pub(super) can_be_displaced: Option<bool>,
    pub(super) build_overrides: HashSet<AttributeOverride>,
    pub(super) on_start_turn: Vec<UnitScript>,
    pub(super) on_end_turn: Vec<UnitScript>,
    pub(super) on_attack: Vec<AttackScript>,
    pub(super) on_defend: Vec<DefendScript>,
    pub(super) on_kill: Vec<KillScript>,
    pub(super) on_death: Vec<DeathScript>,
    pub(super) aura_range: NumberMod<i8>,
    pub(super) aura_range_transported: NumberMod<i8>,
}

impl CommanderPowerUnitConfig {
    pub fn parse(data: &HashMap<CommanderPowerUnitConfigHeader, &str>, load_config: &Box<dyn Fn(&str) -> Result<String, Box<dyn Error>>>) -> Result<Self, ConfigParseError> {
        use CommanderPowerUnitConfigHeader as H;
        let result = Self {
            power: match data.get(&H::Power) {
                Some(s) if s.len() > 0 => s.parse()?,
                _ => PowerRestriction::None,
            },
            affects: parse_vec_dyn_def(data, H::Affects, Vec::new(), |s| UnitFilter::from_conf(s, load_config))?,
            attack: parse_def(data, H::Attack, NumberMod::Keep)?,
            defense: parse_def(data, H::Defense, NumberMod::Keep)?,
            attack_reduced_by_damage: parse_def(data, H::AttackReducedByDamage, NumberMod::Keep)?,
            min_range: parse_def(data, H::MinRange, NumberMod::Keep)?,
            max_range: parse_def(data, H::MaxRange, NumberMod::Keep)?,
            visibility: match data.get(&H::Visibility) {
                Some(s) if s.len() > 0 => Some(UnitVisibility::from_conf(s)?.0),
                _ => None,
            },
            movement_type: match data.get(&H::MovementType) {
                Some(s) if s.len() > 0 => Some(MovementType::from_conf(s)?.0),
                _ => None,
            },
            water_movement_type: match data.get(&H::WaterMovementType) {
                Some(s) if s.len() > 0 => Some(MovementType::from_conf(s)?.0),
                _ => None,
            },
            movement_points: parse_def(data, H::MovementPoints, NumberMod::Keep)?,
            vision: parse_def(data, H::Vision, NumberMod::Keep)?,
            true_vision: parse_def(data, H::TrueVision, NumberMod::Keep)?,
            stealthy: match data.get(&H::Stealthy) {
                Some(s) if s.len() > 0 => Some(s.parse().map_err(|_| ConfigParseError::InvalidBool(s.to_string()))?),
                _ => None,
            },
            attack_targets: match data.get(&H::AttackTargets) {
                Some(s) if s.len() > 0 => Some(AttackTargeting::from_conf(s)?.0),
                _ => None,
            },
            splash_damage: parse_vec_def(data, H::SplashDamage, Vec::new())?,
            cost: parse_def(data, H::Cost, NumberMod::Keep)?,
            displacement: match data.get(&H::Displacement) {
                Some(s) if s.len() > 0 => Some(Displacement::from_conf(s)?.0),
                _ => None,
            },
            displacement_distance: parse_def(data, H::DisplacementDistance, NumberMod::Keep)?,
            can_be_displaced: match data.get(&H::CanBeDisplaced) {
                Some(s) if s.len() > 0 => Some(s.parse().map_err(|_| ConfigParseError::InvalidBool(s.to_string()))?),
                _ => None,
            },
            build_overrides: parse_vec_def(data, H::BuildOverrides, Vec::new())?.into_iter().collect(),
            on_start_turn: parse_vec_def(data, H::OnStartTurn, Vec::new())?,
            on_end_turn: parse_vec_def(data, H::OnEndTurn, Vec::new())?,
            on_attack: parse_vec_def(data, H::OnAttack, Vec::new())?,
            on_defend: parse_vec_def(data, H::OnDefend, Vec::new())?,
            on_kill: parse_vec_def(data, H::OnKill, Vec::new())?,
            on_death: parse_vec_def(data, H::OnDeath, Vec::new())?,
            aura_range: parse_def(data, H::AuraRange, NumberMod::Keep)?,
            aura_range_transported: parse_def(data, H::AuraRangeTransported, NumberMod::Keep)?,
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
        Defense,
        AttackReducedByDamage,
        MinRange,
        MaxRange,
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
        Cost,
        Displacement,
        DisplacementDistance,
        CanBeDisplaced,
        TransportCapacity,
        BuildOverrides,
        OnStartTurn,
        OnEndTurn,
        OnAttack,
        OnDefend,
        OnKill,
        OnDeath,
        AuraRange,
        AuraRangeTransported,
    }
}

#[derive(Debug)]
pub(super) enum PowerRestriction {
    None,
    Commander(CommanderType, Option<u8>),
    //Hero(HeroType, Option<bool>),
}

impl FromStr for PowerRestriction {
    type Err = ConfigParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (base, s) = string_base(s);
        Ok(match base {
            "None" | "" => Self::None,
            "Commander" | "Co" => {
                if let Ok((commander, power, _)) = parse_tuple2(s) {
                    Self::Commander(commander, Some(power))
                } else {
                    let (commander, _) = parse_tuple1(s)?;
                    Self::Commander(commander, None)
                }
            }
            /*"Hero" | "He" => {
                let commander: HeroType = it.next().ok_or(ConfigParseError::NotEnoughValues(s.to_string()))?.parse()?;
                let power = if let Some(power) = it.next() {
                    Some(power.parse().map_err(|_| ConfigParseError::InvalidBool(s.to_string()))?)
                } else {
                    None
                };
                Self::Hero(commander, power)
            }*/
            invalid => return Err(ConfigParseError::UnknownEnumMember(invalid.to_string())),
        })
    }
}
