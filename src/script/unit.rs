use std::collections::{HashMap, HashSet};
use serde::Deserialize;

use crate::game::event_handler::EventHandler;
use crate::map::direction::Direction;
use crate::map::point::Point;
use crate::terrain::{KRAKEN_ATTACK_RANGE, KRAKEN_MAX_ANGER};
use crate::terrain::attributes::TerrainAttributeKey;
use crate::units::attributes::AttributeKey;
use crate::units::combat::{AttackType, AttackVector};
use crate::units::movement::Path;
use crate::units::unit::Unit;

#[derive(Debug, Clone, Deserialize)]
pub enum UnitScript {
    Kraken,
    Attack(bool, bool),
}

impl UnitScript {
    pub fn trigger<D: Direction>(&self, handler: &mut EventHandler<D>, position: Point, unit: &Unit<D>) {
        match self {
            Self::Kraken => anger_kraken(handler),
            Self::Attack(allow_counter, charge_powers) => attack(handler, position, unit, *allow_counter, *charge_powers),
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
                            let hp = unit.get_hp();
                            handler.unit_damage(p, damage);
                            if damage >= hp as u16 {
                                handler.unit_death(p, true);
                            }
                        }
                    }
                }
                handler.unit_mass_damage(&damage_map);
                let dead = damage_map.keys().cloned().filter(|p| {
                    let unit = handler.get_map().get_unit(*p).unwrap();
                    unit.get_hp() == 0
                }).collect();
                // if this triggered death effects, an infinite loop would be possible
                handler.unit_mass_death(dead, false);
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
            if let Some(dp) = handler.get_map().get_neighbor(position, unit.get_direction()) {
                AttackVector::DirectedPoint(dp.point, dp.direction)
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
    let path = if allow_counter {
        Some(Path::new(position))
    } else {
        None
    };
    attack_vector.execute(handler, position, path.as_ref(), false, false, charge_powers);
}
