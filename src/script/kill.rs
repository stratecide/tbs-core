use crate::config::parse::{parse_tuple1, string_base, FromConfig};
use crate::config::ConfigParseError;
use crate::details::{Detail, SkullData, MAX_STACK_SIZE};
use crate::game::event_handler::EventHandler;
use crate::map::direction::Direction;
use crate::map::point::Point;
use crate::units::attributes::{AttributeKey, ActionStatus};
use crate::units::unit::Unit;

use super::player::resurrect_zombie;
use super::unit::UnitScript;

#[derive(Debug, Clone)]
pub enum KillScript {
    UnitScript(UnitScript),
    Unexhaust,
    DeadSkull,
    ZombieResurrection(u8),
}

impl FromConfig for KillScript {
    fn from_conf(s: &str) -> Result<(Self, &str), ConfigParseError> {
        let (base, mut remainder) = string_base(s);
        Ok((match base {
            "Unexhaust" => Self::Unexhaust,
            "DeadSkull" => Self::DeadSkull,
            "ZombieResurrection" => {
                let (hp, r) = parse_tuple1(remainder)?;
                remainder = r;
                Self::ZombieResurrection(1.max(100.min(hp)))
            }
            invalid => {
                if let Ok((us, r)) = UnitScript::from_conf(s) {
                    remainder = r;
                    Self::UnitScript(us)
                } else {
                    return Err(ConfigParseError::UnknownEnumMember(format!("KillScript::{}", invalid)))
                }
            }
        }, remainder))
    }
}

impl KillScript {
    pub fn trigger<D: Direction>(&self, handler: &mut EventHandler<D>, attacker_pos: Option<(Point, Option<usize>)>, attacker: &Unit<D>, defender_pos: Point, defender: &Unit<D>) {
        match self {
            Self::UnitScript(us) => {
                if let Some((p, None)) = attacker_pos {
                    us.trigger(handler, p, attacker);
                }
            }
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
                    handler.detail_add(defender_pos, Detail::Skull(SkullData::new(defender, attacker.get_owner_id())));
                }
            },
            Self::ZombieResurrection(hp) => {
                resurrect_zombie(handler, defender_pos, attacker.get_owner_id(), *hp);
            }
        }
    }
}
