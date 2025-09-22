use rhai::ImmutableString;
use rustc_hash::FxHashMap as HashMap;
use std::error::Error;
use num_rational::Rational32;

use crate::combat::*;
use crate::config::OwnershipPredicate;
use crate::game::fog::VisionMode;
use crate::units::movement::MovementType;
use crate::units::UnitVisibility;

use super::file_loader::FileLoader;
use super::file_loader::TableLine;
use super::ConfigParseError;
use super::movement_type_config::MovementPattern;
use super::parse::*;

#[derive(Debug)]
pub struct UnitTypeConfig {
    pub(super) name: String,
    pub(super) visibility: UnitVisibility,
    pub(super) movement_pattern: MovementPattern,
    pub(super) movement_type: MovementType,
    pub(super) movement_points: Rational32,
    pub(super) vision_mode: VisionMode,
    pub(super) vision: usize,
    pub(super) true_vision: usize,
    pub(super) owned: OwnershipPredicate,
    pub(super) pass_enemy_units: bool,
    pub(super) can_be_moved_through: bool,
    pub(super) can_be_taken: bool,
    pub(super) can_attack_after_moving: bool,
    pub(super) attack_pattern: AttackPattern,
    pub(super) attack_type: AttackType,
    pub(super) attack_targets: ValidAttackTargets,
    pub(super) attack_direction: AllowedAttackInputDirectionSource,
    pub(super) can_be_displaced: bool,
    pub(super) transport_capacity: usize,
    pub(super) custom_columns: HashMap<ImmutableString, ImmutableString>,
}

impl TableLine for UnitTypeConfig {
    type Header = UnitTypeConfigHeader;
    fn parse(data: &HashMap<Self::Header, &str>, loader: &mut FileLoader) -> Result<Self, Box<dyn Error>> {
        use UnitTypeConfigHeader as H;
        use ConfigParseError as E;
        let get = |key| {
            data.get(&key).ok_or(E::MissingColumn(format!("{key:?}")))
        };
        let mut custom_columns = HashMap::default();
        for (header, s) in data {
            if let H::Custom(name) = header {
                let s = s.trim();
                custom_columns.insert(name.into(), s.into());
            }
        }
        let result = Self {
            name: get(H::Id)?.trim().to_string(),
            visibility: match data.get(&H::Visibility) {
                Some(s) => UnitVisibility::from_conf(s, loader)?.0,
                None => UnitVisibility::Normal,
            },
            movement_pattern: parse_def(data, H::MovementPattern, MovementPattern::Standard, loader)?,
            movement_type: parse(data, H::MovementType, loader)?,
            movement_points: parse_def(data, H::MovementPoints, Rational32::from_integer(0), loader)?,
            vision_mode: parse_def(data, H::VisionMode, VisionMode::Normal, loader)?,
            vision: parse_def(data, H::Vision, 0 as u8, loader)? as usize,
            true_vision: parse_def(data, H::TrueVision, 0 as u8, loader)? as usize,
            owned: parse_def(data, H::Owned, OwnershipPredicate::Either, loader)?,
            pass_enemy_units: parse_def(data, H::PassEnemyUnits, false, loader)?,
            can_be_moved_through: parse_def(data, H::CanBeMovedThrough, false, loader)?,
            can_be_taken: parse_def(data, H::CanBeTaken, false, loader)?,
            can_attack_after_moving: parse_def(data, H::CanAttackAfterMoving, false, loader)?,
            attack_pattern: match data.get(&H::AttackPattern) {
                Some(s) if s.len() > 0 => AttackPattern::from_conf(s, loader)?.0,
                _ => AttackPattern::None,
            },
            attack_direction: parse_def(data, H::AttackDirection, AllowedAttackInputDirectionSource::AllDirections, loader)?,
            attack_targets: parse_def(data, H::AttackTargets, ValidAttackTargets::Enemy, loader)?,
            attack_type: match data.get(&H::AttackType) {
                Some(s) if s.len() > 0 => AttackType::from_conf(s, loader)?.0,
                _ => AttackType(None),
            },
            can_be_displaced: parse_def(data, H::CanBeDisplaced, false, loader)?,
            transport_capacity: parse_def(data, H::TransportCapacity, 0 as u8, loader)? as usize,
            custom_columns,
        };
        Ok(result)
    }

    fn simple_validation(&self) -> Result<(), Box<dyn Error>> {
        if self.name.trim().len() == 0 {
            return Err(Box::new(ConfigParseError::NameTooShort));
        }
        if self.vision < self.true_vision {
            // TODO
        }
        Ok(())
    }
}

crate::enum_with_custom! {
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum UnitTypeConfigHeader {
        Id,
        Visibility,
        MovementPattern,
        MovementType,
        MovementPoints,
        VisionMode,
        Vision,
        TrueVision,
        Owned,
        PassEnemyUnits,
        CanBeMovedThrough,
        CanBeTaken,
        CanAttackAfterMoving,
        AttackPattern,
        AttackDirection,
        AttackTargets,
        AttackType,
        CanBeDisplaced,
        TransportCapacity,
    }
}
