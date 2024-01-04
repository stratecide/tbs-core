use serde::Deserialize;

use crate::details::{MAX_STACK_SIZE, Detail};
use crate::game::event_handler::EventHandler;
use crate::map::direction::Direction;
use crate::map::point::Point;
use crate::units::attributes::{AttributeKey, ActionStatus};
use crate::units::unit::Unit;

#[derive(Debug, Deserialize)]
pub enum KillScript {
    Unexhaust,
    DeadSkull,
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
                    handler.detail_add(defender_pos, Detail::Skull(attacker.get_owner_id() as u8, defender.typ()));
                }
            },
        }
    }
}
