use std::collections::{HashSet, HashMap};

use num_rational::Rational32;
use serde::Deserialize;
use zipper_derive::Zippable;

use crate::game::events::Effect;
use crate::game::game::Game;
use crate::game::fog::FogIntensity;
use crate::game::event_handler::EventHandler;
use crate::map::point::Point;
use crate::map::direction::Direction;
use crate::map::map::{Map, NeighborMode};
use crate::map::wrapping_map::OrientedPoint;
use crate::script::attack::AttackScript;
use crate::script::kill::KillScript;

use super::attributes::{AttributeKey, ActionStatus};
use super::movement::{PathStep, get_diagonal_neighbor};
use super::{unit::Unit, movement::Path};


#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum AttackType {
    None,
    Adjacent,
    Ranged(u8, u8),
    // can get blocked by units that stand in the way
    // TODO: ensure that min and max are >0 in config
    Straight(u8, u8),
    Triangle(u8, u8),
}

impl AttackType {
    pub fn attackable_positions<D: Direction>(&self, map: &Map<D>, position: Point, unit: &Unit<D>) -> HashSet<Point> {
        let mut result = HashSet::new();
        match self {
            Self::None => (),
            Self::Adjacent => {
                for p in map.get_neighbors(position, NeighborMode::FollowPipes) {
                    result.insert(p.point);
                }
            }
            Self::Straight(min_range, max_range) => {
                for d in D::list() {
                    let mut current_pos = OrientedPoint::new(position, false, d);
                    for i in 0..*max_range {
                        if let Some(dp) = map.get_neighbor(current_pos.point, current_pos.direction) {
                            if i + 1 >= *min_range {
                                result.insert(dp.point);
                            } else if map.get_unit(dp.point).is_some() {
                                break;
                            }
                            current_pos = dp;
                        } else {
                            break;
                        }
                    }
                }
            }
            Self::Ranged(min_range, max_range) => {
                let range_bonus = map.get_terrain(position).unwrap().range_bonus(unit);
                let min_range = min_range + range_bonus;
                let max_range = max_range + range_bonus;
                // each point in a layer is probably in it 2 times
                let mut layers = map.range_in_layers(position, max_range as usize);
                for _ in min_range-1..max_range {
                    for p in layers.pop().unwrap() {
                        result.insert(p);
                    }
                }
            }
            Self::Triangle(min_range, max_range) => {
                if !unit.has_attribute(AttributeKey::Direction) {
                    return HashSet::new();
                }
                let direction = unit.get_direction();
                if let Some(dp) = map.get_neighbor(position, direction) {
                    attack_area_cannon(map, dp, *min_range as usize, *max_range as usize, |dp, _| {
                        result.insert(dp.point);
                    });
                }
            }
        }
        result
    }
}

fn attack_area_cannon<D: Direction, F: FnMut(OrientedPoint<D>, usize)>(map: &Map<D>, dp: OrientedPoint<D>, min_range: usize, max_range: usize, mut callback: F) {
    if D::is_hex() {
        let mut old_front = HashMap::new();
        let mut front = HashMap::new();
        front.insert(dp, true);
        for i in 0..(max_range * 2 - 1) {
            let older_front = old_front;
            old_front = front;
            front = HashMap::new();
            for (position, _) in older_front {
                if i >= min_range {
                    callback(position, i);
                }
                if let Some(dp) = map.get_neighbor(position.point, position.direction) {
                    front.insert(dp, true);
                }
            }
            // in order to not spread too much, only spread if
            //      - previously moved straight forward
            //      - current position was spread to from both sides
            for (position, may_spread) in &old_front {
                if *may_spread {
                    if let Some(dp) = map.get_neighbor(position.point, position.direction.rotate(true)) {
                        let key = OrientedPoint::new(dp.point, dp.mirrored != position.mirrored, dp.direction.rotate(dp.mirrored));
                        front.insert(key, front.contains_key(&key));
                    }
                    if let Some(dp) = map.get_neighbor(position.point, position.direction.rotate(false)) {
                        let key = OrientedPoint::new(dp.point, dp.mirrored != position.mirrored, dp.direction.rotate(!dp.mirrored));
                        front.insert(key, front.contains_key(&key));
                    }
                }
            }
        }
        for position in old_front.keys() {
            callback(*position, max_range * 2 - 1);
        }
        for position in front.keys() {
            callback(*position, max_range * 2);
        }
    } else {
        let mut front = HashSet::new();
        front.insert(dp);
        for i in 0..max_range {
            let old_front = front;
            front = HashSet::new();
            for position in old_front {
                if i * 2 >= min_range {
                    callback(position, i * 2);
                }
                if let Some(dp) = map.get_neighbor(position.point, position.direction) {
                    front.insert(dp);
                }
                if let Some(dp) = get_diagonal_neighbor(map, position.point, position.direction) {
                    front.insert(dp);
                }
                if let Some(dp) = get_diagonal_neighbor(map, position.point, position.direction.rotate(true)) {
                    front.insert(OrientedPoint::new(dp.point, dp.mirrored != position.mirrored, dp.direction.rotate(dp.mirrored)));
                }
            }
        }
        for position in front {
            callback(position, max_range * 2);
        }
    }
}


#[derive(Debug, Clone, PartialEq, Zippable)]
#[zippable(bits = 2)]
pub enum AttackVector<D: Direction> {
    // prefer Direction over Point whenever possible
    Direction(D),
    Point(Point),
    DirectedPoint(Point, D),
}

impl<D: Direction> AttackVector<D> {
    // returns all AttackVectors with which the unit at target position can be attacked
    // if there is no unit or it can't be attacked, an empty Vec is returned
    // TODO: make sure the same Self isn't returned multiple times
    pub fn find(attacker: &Unit<D>, game: &Game<D>, pos: Point, target: Point, get_fog: impl Fn(Point) -> FogIntensity) -> Vec<Self> {
        let defender = match game.get_map().get_unit(target)
        .and_then(|u| u.fog_replacement(game.get_map().get_terrain(target).unwrap(), get_fog(target))) {
            None => return Vec::new(),
            Some(defender) => defender,
        };
        if !attacker.could_attack(&defender) {
            return Vec::new();
        }
        // TODO: check if target is protected by terrain (e.g. tank can only attack stranded Submarines)
        let mut result = Vec::new();
        let splash_damage = attacker.get_splash_damage();
        match attacker.attack_pattern() {
            AttackType::None => (),
            AttackType::Straight(min, max) => {
                for d in D::list() {
                    /*let points: Vec<_> = game.get_map().get_line(pos, d, max as usize + splash_damage.len(), NeighborMode::FollowPipes)
                    .into_iter()
                    .skip(1) // skip pos
                    //.map(|dp| dp.point)
                    .collect();
                    if !points.iter().skip(min as usize - 1).any(|dp| dp.point == target) {
                        continue;
                    }
                    let blocking_unit = points.iter().position(|dp| {
                        game.get_map().get_unit(dp.point)
                            .and_then(|u| u.fog_replacement(game.get_map().get_terrain(dp.point).unwrap(), get_fog(dp.point)))
                            .is_some()
                    });
                    if blocking_unit.and_then(|i| Some(i + 1 < min as usize)).unwrap_or(false) {
                        // blocked by unit before min range
                        continue;
                    }
                    if points.iter().enumerate().skip(min as usize - 1).any(|(i, dp)| {
                        if dp.point == target && i < blocking_unit.unwrap_or(i) + splash_damage.len() {
                            let splash_index = i - blocking_unit.unwrap_or(i).min(i);
                            if splash_damage[i] != Rational32::from_integer(0) && attacker.base_damage(defender.typ()) != Some(0) {
                                true
                            } else if attacker.displacement() == Displacement::None {
                                false
                            } else {
                                // check if displacement would succeed
                                let target_points = if attacker.displacement_distance() < 0 {
                                    game.get_map().get_line(dp.point, dp.direction.opposite_direction(), (-attacker.displacement_distance()) as usize + 1, NeighborMode::FollowPipes)
                                } else {
                                    game.get_map().get_line(dp.point, dp.direction, attacker.displacement_distance() as usize + splash_damage.len() - splash_index, NeighborMode::FollowPipes)
                                };
                                target_points
                                .into_iter()
                                .skip(1)
                                .any(|dp| {
                                    // not blocked by the attacker or any other unit
                                    dp.point != pos &&
                                    game.get_map().get_unit(dp.point)
                                    .and_then(|u| u.fog_replacement(game.get_map().get_terrain(dp.point).unwrap(), get_fog(dp.point)))
                                    .is_none()
                                })
                            }
                        } else {
                            false
                        }
                    }) {
                        result.push(Self::Direction(d));
                    }*/
                    if Self::straight_splash(attacker, game, pos, d, min as usize, max as usize, get_fog).iter()
                    .any(|(dp, _)| dp.point == target) {
                        result.push(Self::Direction(d));
                    }
                }
            }
            AttackType::Adjacent => {
                for d in D::list() {
                    if let Some(dp) = game.get_map().get_neighbor(pos, d) {
                        if dp.point == target {
                            result.push(Self::Direction(d));
                        }
                    }
                }
            }
            AttackType::Ranged(min, max) => {
                if attacker.attack_pattern().attackable_positions(game.get_map(), pos, attacker).contains(&target) {
                    // TODO: consider splash damage
                    result.push(Self::Point(target));
                }
            }
            AttackType::Triangle(min, max) => {
                if attacker.has_attribute(AttributeKey::Direction) {
                    let direction = attacker.get_direction();
                    if let Some(dp) = game.get_map().get_neighbor(pos, direction) {
                        attack_area_cannon(game.get_map(), dp, min as usize, max as usize, |dp, _| {
                            if dp.point == target {
                                // TODO: consider splash damage
                                result.push(Self::DirectedPoint(dp.point, dp.direction));
                            }
                        });
                    }
                }
            }
        }
        result
    }

    fn straight_splash(attacker: &Unit<D>, game: &Game<D>, pos: Point, dir: D, min: usize, max: usize, get_fog: impl Fn(Point) -> FogIntensity) -> Vec<(OrientedPoint<D>, Rational32)> {
        let splash_damage = attacker.get_splash_damage();
        let points: Vec<_> = game.get_map().get_line(pos, dir, max + splash_damage.len(), NeighborMode::FollowPipes)
        .into_iter()
        .skip(1) // skip pos
        //.map(|dp| dp.point)
        .collect();
        let blocking_unit = points.iter().enumerate().position(|(i, dp)| {
            i < max && game.get_map().get_unit(dp.point)
                .and_then(|u| u.fog_replacement(game.get_map().get_terrain(dp.point).unwrap(), get_fog(dp.point)))
                .is_some()
        });
        if blocking_unit.and_then(|i| Some(i + 1 < min)).unwrap_or(false) {
            // blocked by unit before min range
            return Vec::new();
        }
        if points.iter()
        .skip(blocking_unit.unwrap_or(max - 1))
        .take(splash_damage.len())
        .enumerate()
        .any(|(i, dp)| {
            let defender = match game.get_map().get_unit(dp.point)
            .and_then(|u| u.fog_replacement(game.get_map().get_terrain(dp.point).unwrap(), get_fog(dp.point))) {
                Some(u) => u,
                _ => return false,
            };
            if !attacker.could_attack(&defender) {
                return false;
            }
            // TODO: check if target is protected by terrain (e.g. tank can only attack stranded Submarines)
            if splash_damage[i] != Rational32::from_integer(0) && attacker.base_damage(defender.typ()) != Some(0) {
                true
            } else if attacker.displacement() == Displacement::None {
                false
            } else {
                // check if displacement would succeed
                let target_points = if attacker.displacement_distance() < 0 {
                    game.get_map().get_line(dp.point, dp.direction.opposite_direction(), (-attacker.displacement_distance()) as usize + 1, NeighborMode::FollowPipes)
                } else {
                    game.get_map().get_line(dp.point, dp.direction, attacker.displacement_distance() as usize + splash_damage.len() - i, NeighborMode::FollowPipes)
                };
                target_points
                .into_iter()
                .skip(1)
                .any(|dp| {
                    // not blocked by the attacker or any other unit
                    dp.point != pos &&
                    game.get_map().get_unit(dp.point)
                    .and_then(|u| u.fog_replacement(game.get_map().get_terrain(dp.point).unwrap(), get_fog(dp.point)))
                    .is_none()
                })
            }
        }) {
            points.into_iter()
            .skip(blocking_unit.unwrap_or(max - 1))
            .zip(splash_damage.iter().cloned())
            .collect()
        } else {
            Vec::new()
        }
    }

    fn adjacent_splash(attacker: &Unit<D>, game: &Game<D>, pos: Point, dir: D) -> Vec<(OrientedPoint<D>, Rational32)> {
        let splash_damage = attacker.get_splash_damage();
        let mut result = Vec::new();
        if let Some(dp) = game.get_map().get_neighbor(pos, dir) {
            result.push((dp, splash_damage[0]));
        }
        let mut clockwise = dir;
        let mut counter_clockwise = dir;
        for i in 1..splash_damage.len() {
            clockwise = clockwise.rotate(true);
            counter_clockwise = counter_clockwise.rotate(false);
            if let Some(dp) = game.get_map().get_neighbor(pos, clockwise) {
                result.push((dp, splash_damage[i]));
            }
            if let Some(dp) = game.get_map().get_neighbor(pos, counter_clockwise) {
                result.push((dp, splash_damage[i]));
            }
        }
        result
    }

    fn ranged_splash(attacker: &Unit<D>, game: &Game<D>, pos: Point) -> Vec<(Point, Rational32)> {
        let splash_damage = attacker.get_splash_damage();
        let mut result = vec![(pos, splash_damage[0])];
        for (i, ring) in game.get_map().range_in_layers(pos, splash_damage.len() - 1).into_iter().enumerate() {
            for p in ring {
                result.push((p, splash_damage[i + 1]));
            }
        }
        result
    }

    fn triangle_splash(attacker: &Unit<D>, game: &Game<D>, pos: Point, dir: D) -> Vec<(OrientedPoint<D>, Rational32)> {
        let splash_damage = attacker.get_splash_damage();
        let mut result = Vec::new();
        attack_area_cannon(game.get_map(), OrientedPoint::new(pos, false, dir), 0, (splash_damage.len() + 1) / 2, |dp, splash_index| {
            if splash_index < splash_damage.len() {
                result.push((dp, splash_damage[splash_index]));
            }
        });
        result
    }

    pub fn get_splash(&self, attacker: &Unit<D>, game: &Game<D>, pos: Point, get_fog: impl Fn(Point) -> FogIntensity) -> Vec<(Point, Option<D>, Rational32)> {
        match (attacker.attack_pattern(), self) {
            (AttackType::None, _) => Vec::new(),
            (AttackType::Straight(min, max), Self::Direction(dir)) => {
                Self::straight_splash(attacker, game, pos, *dir, min as usize, max as usize, get_fog).into_iter()
                .map(|(dp, ratio)| (dp.point, Some(dp.direction), ratio))
                .collect()
            }
            (AttackType::Adjacent, Self::Direction(dir)) => {
                Self::adjacent_splash(attacker, game, pos, *dir).into_iter()
                .map(|(dp, ratio)| (dp.point, Some(dp.direction), ratio))
                .collect()
            }
            (AttackType::Ranged(min, max), Self::Point(p)) => {
                Self::ranged_splash(attacker, game, *p).into_iter()
                .map(|(p, ratio)| (p, None, ratio))
                .collect()
            }
            (AttackType::Triangle(min, max), Self::DirectedPoint(pos, dir)) => {
                Self::triangle_splash(attacker, game, *pos, *dir).into_iter()
                .map(|(dp, ratio)| (dp.point, Some(dp.direction), ratio))
                .collect()
            }
            _ => panic!("AttackPattern is incompatible with AttackVector"),
        }
    }

    /**
     * returns the new position of the attacker or None if the attacker died
     */
    pub fn execute(&self, handler: &mut EventHandler<D>, attacker_pos: Point, path: Option<&Path<D>>, exhaust_after_attacking: bool, execute_scripts: bool, charge_powers: bool) -> Option<(Point, Option<usize>)> {
        let attacker = handler.get_map().get_unit(attacker_pos).unwrap();
        let attacker_id = handler.observe_unit(attacker_pos, None);
        let (
            mut hero_charge,
            counter_attackers,
            mut defenders,
        ) = self.execute_attack(handler, attacker, attacker_pos, path, exhaust_after_attacking, execute_scripts);
        // counter attack
        if path.is_some() {
            for unit_id in counter_attackers {
                let mut attacker_pos = match handler.get_observed_unit(attacker_id) {
                    Some((p, _)) => *p,
                    _ => break,
                };
                let unit_pos = match handler.get_observed_unit(unit_id) {
                    Some((p, None)) => *p,
                    _ => continue,
                };
                let unit = handler.get_map().get_unit(unit_pos).expect(&format!("didn't find counter attacker at {unit_pos:?}"));
                if !handler.get_game().can_see_unit_at(unit.get_team(), attacker_pos, attacker, true)
                || !unit.could_attack(attacker)
                || attacker.get_team() == unit.get_team() {
                    continue;
                }
                let mut attack_vectors = AttackVector::find(unit, handler.get_game(), unit_pos, attacker_pos, |_| FogIntensity::TrueSight);
                let mut attack_vector = attack_vectors.pop();
                if let AttackVector::Direction(d) = self {
                    let inverse = AttackVector::Direction(d.opposite_direction());
                    if attack_vectors.contains(&inverse) {
                        attack_vector = Some(inverse);
                    }
                }
                if let Some(attack_vector) = attack_vector {
                    let (_, _, d2) = attack_vector.execute_attack(handler, unit, unit_pos, None, false, execute_scripts);
                    defenders.extend(d2.into_iter());
                }
            }
        }
        if !charge_powers {
            hero_charge = 0;
            defenders = Vec::new();
        }
        after_attacking(handler, attacker, attacker_pos, hero_charge, defenders);
        handler.get_observed_unit(attacker_id).cloned()
    }

    // set path to None if this is a counter-attack
    fn execute_attack(&self, handler: &mut EventHandler<D>, attacker: &Unit<D>, attacker_pos: Point, path: Option<&Path<D>>, exhaust_after_attacking: bool, execute_scripts: bool) -> (u32, Vec<usize>, Vec<(i8, Unit<D>, u8)>) {
        let mut defenders = self.get_splash(attacker, handler.get_game(), attacker_pos, |_| FogIntensity::TrueSight);
        attack_targets(handler, attacker, attacker_pos, path, defenders, exhaust_after_attacking, execute_scripts)
    }
}

pub(crate) fn attack_targets<D: Direction>(handler: &mut EventHandler<D>, attacker: &Unit<D>, attacker_pos: Point, path: Option<&Path<D>>, targets: Vec<(Point, Option<D>, Rational32)>, exhaust_after_attacking: bool, execute_scripts: bool) -> (u32, Vec<usize>, Vec<(i8, Unit<D>, u8)>) {
    let attacker_id = handler.observe_unit(attacker_pos, None);
    let mut defenders = filter_attack_targets(handler.get_game(), attacker, targets);
    let is_counter = path.is_none();
    let mut recalculate_fog = false;
    let mut hero_charge = 0;
    let mut attacked_units = Vec::new();
    let mut killed_units = Vec::new();
    match attacker.displacement() {
        Displacement::None => {
            (defenders, hero_charge, attacked_units, killed_units) = deal_damage(handler, attacker, attacker_pos, path, defenders);
        }
        Displacement::BeforeAttack => {
            defenders = displace(handler, attacker, defenders);
            (defenders, hero_charge, attacked_units, killed_units) = deal_damage(handler, attacker, attacker_pos, path, defenders);
        }
        Displacement::BetweenAttacks => {
            (defenders, hero_charge, attacked_units, killed_units) = deal_damage(handler, attacker, attacker_pos, path, defenders);
            defenders = displace(handler, attacker, defenders);
        }
        Displacement::InsteadOfAttack => {
            defenders = displace(handler, attacker, defenders);
            let mut collided = Vec::new();
            for (p, d, ratio) in defenders {
                if let Some(d) = d {
                    collided.push((p, None, ratio));
                    if let Some(dp) = handler.get_map().get_neighbor(p, d) {
                        if handler.get_map().get_unit(dp.point).is_some() {
                            collided.push((dp.point, None, ratio));
                        }
                    }
                }
            }
            // units that couldn't be fully displaced take damage
            (defenders, hero_charge, attacked_units, killed_units) = deal_damage(handler, attacker, attacker_pos, path, collided);
        }
    }
    /*for (target, displacement_dir, factor) in splash_area {
        if let Some(defender) = handler.get_map().get_unit(target) {
            let damage = defender.calculate_attack_damage(handler.get_game(), target, attacker_pos, attacker, path);
            if let Some((weapon, damage)) = damage {
                let hp = defender.get_hp();
                if !is_counter && defender.get_owner() != Some(attacker.get_owner()) {
                    for (p, _) in handler.get_map().mercenary_influence_at(attacker_pos, Some(attacker.get_owner())) {
                        let change = if p == attacker_pos {
                            3
                        } else {
                            1
                        };
                        charges.insert(p, charges.get(&p).unwrap_or(&0) + change);
                    }
                }
                defenders.push((target.clone(), defender.clone(), damage));
                let defender = defender.clone();
                handler.effect_weapon(target, weapon);
                handler.unit_damage(target.clone(), damage);
                if damage >= hp as u16 {
                    dead_units.insert(target);
                    handler.unit_death(target, true);
                    if handler.get_game().get_team(Some(attacker.get_owner())) != handler.get_game().get_team(defender.get_owner()) {
                        if let Some(commander) = handler.get_game().get_owning_player(attacker.get_owner()).and_then(|player| Some(player.commander.clone())) {
                            commander.after_killing_unit(handler, attacker.get_owner(), target, &defender);
                        }
                    }
                    recalculate_fog = true;
                } else {
                    potential_counters.push(target);
                }
            }
        }
    }
    // add charge to nearby mercs
    for (p, change) in charges {
        if !dead_units.contains(&p) {
            handler.mercenary_charge_add(p, change);
        }
    }
    // add charge to commanders of involved players
    if defenders.len() > 0 {
        let attacker_team = handler.get_game().get_team(Some(attacker.get_owner()));
        let mut charges = HashMap::new();
        for (_, defender, damage) in &defenders {
            if let Some(player) = defender.get_owner().and_then(|owner| handler.get_game().get_owning_player(owner)) {
                if ClientPerspective::Team(*player.team as u8) != attacker_team {
                    let commander_charge = defender.get_hp().min(*damage as u8) as u32 * defender.type_value() as u32 / 100;
                    let old_charge = charges.remove(&player.owner_id).unwrap_or(0);
                    charges.insert(player.owner_id, commander_charge + old_charge);
                    let old_charge = charges.remove(&attacker.get_owner()).unwrap_or(0);
                    charges.insert(attacker.get_owner(), commander_charge / 2 + old_charge);
                }
            }
        }
        for (owner, commander_charge) in charges {
            handler.commander_charge_add(owner, commander_charge);
        }
        if let Some(commander) = handler.get_game().get_owning_player(attacker.get_owner()).and_then(|player| Some(player.commander.clone())) {
            commander.after_attacking(handler, attacker_pos, attacker, defenders, is_counter);
        }
    }
    if recalculate_fog {
        handler.recalculate_fog();
    }*/

    if exhaust_after_attacking {
        match handler.get_observed_unit(attacker_id) {
            Some((p, None)) => handler.unit_status(*p, ActionStatus::Exhausted),
            Some((p, Some(index))) => handler.unit_status_boarded(*p, *index, ActionStatus::Exhausted),
            None => (),
        }
    }
    let mut counter_attackers = Vec::new();
    let mut defenders = Vec::new();
    for (defender_id, _, defender, damage) in attacked_units {
        if attacker.get_team() != defender.get_team() && !counter_attackers.contains(&defender_id) {
            counter_attackers.push(defender_id);
        }
        defenders.push((attacker.get_owner_id(), defender, damage));
    }
    if execute_scripts {
        let attack_scripts: Vec<&AttackScript> = attacker.get_attack_scripts(handler.get_game(), attacker_pos);
        for (defender_id, defender_pos, defender, damage) in attacked_units {
            for script in &attack_scripts {
                script.trigger(handler, handler.get_observed_unit(attacker_id).cloned(), attacker, defender_pos, &defender, handler.get_observed_unit(defender_id).cloned(), damage);
            }
        }
        let kill_scripts: Vec<&KillScript> = attacker.get_kill_scripts(handler.get_game(), attacker_pos);
        for (defender_pos, defender) in killed_units {
            for script in &kill_scripts {
                script.trigger(handler, handler.get_observed_unit(attacker_id).cloned(), attacker, defender_pos, &defender);
            }
        }
    }
    (hero_charge, counter_attackers, defenders)
}

fn filter_attack_targets<D: Direction>(game: &Game<D>, attacker: &Unit<D>, targets: Vec<(Point, Option<D>, Rational32)>) -> Vec<(Point, Option<D>, Rational32)> {
    targets.into_iter()
    .filter(|(p, _, _)| {
        if let Some(defender) = game.get_map().get_unit(*p) {
            attacker.could_attack(defender)
        } else {
            false
        }
    })
    .collect()
}

fn deal_damage<D: Direction>(handler: &mut EventHandler<D>, attacker: &Unit<D>, attacker_pos: Point, path: Option<&Path<D>>, targets: Vec<(Point, Option<D>, Rational32)>) -> (Vec<(Point, Option<D>, Rational32)>, u32, Vec<(usize, Point, Unit<D>, u8)>, Vec<(Point, Unit<D>)>) {
    let is_counter = path.is_none();
    let mut raw_damage = HashMap::new();
    let mut hero_charge = 0;
    for (defender_pos, _, factor) in targets.iter().cloned() {
        let defender = handler.get_map().get_unit(defender_pos).unwrap();
        let hp = defender.get_hp();
        if hp == 0 {
            continue;
        }
        let damage = calculate_attack_damage(handler.get_game(), attacker, attacker_pos, path, defender, defender_pos);
        if damage == 0 {
            continue;
        }
        let defender = defender.clone();
        handler.effect_weapon(defender_pos, attacker.weapon());
        if damage > 0 {
            if attacker.get_team() != defender.get_team() {
                hero_charge += 1;
            }
            let previous_damage = raw_damage.remove(&defender_pos).unwrap_or(0);
            raw_damage.insert(defender_pos, previous_damage + damage as u16);
        } else {
            handler.unit_heal(defender_pos.clone(), (-damage) as u8);
        }
    }
    let attack_script_targets: Vec<(usize, Point, Unit<D>, u8)> = raw_damage.iter()
    .map(|(p, raw_damage)| {
        let unit = handler.get_map().get_unit(*p).unwrap().clone();
        let hp = unit.get_hp() as u16;
        (handler.observe_unit(*p, None), *p, unit, hp.min(*raw_damage) as u8)
    })
    //.filter(|(_, _, u, _)| u.get_team() != attacker.get_team())
    .collect();
    /*let defenders: Vec<(Unit<D>, u8)> = attack_script_targets.iter()
    .map(|(_, unit, damage)| {
        (unit.clone(), *damage)
    })
    .collect();*/
    handler.unit_mass_damage(raw_damage);
    // destroy defeated units
    let dead_units: Vec<(Point, Unit<D>)> = raw_damage.keys()
    .filter_map(|p| handler.get_map().get_unit(*p).and_then(|u| {
        if u.get_hp() == 0 {
            Some((*p, u.clone()))
        } else {
            None
        }
    }))
    .collect();
    let deaths: HashSet<Point> = dead_units.iter()
    .map(|(p, _)| *p)
    .collect();
    handler.unit_mass_death(deaths, true);
    // commander effects
    /*attacker.get_commander(handler.get_game()).after_attacking(handler, attacker_pos, attacker, defenders.clone(), is_counter);
    for (p, corpse) in dead_units {
        corpse.get_commander(handler.get_game()).after_killing_unit(handler, corpse.get_owner_id(), p, &corpse);
    }*/
    //handler.recalculate_fog();
    (filter_attack_targets(handler.get_game(), attacker, targets), hero_charge, attack_script_targets, dead_units)
}

fn calculate_attack_damage<D: Direction>(game: &Game<D>, attacker: &Unit<D>, attacker_pos: Point, path: Option<&Path<D>>, defender: &Unit<D>, defender_pos: Point) -> i32 {
    let is_counter = path.is_none();
    let defensive_terrain = game.get_map().get_terrain(defender_pos).unwrap();
    let mut terrain_defense = Rational32::from_integer(1) + defensive_terrain.defense_bonus(defender);
    /*for t in game.get_map().get_neighbors(defender_pos, crate::map::map::NeighborMode::Direct).into_iter().map(|p| game.get_map().get_terrain(p.point).unwrap()) {
        terrain_defense += t.adjacent_defense_bonus(defender);
    }*/
    let mut defense_bonus = Rational32::from_integer(1);
    defense_bonus += defender.get_commander(game).defense_bonus(defender, game, defender_pos, is_counter);
    for (p, hero) in game.get_map().mercenary_influence_at(defender_pos, defender.get_owner_id()) {
        defense_bonus += hero.aura_defense_bonus(defender, is_counter);
    }
    let defense_bonus = defense_bonus; // to make sure it's not updated in the for-loop on accident
    let mut attack = Rational32::from_integer(attacker.base_damage(defender.typ()).unwrap() as i32);
    if let Some(path) = path {
        //attack *= attacker.attack_factor_from_path(game, path);
    } else {
        //attack *= attacker.attack_factor_from_counter(game);
    }
    let mut attack_bonus = Rational32::from_integer(1);
    attack_bonus += attacker.get_commander(game).attack_bonus(attacker, game, attacker_pos, is_counter);
    for (p, hero) in game.get_map().mercenary_influence_at(attacker_pos, attacker.get_owner_id()) {
        attack_bonus += hero.aura_attack_bonus(attacker, is_counter);
    }
    (Rational32::from_integer(attacker.get_hp() as i32) / 100 * attack * attack_bonus / defense_bonus / terrain_defense)
    .ceil().to_integer()
}

fn displace<D: Direction>(handler: &mut EventHandler<D>, attacker: &Unit<D>, mut targets: Vec<(Point, Option<D>, Rational32)>) -> Vec<(Point, Option<D>, Rational32)> {
    let distance = attacker.displacement_distance();
    if distance == 0 {
        return targets;
    }
    for (pos, dir, _) in targets.iter_mut().rev() {
        if let Some(d) = dir {
            let d = if distance < 0 {
                d.opposite_direction()
            } else {
                *d
            };
            let mut line = handler.get_map().get_line(*pos, d, distance.abs() as usize, NeighborMode::FollowPipes);
            while line.len() > 1 {
                let end = line.pop().unwrap().point;
                if handler.get_map().get_unit(end).is_none() {
                    let steps = line.into_iter()
                    .map(|dp| PathStep::Dir(dp.direction))
                    .collect();
                    let path = Path {
                        start: *pos,
                        steps,
                    };
                    *pos = end;
                    if path.steps.len() == distance.abs() as usize {
                        *dir = None;
                    }
                    handler.unit_path(None, &path, false, true);
                    break;
                }
            }
        }
    }
    filter_attack_targets(handler.get_game(), attacker, targets)
}

pub(crate) fn after_attacking<D: Direction>(handler: &mut EventHandler<D>, attacker: &Unit<D>, attacker_pos: Point, hero_charge: u32, defenders: Vec<(i8, Unit<D>, u8)>) {
    // add charge to heroes
    if hero_charge > 0 {
        for (p, _) in handler.get_map().mercenary_influence_at(attacker_pos, attacker.get_owner_id()) {
            let change = if p == attacker_pos {
                3
            } else {
                1
            } * hero_charge;
            handler.hero_charge_add(p, change.min(u8::MAX as u32) as u8);
        }
    }
    // add charge to commanders
    if defenders.len() > 0 {
        let mut charges = HashMap::new();
        for (attacker_owner, defender, damage) in defenders {
            let commander_charge = Rational32::from_integer(damage as i32) * Rational32::from_integer(defender.typ().price(handler.environment(), defender.get_owner_id())) / Rational32::from_integer(100);
            let old_charge = charges.remove(&defender.get_owner_id()).unwrap_or(0);
            charges.insert(defender.get_owner_id(), old_charge + commander_charge.ceil().to_integer());
            let old_charge = charges.remove(&attacker_owner).unwrap_or(0);
            charges.insert(attacker_owner, old_charge + (commander_charge / Rational32::from_integer(2)).ceil().to_integer());
        }
        for (owner, commander_charge) in charges {
            handler.commander_charge_add(owner, commander_charge as u32);
        }
    }
    // units may have died, hero/co ability may change fog
    handler.recalculate_fog();
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Deserialize)]
pub enum WeaponType {
    MachineGun,
    Shells,
    AntiAir,
    Flame,
    Rocket,
    Torpedo,
    Rifle,
    Bombs,
    // immobile ranged
    SurfaceMissiles,
    AirMissiles,
}

impl WeaponType {
    pub fn effect<D: Direction>(&self, p: Point) -> Effect<D> {
        match self {
            Self::Flame => Effect::Flame(p),
            Self::MachineGun => Effect::GunFire(p),
            Self::Rifle => Effect::GunFire(p),
            Self::Shells => Effect::ShellFire(p),
            _ => Effect::ShellFire(p), // TODO
        }
    }
}

/*impl WeaponType {
    pub fn damage_factor(&self, armor: &ArmorType, in_water: bool) -> Option<f32> {
        if !in_water && *self == Self::Torpedo {
            return None;
        }
        match (self, armor) {
            (_, ArmorType::Unknown) => Some(0.1),
            (Self::MachineGun, ArmorType::Infantry) => Some(1.00),
            (Self::MachineGun, ArmorType::Light) => Some(0.15),
            (Self::MachineGun, ArmorType::Heavy) => Some(0.05),
            (Self::MachineGun, ArmorType::Heli) => Some(0.20),
            (Self::MachineGun, ArmorType::Plane) => None,
            (Self::MachineGun, ArmorType::Submarine) => if !in_water { Some(0.15) } else { None },
            (Self::MachineGun, ArmorType::Structure) => Some(0.20),

            (Self::Rifle, ArmorType::Infantry) => Some(1.20),
            (Self::Rifle, ArmorType::Light) => Some(0.25),
            (Self::Rifle, ArmorType::Heavy) => Some(0.15),
            (Self::Rifle, ArmorType::Heli) => Some(0.40),
            (Self::Rifle, ArmorType::Plane) => Some(0.15),
            (Self::Rifle, ArmorType::Submarine) => if !in_water { Some(0.15) } else { None },
            (Self::Rifle, ArmorType::Structure) => Some(0.10),

            (Self::Shells, ArmorType::Infantry) => Some(0.90),
            (Self::Shells, ArmorType::Light) => Some(1.10),
            (Self::Shells, ArmorType::Heavy) => Some(0.70),
            (Self::Shells, ArmorType::Heli) => None,
            (Self::Shells, ArmorType::Plane) => None,
            (Self::Shells, ArmorType::Submarine) => if !in_water { Some(1.10) } else { None },
            (Self::Shells, ArmorType::Structure) => Some(1.00),

            (Self::Bombs, ArmorType::Infantry) => Some(1.10),
            (Self::Bombs, ArmorType::Light) => Some(1.10),
            (Self::Bombs, ArmorType::Heavy) => Some(0.9),
            (Self::Bombs, ArmorType::Heli) => None,
            (Self::Bombs, ArmorType::Plane) => None,
            (Self::Bombs, ArmorType::Submarine) => if !in_water { Some(1.10) } else { None },
            (Self::Bombs, ArmorType::Structure) => Some(1.00),

            (Self::AntiAir, ArmorType::Heli) => Some(1.50),
            (Self::AntiAir, ArmorType::Plane) => Some(1.20),
            (Self::AntiAir, _) => None,

            (Self::Flame, ArmorType::Infantry) => Some(1.20),
            (Self::Flame, ArmorType::Light) => Some(0.35),
            (Self::Flame, ArmorType::Heavy) => Some(0.10),
            (Self::Flame, ArmorType::Heli) => Some(0.50),
            (Self::Flame, ArmorType::Plane) => None,
            (Self::Flame, ArmorType::Submarine) => if !in_water { Some(0.35) } else { None },
            (Self::Flame, ArmorType::Structure) => Some(0.05),

            (Self::Rocket, ArmorType::Infantry) => Some(0.70),
            (Self::Rocket, ArmorType::Light) => Some(0.70),
            (Self::Rocket, ArmorType::Heavy) => Some(1.20),
            (Self::Rocket, ArmorType::Heli) => Some(1.10),
            (Self::Rocket, ArmorType::Plane) => None,
            (Self::Rocket, ArmorType::Submarine) => if !in_water { Some(0.70) } else { None },
            (Self::Rocket, ArmorType::Structure) => Some(1.20),
            // in_water is checked at the top
            (Self::Torpedo, ArmorType::Infantry) => Some(0.90),
            (Self::Torpedo, ArmorType::Light) => Some(1.10),
            (Self::Torpedo, ArmorType::Heavy) => Some(0.70),
            (Self::Torpedo, ArmorType::Heli) => None,
            (Self::Torpedo, ArmorType::Plane) => None,
            (Self::Torpedo, ArmorType::Submarine) => Some(1.10),
            (Self::Torpedo, ArmorType::Structure) => Some(1.00),

            (Self::SurfaceMissiles, ArmorType::Infantry) => Some(1.20),
            (Self::SurfaceMissiles, ArmorType::Light) => Some(1.20),
            (Self::SurfaceMissiles, ArmorType::Heavy) => Some(1.00),
            (Self::SurfaceMissiles, ArmorType::Heli) => None,
            (Self::SurfaceMissiles, ArmorType::Plane) => None,
            (Self::SurfaceMissiles, ArmorType::Submarine) => if !in_water { Some(1.20) } else { None },
            (Self::SurfaceMissiles, ArmorType::Structure) => Some(0.80),

            (Self::AirMissiles, ArmorType::Heli) => Some(1.20),
            (Self::AirMissiles, ArmorType::Plane) => Some(1.00),
            (Self::AirMissiles, _) => None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ArmorType {
	Infantry,
	Light,
	Heavy,
	Heli,
	Plane,
    Submarine,
	Structure,
    Unknown, // units half-hidden in light fog
}*/

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
pub enum Displacement {
    #[default]
    None,
    InsteadOfAttack,
    BeforeAttack,
    BetweenAttacks,
    // AfterCounter makes no sense to me
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
pub enum AttackTargeting {
    #[default]
    Enemy,
    Friendly,
    Owned,
    All,
    // special case for King-Castling
    // same owner + both unmoved
    OwnedBothUnmoved,
}
