use rustc_hash::FxHashMap as HashMap;
use std::error::Error;
use num_rational::Rational32;

use crate::game::fog::VisionMode;
use crate::units::attributes::*;
use crate::units::combat::*;
use crate::units::movement::MovementType;
use crate::units::unit_types::UnitType;

use super::file_loader::FileLoader;
use super::file_loader::TableLine;
use super::ConfigParseError;
use super::movement_type_config::MovementPattern;
use super::parse::*;

#[derive(Debug)]
pub struct UnitTypeConfig {
    pub(super) id: UnitType,
    pub(super) name: String,
    //pub(super) attribute_keys: Vec<AttributeKey>,
    //pub(super) attribute_keys_hidden_by_fog: Vec<AttributeKey>,
    //pub(super) valid_action_status: Vec<ActionStatus>,
    pub(super) visibility: UnitVisibility,
    pub(super) movement_pattern: MovementPattern,
    pub(super) movement_type: MovementType,
    pub(super) water_movement_type: Option<MovementType>,
    pub(super) movement_points: Rational32,
    pub(super) vision_mode: VisionMode,
    pub(super) vision: usize,
    pub(super) true_vision: usize,
    pub(super) needs_owner: bool,
    pub(super) stealthy: bool,
    pub(super) can_be_moved_through: bool,
    pub(super) can_be_taken: bool,
    pub(super) weapon: WeaponType,
    pub(super) can_attack_after_moving: bool,
    pub(super) attack_pattern: AttackType,
    pub(super) attack_targets: AttackTargeting,
    //#[serde(default)]
    //pub(super) splash_range: usize,
    //pub(super) splash_factor: Rational32,
    pub(super) splash_damage: Vec<Rational32>, // empty if no splash damage. contains factor per additional distance
    pub(super) cost: i32,
    pub(super) displacement: Displacement, // implies that attack_pattern is Adjacent or Straight
    pub(super) displacement_distance: i8, // can only be 0 if Displacement::None
    pub(super) can_be_displaced: bool,
    pub(super) transport_capacity: usize,
}

impl TableLine for UnitTypeConfig {
    type Header = UnitTypeConfigHeader;
    fn parse(data: &HashMap<Self::Header, &str>, loader: &mut FileLoader) -> Result<Self, Box<dyn Error>> {
        use UnitTypeConfigHeader as H;
        use ConfigParseError as E;
        let get = |key| {
            data.get(&key).ok_or(E::MissingColumn(format!("{key:?}")))
        };
        let result = Self {
            id: parse(data, H::Id, loader)?,
            name: get(H::Name)?.to_string(),
            visibility: match data.get(&H::Visibility) {
                Some(s) => UnitVisibility::from_conf(s, loader)?.0,
                None => UnitVisibility::Normal,
            },
            movement_pattern: parse_def(data, H::MovementPattern, MovementPattern::Standard, loader)?,
            movement_type: parse(data, H::MovementType, loader)?,
            water_movement_type: match data.get(&H::WaterMovementType) {
                Some(s) if s.len() > 0 => Some(MovementType::from_conf(s, loader)?.0),
                _ => None,
            },
            movement_points: parse_def(data, H::MovementPoints, Rational32::from_integer(0), loader)?,
            vision_mode: parse_def(data, H::VisionMode, VisionMode::Normal, loader)?,
            vision: parse_def(data, H::Vision, 0 as u8, loader)? as usize,
            true_vision: parse_def(data, H::TrueVision, 0 as u8, loader)? as usize,
            needs_owner: parse_def(data, H::NeedsOwner, false, loader)?,
            stealthy: parse_def(data, H::Stealthy, false, loader)?,
            can_be_moved_through: parse_def(data, H::CanBeMovedThrough, false, loader)?,
            can_be_taken: parse_def(data, H::CanBeTaken, false, loader)?,
            weapon: parse_def(data, H::Weapon, WeaponType::MachineGun, loader)?,
            can_attack_after_moving: parse_def(data, H::CanAttackAfterMoving, false, loader)?,
            attack_pattern: parse_def(data, H::AttackPattern, AttackType::None, loader)?,
            attack_targets: parse_def(data, H::AttackTargets, AttackTargeting::Enemy, loader)?,
            splash_damage: parse_vec_def(data, H::SplashDamage, vec![Rational32::from_integer(1)], loader)?,
            cost: parse(data, H::Cost, loader)?,
            displacement: parse_def(data, H::Displacement, Displacement::None, loader)?,
            displacement_distance: parse_def(data, H::DisplacementDistance, 0, loader)?,
            can_be_displaced: parse_def(data, H::CanBeDisplaced, false, loader)?,
            transport_capacity: parse_def(data, H::TransportCapacity, 0 as u8, loader)? as usize,
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

crate::listable_enum! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum UnitTypeConfigHeader {
        Id,
        Name,
        Visibility,
        MovementPattern,
        MovementType,
        WaterMovementType,
        MovementPoints,
        VisionMode,
        Vision,
        TrueVision,
        NeedsOwner,
        Stealthy,
        CanBeMovedThrough,
        CanBeTaken,
        Weapon,
        CanAttackAfterMoving,
        AttackPattern,
        AttackTargets,
        SplashDamage,
        Cost,
        Displacement,
        DisplacementDistance,
        CanBeDisplaced,
        TransportCapacity,
    }
}

/*pub mod tests {
    use crate::units::attributes::AttributeKey;

    use super::UnitTypeConfig;

    impl UnitTypeConfig {
        pub(crate) fn test(&self) {
            let keys = &self.attribute_keys;
            for key in keys {
                // no double-entries
                assert_eq!(1, keys.iter().filter(|a| *a == key).count());
            }
            let hidden_keys = &self.attribute_keys_hidden_by_fog;
            for key in hidden_keys {
                assert!(keys.contains(key));
                // no double-entries
                assert_eq!(1, hidden_keys.iter().filter(|a| *a == key).count());
            }
            if self.needs_owner {
                assert!(keys.contains(&AttributeKey::Owner));
                assert!(!hidden_keys.contains(&AttributeKey::Owner));
            }
            assert_eq!(
                self.transport.is_some(),
                keys.contains(&AttributeKey::Transported)
            );
            if keys.contains(&AttributeKey::Owner) {
                assert_eq!(keys[0], AttributeKey::Owner);
            }
        }
    }
}*/
