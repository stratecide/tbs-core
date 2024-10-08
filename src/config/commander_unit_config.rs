use std::error::Error;

use num_rational::Rational32;
use rustc_hash::{FxHashMap as HashMap, FxHashSet as HashSet};

use crate::config::parse::*;
use crate::units::attributes::*;
use crate::units::combat::*;
use crate::units::movement::MovementType;

use super::file_loader::FileLoader;
use super::file_loader::TableLine;
use super::number_modification::NumberMod;
use super::ConfigParseError;
use super::unit_filter::UnitFilter;

#[derive(Debug)]
pub(super) struct CommanderPowerUnitConfig {
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
    pub(super) attack_pattern: Option<AttackType>,
    pub(super) attack_targets: Option<AttackTargeting>,
    pub(super) splash_damage: Vec<Rational32>, // doesn't override if empty. contains factor per additional distance
    pub(super) cost: NumberMod<i32>,
    pub(super) displacement: Option<Displacement>, // implies that attack_pattern is Adjacent or Straight
    pub(super) displacement_distance: NumberMod<i8>, // can only be 0 if Displacement::None
    pub(super) can_be_displaced: Option<bool>,
    pub(super) build_overrides: HashSet<AttributeOverride>,
    pub(super) on_start_turn: Option<usize>,
    pub(super) on_end_turn: Option<usize>,
    pub(super) on_attack: Option<usize>,
    pub(super) on_defend: Option<usize>,
    pub(super) on_kill: Option<usize>,
    pub(super) on_death: Option<usize>,
    pub(super) aura_range: NumberMod<i8>,
    pub(super) aura_range_transported: NumberMod<i8>,
}

impl TableLine for CommanderPowerUnitConfig {
    type Header = CommanderPowerUnitConfigHeader;
    fn parse(data: &HashMap<Self::Header, &str>, loader: &mut FileLoader) -> Result<Self, Box<dyn Error>> {
        use CommanderPowerUnitConfigHeader as H;
        let power = parse_vec_def(data, H::Power, Vec::new(), loader)?;
        let affects = parse_vec_def(data, H::Affects, Vec::new(), loader)?;
        let result = Self {
            affects: power.into_iter().chain(affects.into_iter()).collect(),
            attack: parse_def(data, H::Attack, NumberMod::Keep, loader)?,
            defense: parse_def(data, H::Defense, NumberMod::Keep, loader)?,
            attack_reduced_by_damage: parse_def(data, H::AttackReducedByDamage, NumberMod::Keep, loader)?,
            min_range: parse_def(data, H::MinRange, NumberMod::Keep, loader)?,
            max_range: parse_def(data, H::MaxRange, NumberMod::Keep, loader)?,
            visibility: match data.get(&H::Visibility) {
                Some(s) if s.len() > 0 => Some(UnitVisibility::from_conf(s, loader)?.0),
                _ => None,
            },
            movement_type: match data.get(&H::MovementType) {
                Some(s) if s.len() > 0 => Some(MovementType::from_conf(s, loader)?.0),
                _ => None,
            },
            water_movement_type: match data.get(&H::WaterMovementType) {
                Some(s) if s.len() > 0 => Some(MovementType::from_conf(s, loader)?.0),
                _ => None,
            },
            movement_points: parse_def(data, H::MovementPoints, NumberMod::Keep, loader)?,
            vision: parse_def(data, H::Vision, NumberMod::Keep, loader)?,
            true_vision: parse_def(data, H::TrueVision, NumberMod::Keep, loader)?,
            stealthy: match data.get(&H::Stealthy) {
                Some(s) if s.len() > 0 => Some(s.parse().map_err(|_| ConfigParseError::InvalidBool(s.to_string()))?),
                _ => None,
            },
            attack_pattern: match data.get(&H::AttackPattern) {
                Some(s) if s.len() > 0 => Some(AttackType::from_conf(s, loader)?.0),
                _ => None,
            },
            attack_targets: match data.get(&H::AttackTargets) {
                Some(s) if s.len() > 0 => Some(AttackTargeting::from_conf(s, loader)?.0),
                _ => None,
            },
            splash_damage: parse_vec_def(data, H::SplashDamage, Vec::new(), loader)?,
            cost: parse_def(data, H::Cost, NumberMod::Keep, loader)?,
            displacement: match data.get(&H::Displacement) {
                Some(s) if s.len() > 0 => Some(Displacement::from_conf(s, loader)?.0),
                _ => None,
            },
            displacement_distance: parse_def(data, H::DisplacementDistance, NumberMod::Keep, loader)?,
            can_be_displaced: match data.get(&H::CanBeDisplaced) {
                Some(s) if s.len() > 0 => Some(s.parse().map_err(|_| ConfigParseError::InvalidBool(s.to_string()))?),
                _ => None,
            },
            build_overrides: parse_vec_def(data, H::BuildOverrides, Vec::new(), loader)?.into_iter().collect(),
            on_start_turn: match data.get(&H::OnStartTurn) {
                Some(s) if s.len() > 0 => Some(loader.rhai_function(s, 0..=0)?.index),
                _ => None,
            },
            on_end_turn: match data.get(&H::OnEndTurn) {
                Some(s) if s.len() > 0 => Some(loader.rhai_function(s, 0..=0)?.index),
                _ => None,
            },
            on_attack: match data.get(&H::OnAttack) {
                Some(s) if s.len() > 0 => Some(loader.rhai_function(s, 0..=0)?.index),
                _ => None,
            },
            on_defend: match data.get(&H::OnDefend) {
                Some(s) if s.len() > 0 => Some(loader.rhai_function(s, 0..=0)?.index),
                _ => None,
            },
            on_kill: match data.get(&H::OnKill) {
                Some(s) if s.len() > 0 => Some(loader.rhai_function(s, 0..=0)?.index),
                _ => None,
            },
            on_death: match data.get(&H::OnDeath) {
                Some(s) if s.len() > 0 => Some(loader.rhai_function(s, 0..=0)?.index),
                _ => None,
            },
            aura_range: parse_def(data, H::AuraRange, NumberMod::Keep, loader)?,
            aura_range_transported: parse_def(data, H::AuraRangeTransported, NumberMod::Keep, loader)?,
        };
        Ok(result)
    }

    fn simple_validation(&self) -> Result<(), Box<dyn Error>> {
        let mut overrides = HashSet::default();
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
