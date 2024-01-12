use std::str::FromStr;

use serde::Deserialize;

use crate::config::ConfigParseError;
use crate::details::{MAX_STACK_SIZE, Detail};
use crate::game::event_handler::EventHandler;
use crate::map::direction::Direction;
use crate::map::point::Point;
use crate::player::Owner;
use crate::units::attributes::{AttributeKey, ActionStatus};
use crate::units::unit::Unit;

#[derive(Debug, Clone, Deserialize)]
pub enum KillScript {
    Unexhaust,
    DeadSkull,
}

impl FromStr for KillScript {
    type Err = ConfigParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut it = s.split(&['(', ' ', '-', ')'])
        .map(str::trim);
        Ok(match it.next().unwrap() {
            "Unexhaust" => Self::Unexhaust,
            "DeadSkull" => Self::DeadSkull,
            invalid => return Err(ConfigParseError::UnknownEnumMember(invalid.to_string())),
        })
    }
}

impl KillScript {
    pub fn trigger<D: Direction>(&self, handler: &mut EventHandler<D>, attacker_pos: Option<(Point, Option<usize>)>, attacker: &Unit<D>, defender_pos: Point, defender: &Unit<D>) {
        match self {
            Self::Unexhaust => {
                match attacker_pos {
                    Some((p, None)) => handler.unit_status(p, ActionStatus::Ready),
                    Some((p, Some(index))) => handler.unit_status_boarded(p, index, ActionStatus::Ready),
                    None => (),
                }
            },
            Self::DeadSkull => {
                let details = handler.get_map().get_details(defender_pos);
                if details.len() < MAX_STACK_SIZE as usize && defender.get_team() != attacker.get_team() && defender.has_attribute(AttributeKey::Zombified) {
                    handler.detail_add(defender_pos, Detail::Skull(Owner(attacker.get_owner_id()), defender.typ()));
                }
            },
        }
    }
}
