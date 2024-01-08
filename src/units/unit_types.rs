use serde::Deserialize;
use zipper::*;

use crate::map::direction::Direction;
use crate::config::environment::Environment;

use super::unit::UnitBuilder;

crate::enum_with_custom! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize)]
    pub enum UnitType {
        HoverBike,
        SmallTank,
        DroneTower,
        LightDrone,
        Tentacle,
        Pyramid,
        Unknown,
    }
}

impl UnitType {
    /*pub fn attribute_keys(&self) -> &'static [AttributeKey] {
        use AttributeKey as A;
        match self {
            Self::HoverBike => &[A::Owner, A::Hero, A::Hp, A::ActionStatus, A::Amphibious],
            Self::SmallTank => &[A::Owner, A::Hero, A::Hp, A::ActionStatus],
            Self::DroneTower => &[A::Owner, A::Hp, A::ActionStatus, A::DroneStationId, A::Transported],
            Self::LightDrone => &[A::Owner, A::Hp, A::ActionStatus, A::DroneId],
            Self::Tentacle => &[A::Hp],
            Self::Pyramid => &[A::Owner, A::Hp],
            Self::Unknown => &[],
        }
    }

    fn attribute_keys_hidden_by_fog(&self) -> &'static [AttributeKey] {
        use AttributeKey as A;
        match self {
            Self::DroneTower => &[A::ActionStatus, A::DroneStationId, A::Transported],
            Self::Pyramid => &[],
            _ => &[],
        }
    }

    // should never return a list of size 1
    fn valid_action_status(&self) -> &'static [ActionStatus] {
        use ActionStatus as A;
        match self {
            Self::HoverBike => &[A::Ready, A::Exhausted, A::Capturing, A::Repairing],
            Self::SmallTank => &[A::Ready, A::Exhausted, A::Repairing],
            Self::DroneTower => &[A::Ready, A::Exhausted],
            Self::LightDrone => &[A::Ready, A::Exhausted],
            Self::Tentacle => &[],
            Self::Pyramid => &[],
            Self::Unknown => &[],
        }
    }

    // hm...
    // would it be better as 2 separate keys? no, then a unit could have both keys
    // or give a boolean to the key?
    pub(super) fn needs_owner(&self) -> bool {
        match self {
            Self::Unknown => false,
            Self::Tentacle => false,
            Self::Pyramid => false,
            _ => true
        }
    }

    fn transport_capacity(&self) -> usize {
        match self {
            Self::DroneTower => 3,
            _ => 0
        }
    }

    fn transports(&self) -> &'static [UnitType] {
        match self {
            Self::DroneTower => &[Self::LightDrone],
            _ => &[]
        }
    }

    fn visibility(&self) -> UnitVisibility {
        match self {
            Self::HoverBike => UnitVisibility::Normal,
            Self::SmallTank => UnitVisibility::Normal,
            Self::DroneTower => UnitVisibility::AlwaysVisible,
            Self::LightDrone => UnitVisibility::Normal,
            Self::Tentacle => UnitVisibility::Normal,
            Self::Pyramid => UnitVisibility::AlwaysVisible,
            Self::Unknown => UnitVisibility::Normal,
        }
    }*/

    pub fn price(&self, environment: &Environment, owner_id: i8) -> i32 {
        environment.unit_cost(*self, owner_id)
    }

    pub fn instance<D: Direction>(&self, environment: &Environment) -> UnitBuilder<D> {
        UnitBuilder::new(environment, *self)
    }
}

/*impl Display for UnitType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", match self {
            Self::HoverBike => "Hover-Bike",
            Self::SmallTank => "Small Tank",
            Self::DroneTower => "Drone Tower",
            Self::LightDrone => "Light Drone",
            Self::Tentacle => "Tentacle",
            Self::Pyramid => "Pyramid",
            Self::Unknown => "???",
            Self::Custom(id) => return write!(f, "Custom{id}"),
        })
    }
}*/

impl SupportedZippable<&Environment> for UnitType {
    fn export(&self, zipper: &mut Zipper, support: &Environment) {
        let index = support.config.unit_types().iter().position(|t| t == self).unwrap();
        let bits = bits_needed_for_max_value(support.config.unit_count() as u32 - 1);
        zipper.write_u32(index as u32, bits);
    }
    fn import(unzipper: &mut Unzipper, support: &Environment) -> Result<Self, ZipperError> {
        let bits = bits_needed_for_max_value(support.config.unit_count() as u32 - 1);
        let index = unzipper.read_u32(bits)? as usize;
        if index < support.config.unit_count() {
            Ok(support.config.unit_types()[index])
        } else {
            Err(ZipperError::EnumOutOfBounds(format!("UnitType index {}", index)))
        }
    }
}
