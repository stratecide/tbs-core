use num_rational::Rational32;

use crate::config::parse::{string_base, FromConfig};
use crate::config::ConfigParseError;
use crate::game::event_handler::EventHandler;
use crate::map::direction::Direction;
use crate::map::point::Point;
use crate::units::attributes::ActionStatus;
use crate::units::unit::Unit;

#[derive(Debug, Clone)]
pub enum AttackScript {
    LifeSteal(Rational32),
}

impl FromConfig for AttackScript {
    fn from_conf(s: &str) -> Result<(Self, &str), ConfigParseError> {
        let (base, mut s) = string_base(s);
        Ok((match base {
            "LifeSteal" => {
                let (lifesteal, r) = Rational32::from_conf(s)?;
                s = r;
                Self::LifeSteal(lifesteal)
            }
            invalid => return Err(ConfigParseError::UnknownEnumMember(format!("AttackScript::{}", invalid))),
        }, s))
    }
}

impl AttackScript {
    pub fn trigger<D: Direction>(&self, handler: &mut EventHandler<D>, attacker_pos: Option<(Point, Option<usize>)>, attacker: &Unit<D>, defender_pos: Point, defender: &Unit<D>, current_defender_pos: Option<(Point, Option<usize>)>, damage: u8) {
        match self {
            Self::LifeSteal(factor) => {
                if let Some((pos, unload_index)) = attacker_pos {
                    let health_change = (*factor * Rational32::from_integer(damage as i32)).floor().to_integer().max(-100).min(99) as i8;
                    if health_change == 0 {
                        return;
                    }
                    if let Some(index) = unload_index {
                        if health_change >= 0 {
                            handler.unit_heal_boarded(pos, index, health_change as u8);
                        } else {
                            handler.unit_damage_boarded(pos, index, (-health_change) as u8);
                            if handler.get_map().get_unit(pos).unwrap().get_transported()[index].get_hp() == 0 {
                                handler.unit_death_boarded(pos, index);
                            }
                        }
                    } else {
                        if health_change >= 0 {
                            handler.unit_heal(pos, health_change as u8);
                        } else {
                            handler.unit_damage(pos, (-health_change) as u16);
                            if handler.get_map().get_unit(pos).unwrap().get_hp() == 0 {
                                // if this triggered on_death effects, an infinite loop could be possible
                                handler.unit_death(pos);
                            }
                        }
                    }
                }
                match attacker_pos {
                    Some((p, None)) => handler.unit_status(p, ActionStatus::Ready),
                    Some((p, Some(index))) => handler.unit_status_boarded(p, index, ActionStatus::Ready),
                    None => (),
                }
            },
        }
    }
}
