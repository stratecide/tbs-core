use std::collections::HashMap;

use crate::config::parse::{parse_tuple1, parse_tuple2, string_base, FromConfig};
use crate::config::ConfigParseError;
use crate::game::event_handler::EventHandler;
use crate::map::direction::Direction;
use crate::map::map_view::MapView;
use crate::map::point::Point;
use crate::terrain::{KRAKEN_ATTACK_RANGE, KRAKEN_MAX_ANGER};
use crate::terrain::attributes::TerrainAttributeKey;
use crate::units::attributes::AttributeKey;
use crate::units::combat::{AttackType, AttackVector};
use crate::units::unit::Unit;

#[derive(Debug, Clone)]
pub enum UnitScript {
    Kraken,
    Attack(bool, bool),
    TakeDamage(u8),
    Heal(u8),
}

impl FromConfig for UnitScript {
    fn from_conf(s: &str) -> Result<(Self, &str), ConfigParseError> {
        let (base, mut s) = string_base(s);
        Ok((match base {
            "Kraken" => Self::Kraken,
            "Attack" => {
                let (allow_counter, charge_powers, r) = parse_tuple2(s)?;
                s = r;
                Self::Attack(allow_counter, charge_powers)
            }
            "TakeDamage" => {
                let (damage, r) = parse_tuple1(s)?;
                s = r;
                Self::TakeDamage(1.max(damage))
            }
            "Heal" => {
                let (heal, r) = parse_tuple1(s)?;
                s = r;
                Self::Heal(1.max(heal).min(99))
            }
            invalid => return Err(ConfigParseError::UnknownEnumMember(format!("UnitScript::{}", invalid))),
        }, s))
    }
}

impl UnitScript {
    pub fn trigger<D: Direction>(&self, handler: &mut EventHandler<D>, position: Point, unit: &Unit<D>) {
        match self {
            Self::Kraken => anger_kraken(handler),
            Self::Attack(allow_counter, charge_powers) => attack(handler, position, unit, *allow_counter, *charge_powers),
            Self::TakeDamage(damage) => take_damage(handler, position, *damage),
            Self::Heal(h) => heal(handler, position, *h),
        }
    }
}

pub(super) fn anger_kraken<D: Direction>(handler: &mut EventHandler<D>) {
    for p in handler.get_map().all_points() {
        let terrain = handler.get_map().get_terrain(p).unwrap();
        if terrain.has_attribute(TerrainAttributeKey::Anger) {
            let anger = (terrain.get_anger() as usize + 1) % (KRAKEN_MAX_ANGER + 1);
            if anger == 0 {
                handler.effect_kraken_rage(p);
                let mut damage_map = HashMap::new();
                for p in handler.get_map().range_in_layers(p, KRAKEN_ATTACK_RANGE).into_iter().flatten() {
                    if let Some(unit) = handler.get_map().get_unit(p) {
                        if unit.get_owner_id() > 0 {
                            let damage = damage_map.remove(&p).unwrap_or(0) + 40;
                            damage_map.insert(p, damage);
                        }
                    }
                }
                handler.unit_mass_damage(&damage_map);
                let dead = damage_map.keys().cloned().filter(|p| {
                    let unit = handler.get_map().get_unit(*p).unwrap();
                    unit.get_hp() == 0
                }).collect();
                // if this triggered on_death effects, an infinite loop would be possible
                handler.unit_mass_death(&dead);
            }
            handler.terrain_anger(p, anger as u8);
        }
    }
}

fn attack<D: Direction>(handler: &mut EventHandler<D>, position: Point, unit: &Unit<D>, allow_counter: bool, charge_powers: bool) {
    let attack_vector = match unit.attack_pattern() {
        AttackType::None => return,
        AttackType::Adjacent |
        AttackType::Straight(_, _) => {
            if !unit.has_attribute(AttributeKey::Direction) {
                return;
            }
            AttackVector::Direction(unit.get_direction())
        },
        AttackType::Triangle(_, _) => {
            if !unit.has_attribute(AttributeKey::Direction) {
                return;
            }
            if let Some((point, distortion)) = handler.get_map().get_neighbor(position, unit.get_direction()) {
                AttackVector::DirectedPoint(point, distortion.update_direction(unit.get_direction()))
            } else {
                return;
            }
        }
        AttackType::Ranged(_, _) => {
            /*let splash_damage = unit.get_splash_damage();
            let mut targets = Vec::new();
            for (i, layer) in handler.get_map().range_in_layers(position, min as usize + splash_damage.len() - 1).into_iter().enumerate() {
                if i + 1 < min as usize {
                    continue;
                }
                for p in layer {
                    targets.push((p, None, splash_damage[i - 1 - min as usize]));
                }
            }
            let path = None;
            let (
                mut hero_charge,
                _,
                mut defenders,
            ) = attack_targets(handler, unit, position, path, targets);
            let defenders = if !charge_powers {
                hero_charge = 0;
                Vec::new()
            } else {
                attacked_units.into_iter().
                for (defender_id, defender_pos, defender, damage) in attacked_units {
                    for script in &attack_scripts {
                        script.trigger(handler, handler.get_observed_unit(unit_id).cloned(), unit, defender_pos, &defender, handler.get_observed_unit(defender_id).cloned(), damage);
                    }
                    defenders.push((unit.get_owner_id(), defender, damage));
                }
            }
            after_attacking(handler, unit, position, hero_charge, defenders);*/
            return;
        }
    };
    // TODO: allow_counter is currently ignored
    attack_vector.execute(handler, position, None, false, false, charge_powers);
}

pub(super) fn take_damage<D: Direction>(handler: &mut EventHandler<D>, position: Point, damage: u8) {
    if handler.get_map().get_unit(position).is_some() {
        handler.unit_damage(position, damage as u16);
        if handler.get_map().get_unit(position).unwrap().get_hp() == 0 {
            handler.unit_death(position);
        }
    }
}

pub(super) fn heal<D: Direction>(handler: &mut EventHandler<D>, position: Point, heal: u8) {
    if handler.get_map().get_unit(position).is_some() {
        handler.unit_heal(position, heal);
    }
}
