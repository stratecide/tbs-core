use std::str::FromStr;

use crate::config::ConfigParseError;
use crate::details::{MAX_STACK_SIZE, Detail};
use crate::game::event_handler::EventHandler;
use crate::map::direction::Direction;
use crate::map::point::Point;
use crate::player::Owner;
use crate::units::attributes::{AttributeKey, ActionStatus};
use crate::units::unit::Unit;

use super::player::resurrect_zombie;

#[derive(Debug, Clone)]
pub enum KillScript {
    Unexhaust,
    DeadSkull,
    ZombieResurrection(u8),
}

impl FromStr for KillScript {
    type Err = ConfigParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut it = s.split(&['(', ' ', '-', ')'])
        .map(str::trim);
        Ok(match it.next().unwrap() {
            "Unexhaust" => Self::Unexhaust,
            "DeadSkull" => Self::DeadSkull,
            "ZombieResurrection" => {
                let hp = it.next().ok_or(ConfigParseError::NotEnoughValues(s.to_string()))?;
                let hp = hp.parse().map_err(|_| ConfigParseError::InvalidInteger(hp.to_string()))?;
                Self::ZombieResurrection(1.max(100.min(hp)))
            }
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
                if details.len() < MAX_STACK_SIZE as usize && defender.get_team() != attacker.get_team()
                && handler.environment().unit_attributes(defender.typ(), attacker.get_owner_id()).any(|a| *a == AttributeKey::Zombified) {
                    handler.detail_add(defender_pos, Detail::Skull(Owner(attacker.get_owner_id()), defender.typ()));
                }
            },
            Self::ZombieResurrection(hp) => {
                resurrect_zombie(handler, defender_pos, attacker.get_owner_id(), *hp);
            }
        }
    }
}
