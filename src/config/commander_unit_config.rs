use std::error::Error;

use num_rational::Rational32;
use rustc_hash::FxHashMap as HashMap;

use crate::combat::AllowedAttackInputDirectionSource;
use crate::combat::AttackPattern;
use crate::combat::AttackType;
use crate::combat::ValidAttackTargets;
use crate::config::parse::*;
use crate::units::UnitVisibility;

use super::file_loader::FileLoader;
use super::file_loader::TableLine;
use super::number_modification::NumberMod;
use super::ConfigParseError;
use super::unit_filter::UnitFilter;

#[derive(Debug)]
pub(super) struct CommanderPowerUnitConfig {
    pub(super) affects: Vec<UnitFilter>,
    pub(super) visibility: Option<UnitVisibility>,
    pub(super) movement_points: NumberMod<Rational32>,
    pub(super) vision: NumberMod<u8>,
    pub(super) true_vision: NumberMod<u8>,
    pub(super) pass_enemy_units: Option<bool>,
    pub(super) attack_pattern: Option<AttackPattern>,
    pub(super) attack_type: Option<AttackType>,
    pub(super) attack_targets: Option<ValidAttackTargets>,
    pub(super) attack_direction: Option<AllowedAttackInputDirectionSource>,
    pub(super) can_be_displaced: Option<bool>,
    pub(super) on_death: Option<usize>,
    pub(super) on_normal_action: Option<usize>,
    pub(super) aura_range: NumberMod<i32>,
    custom_columns: HashMap<String, NumberMod<Rational32>>,
}

impl TableLine for CommanderPowerUnitConfig {
    type Header = CommanderPowerUnitConfigHeader;
    fn parse(data: &HashMap<Self::Header, &str>, loader: &mut FileLoader) -> Result<Self, Box<dyn Error>> {
        use CommanderPowerUnitConfigHeader as H;
        let power = parse_vec_def(data, H::Power, Vec::new(), loader)?;
        let affects = parse_vec_def(data, H::Affects, Vec::new(), loader)?;
        let mut custom_columns = HashMap::default();
        for (header, s) in data {
            if let H::Custom(name) = header {
                let s = s.trim();
                if s.len() > 0 {
                    let nm =NumberMod::from_conf(s, loader)?.0;
                    custom_columns.insert(name.clone(), nm);
                }
            }
        }
        Ok(Self {
            affects: power.into_iter().chain(affects.into_iter()).collect(),
            visibility: match data.get(&H::Visibility) {
                Some(s) if s.len() > 0 => Some(UnitVisibility::from_conf(s, loader)?.0),
                _ => None,
            },
            movement_points: parse_def(data, H::MovementPoints, NumberMod::Keep, loader)?,
            vision: parse_def(data, H::Vision, NumberMod::Keep, loader)?,
            true_vision: parse_def(data, H::TrueVision, NumberMod::Keep, loader)?,
            pass_enemy_units: match data.get(&H::PassEnemyUnits) {
                Some(s) if s.len() > 0 => Some(s.parse().map_err(|_| ConfigParseError::InvalidBool(s.to_string()))?),
                _ => None,
            },
            attack_pattern: match data.get(&H::AttackPattern) {
                Some(s) if s.len() > 0 => Some(AttackPattern::from_conf(s, loader)?.0),
                _ => None,
            },
            attack_type: match data.get(&H::AttackType) {
                Some(s) if s.len() > 0 => Some(AttackType::from_conf(s, loader)?.0),
                _ => None,
            },
            attack_targets: match data.get(&H::Targeting) {
                Some(s) if s.len() > 0 => Some(ValidAttackTargets::from_conf(s, loader)?.0),
                _ => None,
            },
            attack_direction: match data.get(&H::AttackDirection) {
                Some(s) if s.len() > 0 => Some(AllowedAttackInputDirectionSource::from_conf(s, loader)?.0),
                _ => None,
            },
            can_be_displaced: match data.get(&H::CanBeDisplaced) {
                Some(s) if s.len() > 0 => Some(s.parse().map_err(|_| ConfigParseError::InvalidBool(s.to_string()))?),
                _ => None,
            },
            on_death: match data.get(&H::OnDeath) {
                Some(s) if s.len() > 0 => Some(loader.rhai_function(s, 0..=0)?.index),
                _ => None,
            },
            on_normal_action: match data.get(&H::OnNormalAction) {
                Some(s) if s.len() > 0 => Some(loader.rhai_function(s, 0..=0)?.index),
                _ => None,
            },
            aura_range: parse_def(data, H::AuraRange, NumberMod::Keep, loader)?,
            custom_columns,
        })
    }

    fn simple_validation(&self) -> Result<(), Box<dyn Error>> {
        Ok(())
    }
}

impl CommanderPowerUnitConfig {
    pub(super) fn get_fraction(&self, column_name: &String) -> NumberMod<Rational32> {
        self.custom_columns.get(column_name)
        .cloned()
        .unwrap_or(NumberMod::Keep)
    }
}

crate::enum_with_custom! {
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum CommanderPowerUnitConfigHeader {
        Power,
        Affects,
        Visibility,
        MovementPoints,
        Vision,
        TrueVision,
        PassEnemyUnits,
        CanBeMovedThrough,
        CanTake,
        CanBeTaken,
        Weapon,
        CanAttackAfterMoving,
        AttackPattern,
        AttackType,
        Targeting,
        AttackDirection,
        CanBuildUnits,
        CanBeDisplaced,
        TransportCapacity,
        OnDeath,
        OnNormalAction,
        AuraRange,
    }
}
