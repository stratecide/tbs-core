use num_rational::Rational32;

use crate::config::parse::{parse_tuple1, parse_tuple2, string_base, FromConfig};
use crate::config::ConfigParseError;
use crate::game::event_handler::EventHandler;
use crate::map::direction::Direction;
use crate::map::map_view::MapView;
use crate::map::point::Point;
use crate::units::movement::{Path, PathStep};
use crate::units::unit::Unit;

use super::unit::*;

#[derive(Debug, Clone)]
pub enum DefendScript {
    UnitScript(UnitScript),
    Ricochet(u16),
    Attack(bool, bool),
}

impl FromConfig for DefendScript {
    fn from_conf(s: &str) -> Result<(Self, &str), ConfigParseError> {
        let (base, mut remainder) = string_base(s);
        Ok((match base {
            "Ricochet" => {
                let (fizzle, r) = parse_tuple1(remainder)?;
                if fizzle < 1 {
                    return Err(ConfigParseError::InvalidInteger("Ricochet(0)".to_string()));
                }
                remainder = r;
                Self::Ricochet(fizzle)
            }
            "Attack" => {
                let (times_damage_taken, charge_powers, r) = parse_tuple2(remainder)?;
                remainder = r;
                Self::Attack(times_damage_taken, charge_powers)
            }
            invalid => {
                if let Ok((us, r)) = UnitScript::from_conf(s) {
                    remainder = r;
                    Self::UnitScript(us)
                } else {
                    return Err(ConfigParseError::UnknownEnumMember(format!("DefendScript::{}", invalid)))
                }
            }
        }, remainder))
    }
}

impl DefendScript {
    pub fn trigger<D: Direction>(&self, handler: &mut EventHandler<D>, defender: &Unit<D>, defender_pos: Point, unload_index: Option<usize>, _attacker_pos: Option<(Point, Option<usize>)>, _attacker: &Unit<D>, dir: Option<D>, damage: &mut u16) {
        match self {
            Self::UnitScript(us) => {
                us.trigger(handler, defender_pos, defender);
            }
            Self::Ricochet(fizzle) => {
                if let (None, Some(dir)) = (unload_index, dir) {
                    let mut path = Path::new(defender_pos);
                    while *damage as usize >= *fizzle as usize * (path.len() + 1) {
                        if let Ok((end, distortion)) = path.end(handler.get_map()) {
                            if end != defender_pos {
                                let terrain = handler.get_map().get_terrain(end).unwrap();
                                let u = handler.get_map().get_unit(end);
                                if u.is_some() || terrain.movement_cost(defender.default_movement_type()).is_none() {
                                    path.steps.pop();
                                    break;
                                }
                            }
                            path.steps.push(PathStep::Dir(distortion.update_direction(dir)));
                        } else {
                            path.steps.pop();
                            break;
                        }
                    }
                    if path.len() > 0 {
                        handler.unit_path(None, &path, false, true);
                        *damage -= *fizzle * path.len() as u16;
                    }
                }
            }
            Self::Attack(times_damage_taken, charge_powers) => {
                if unload_index.is_none() {
                    let input_factor = if *times_damage_taken {
                        Rational32::new(*damage as i32, 100)
                    } else {
                        Rational32::from_integer(1)
                    };
                    attack(handler, defender_pos, defender, false, *charge_powers, input_factor);
                }
            }
        }
    }
}
