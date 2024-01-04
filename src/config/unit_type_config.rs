use std::collections::HashSet;

use serde::Deserialize;
use num_rational::Rational32;

use crate::game::fog::VisionMode;
use crate::script::unit::UnitScript;
use crate::units::attributes::*;
use crate::units::combat::*;
use crate::units::movement::MovementType;
use crate::units::unit_types::UnitType;

use super::movement_type_config::MovementPattern;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
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
    pub(super) can_take: bool,
    pub(super) can_be_taken: bool,
    pub(super) weapon: WeaponType,
    pub(super) can_attack_after_moving: bool,
    pub(super) attack_pattern: AttackType,
    pub(super) attack_targets: AttackTargeting,
    //#[serde(default)]
    //pub(super) splash_range: usize,
    //pub(super) splash_factor: Rational32,
    pub(super) splash_damage: Vec<Rational32>, // empty if no splash damage. contains factor per additional distance
    pub(super) can_build_units: bool,
    pub(super) cost: usize,
    pub(super) displacement: Displacement, // implies that attack_pattern is Adjacent or Straight
    pub(super) displacement_distance: i8, // can only be 0 if Displacement::None
    pub(super) can_be_displaced: bool,
    #[serde(default)]
    pub(super) transport_capacity: usize,
    #[serde(default)]
    pub(super) heal_transported: i8,
    //pub(super) transport: Option<UnitTransportConfig>,
    #[serde(default)]
    pub(super) build_overrides: HashSet<AttributeOverride>,
    #[serde(default)]
    pub(super) on_start_turn: Vec<UnitScript>,
    #[serde(default)]
    pub(super) on_death: Vec<UnitScript>,
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
