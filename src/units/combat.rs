use std::collections::{HashSet, HashMap};
use std::str::FromStr;
use num_rational::Rational32;
use zipper_derive::Zippable;
use zipper::Exportable;

use crate::config::ConfigParseError;
use crate::game::events::Effect;
use crate::game::game::Game;
use crate::game::fog::FogIntensity;
use crate::game::event_handler::EventHandler;
use crate::map::point::Point;
use crate::config::environment::Environment;
use crate::map::direction::Direction;
use crate::map::map::{Map, NeighborMode};
use crate::map::wrapping_map::{OrientedPoint, Distortion};

use super::attributes::{AttributeKey, ActionStatus};
use super::movement::{PathStep, get_diagonal_neighbor};
use super::{unit::Unit, movement::Path};


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttackType {
    None,
    Adjacent,
    Ranged(u8, u8),
    // can get blocked by units that stand in the way
    // TODO: ensure that min and max are >0 in config
    Straight(u8, u8),
    Triangle(u8, u8),
}

impl FromStr for AttackType {
    type Err = ConfigParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut it = s.split(&['(', ',', '-', ')'])
        .map(str::trim);
        Ok(match it.next().unwrap() {
            "None" => Self::None,
            "Adjacent" => Self::Adjacent,
            s @ "Ranged" | s @ "Straight" | s @ "Triangle" => {
                let min = it.next().ok_or(ConfigParseError::NotEnoughValues(s.to_string()))?;
                let min = min.parse().map_err(|_| ConfigParseError::InvalidInteger(min.to_string()))?;
                let max = it.next().ok_or(ConfigParseError::NotEnoughValues(s.to_string()))?;
                let max: u8 = max.parse().map_err(|_| ConfigParseError::InvalidInteger(max.to_string()))?;
                if min > 50 || max > 50 || min + max > 50 {
                    return Err(ConfigParseError::NumberTooBig(s.to_string()));
                }
                match s {
                    "Ranged" => Self::Ranged(min, min + max),
                    "Straight" => Self::Straight(min, min + max),
                    "Triangle" => Self::Triangle(min, min + max),
                    _ => panic!("impossible AttackType error")
                }
            }
            invalid => return Err(ConfigParseError::UnknownEnumMember(invalid.to_string())),
        })
    }
}

/*impl AttackType {
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
                    let mut position = position;
                    let mut distortion = Distortion::neutral();
                    for i in 0..*max_range {
                        if let Some((point, disto)) = map.get_neighbor(position, distortion.update_direction(d)) {
                            if i + 1 >= *min_range {
                                result.insert(point);
                            } else if map.get_unit(point).is_some() {
                                break;
                            }
                            position = point;
                            distortion += disto;
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
}*/

fn attack_area_cannon<D: Direction, F: FnMut(Point, D, usize)>(map: &Map<D>, point: Point, dir: D, min_range: usize, max_range: usize, mut callback: F) {
    if D::is_hex() {
        let mut old_front = HashMap::new();
        let mut front = HashMap::new();
        front.insert((point, Distortion::neutral()), true);
        for i in 0..(max_range * 2 - 1) {
            let older_front = old_front;
            old_front = front;
            front = HashMap::new();
            for ((point, distortion), _) in older_front {
                let dir = distortion.update_direction(dir);
                if i >= min_range {
                    callback(point, dir, i);
                }
                if let Some((point, distortion)) = map.get_neighbor(point, dir) {
                    front.insert((point, distortion), true);
                }
            }
            // in order to not spread too much, only spread if
            //      - previously moved straight forward
            //      - current point was spread to from both sides
            for ((point, distortion), _) in old_front.iter()
            .filter(|(_, may_spread)| **may_spread) {
                for clockwise in [true, false] {
                    if let Some((p, disto)) = map.get_neighbor(*point, distortion.update_direction(dir.rotate(clockwise))) {
                        let key = (p, *distortion + disto);
                        front.insert(key.clone(), front.contains_key(&key));
                    }
                }
            }
        }
        for (point, distortion) in old_front.keys() {
            callback(*point, distortion.update_direction(dir), max_range * 2 - 1);
        }
        for (point, distortion) in front.keys() {
            callback(*point, distortion.update_direction(dir), max_range * 2);
        }
    } else {
        let mut front = HashSet::new();
        front.insert((point, Distortion::neutral()));
        for i in 0..max_range {
            let old_front = front;
            front = HashSet::new();
            for (point, distortion) in old_front {
                let d = distortion.update_direction(dir);
                if i * 2 >= min_range {
                    callback(point, d, i * 2);
                }
                if let Some((point, disto)) = map.get_neighbor(point, d) {
                    front.insert((point, distortion + disto));
                }
                if let Some((point, disto)) = get_diagonal_neighbor(map, point, d) {
                    front.insert((point, distortion + disto));
                }
                if let Some((point, disto)) = get_diagonal_neighbor(map, point, d.rotate(true)) {
                    front.insert((point, distortion + disto));
                }
            }
        }
        for (point, distortion) in front {
            callback(point, distortion.update_direction(dir), max_range * 2);
        }
    }
}


#[derive(Debug, Clone, PartialEq, Eq, Hash, Zippable)]
#[zippable(bits = 2, support_ref = Environment)]
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
    pub fn find(attacker: &Unit<D>, game: &Game<D>, pos: Point, target: Option<Point>, get_fog: impl Fn(Point) -> FogIntensity) -> HashSet<Self> {
        let valid_target: Box<dyn Fn(Point) -> bool> = if let Some(target) = target {
            let defender = match game.get_map().get_unit(target)
            .and_then(|u| u.fog_replacement(game, target, get_fog(target))) {
                None => return HashSet::new(),
                Some(defender) => defender,
            };
            if !attacker.could_attack(&defender, false) {
                return HashSet::new();
            }
            Box::new(move |t| t == target)
        } else {
            Box::new(|target| {
                match game.get_map().get_unit(target)
                .and_then(|u| u.fog_replacement(game, target, get_fog(target))) {
                    None => false,
                    Some(defender) => attacker.could_attack(&defender, false),
                }
            })
        };
        // TODO: check if target is protected by terrain (e.g. tank can only attack stranded Submarines)
        let mut result = HashSet::new();
        //let splash_damage = attacker.get_splash_damage();
        match attacker.attack_pattern() {
            AttackType::None => (),
            AttackType::Straight(min, max) => {
                for d in D::list() {
                    if Self::straight_splash(attacker, game, pos, d, min as usize, max as usize, &get_fog).iter()
                    .any(|(dp, _)| valid_target(dp.point)) {
                        result.insert(Self::Direction(d));
                    }
                }
            }
            AttackType::Adjacent => {
                for d in D::list() {
                    if let Some(dp) = game.get_map().get_neighbor(pos, d) {
                        if valid_target(dp.0) {
                            result.insert(Self::Direction(d));
                        }
                    }
                }
            }
            AttackType::Ranged(min_range, max_range) => {
                let range_bonus = game.get_map().get_terrain(pos).unwrap().range_bonus(attacker);
                let min_range = min_range + range_bonus;
                let max_range = max_range + range_bonus;
                // each point in a layer is probably in it 2 times
                let mut layers = game.get_map().range_in_layers(pos, max_range as usize);
                for _ in min_range-1..max_range {
                    for p in layers.pop().unwrap() {
                        if valid_target(p) {
                            result.insert(Self::Point(p));
                        }
                    }
                }
            }
            AttackType::Triangle(min, max) => {
                if attacker.has_attribute(AttributeKey::Direction) {
                    let direction = attacker.get_direction();
                    if let Some((point, distortion)) = game.get_map().get_neighbor(pos, direction) {
                        let direction = distortion.update_direction(direction);
                        attack_area_cannon(game.get_map(), point, direction, min as usize, max as usize, |point, d, _| {
                            if valid_target(point) {
                                // TODO: consider splash damage
                                result.insert(Self::DirectedPoint(point, d));
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
                .and_then(|u| u.fog_replacement(game, dp.point, get_fog(dp.point)))
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
            .and_then(|u| u.fog_replacement(game, dp.point, get_fog(dp.point))) {
                Some(u) => u,
                _ => return false,
            };
            if !attacker.could_attack(&defender, false) {
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
                    .and_then(|u| u.fog_replacement(game, dp.point, get_fog(dp.point)))
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

    fn adjacent_splash(attacker: &Unit<D>, game: &Game<D>, pos: Point, dir: D) -> Vec<(Point, Option<D>, Rational32)> {
        let splash_damage = attacker.get_splash_damage();
        let mut result = Vec::new();
        if let Some((pos, distortion)) = game.get_map().get_neighbor(pos, dir) {
            result.push((pos, Some(distortion.update_direction(dir)), splash_damage[0]));
        }
        let mut clockwise = dir;
        let mut counter_clockwise = dir;
        for i in 1..splash_damage.len() {
            clockwise = clockwise.rotate(true);
            counter_clockwise = counter_clockwise.rotate(false);
            if let Some((pos, distortion)) = game.get_map().get_neighbor(pos, clockwise) {
                result.push((pos, Some(distortion.update_direction(clockwise)), splash_damage[i]));
            }
            if let Some((pos, distortion)) = game.get_map().get_neighbor(pos, counter_clockwise) {
                result.push((pos, Some(distortion.update_direction(counter_clockwise)), splash_damage[i]));
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

    fn triangle_splash(attacker: &Unit<D>, game: &Game<D>, pos: Point, dir: D) -> Vec<(Point, Option<D>, Rational32)> {
        let splash_damage = attacker.get_splash_damage();
        let mut result = Vec::new();
        attack_area_cannon(game.get_map(), pos, dir, 0, (splash_damage.len() + 1) / 2, |pos, dir, splash_index| {
            if splash_index < splash_damage.len() {
                result.push((pos, Some(dir), splash_damage[splash_index]));
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
                Self::adjacent_splash(attacker, game, pos, *dir)
            }
            (AttackType::Ranged(_min, _max), Self::Point(p)) => {
                Self::ranged_splash(attacker, game, *p).into_iter()
                .map(|(p, ratio)| (p, None, ratio))
                .collect()
            }
            (AttackType::Triangle(_min, _max), Self::DirectedPoint(pos, dir)) => {
                Self::triangle_splash(attacker, game, *pos, *dir)
            }
            _ => panic!("AttackPattern is incompatible with AttackVector"),
        }
    }

    /**
     * returns the new position of the attacker or None if the attacker died
     */
    pub fn execute(&self, handler: &mut EventHandler<D>, attacker_pos: Point, path: Option<&Path<D>>, exhaust_after_attacking: bool, execute_scripts: bool, charge_powers: bool) -> Option<(Point, Option<usize>)> {
        let (attacker_id, _) = handler.observe_unit(attacker_pos, None);
        let attacker = handler.get_map().get_unit(attacker_pos).cloned().unwrap();
        let mut after_battle_displacements = Vec::new();
        if attacker.displacement() == Displacement::AfterCounter {
            after_battle_displacements.push((self.clone(), attacker.clone(), attacker_pos));
        }
        let (
            mut hero_charge,
            counter_attackers,
            mut defenders,
        ) = self.execute_attack(handler, &attacker, attacker_pos, path, exhaust_after_attacking, execute_scripts);
        // counter attack
        if path.is_some() {
            for unit_id in counter_attackers {
                let attacker_pos = match handler.get_observed_unit(attacker_id) {
                    Some((p, _, _)) => *p,
                    _ => break,
                };
                let unit_pos = match handler.get_observed_unit(unit_id) {
                    Some((p, None, _)) => *p,
                    _ => continue,
                };
                let unit = handler.get_map().get_unit(unit_pos).cloned().expect(&format!("didn't find counter attacker at {unit_pos:?}"));
                if !handler.get_game().can_see_unit_at(unit.get_team(), attacker_pos, &attacker, true)
                || !unit.could_attack(&attacker, false)
                || attacker.get_team() == unit.get_team() {
                    continue;
                }
                let mut attack_vectors: Vec<_> = AttackVector::find(&unit, handler.get_game(), unit_pos, Some(attacker_pos), |_| FogIntensity::TrueSight)
                .into_iter().collect();
                let mut attack_vector = attack_vectors.pop();
                if let AttackVector::Direction(d) = self {
                    let inverse = AttackVector::Direction(d.opposite_direction());
                    if attack_vectors.contains(&inverse) {
                        attack_vector = Some(inverse);
                    }
                }
                if let Some(attack_vector) = attack_vector {
                    if unit.displacement() == Displacement::AfterCounter {
                        after_battle_displacements.push((attack_vector.clone(), unit.clone(), unit_pos));
                    }
                    let (_, _, d2) = attack_vector.execute_attack(handler, &unit, unit_pos, None, false, execute_scripts);
                    defenders.extend(d2.into_iter());
                }
            }
        }
        for (attack_vector, attacker, attacker_pos) in after_battle_displacements {
            let defenders = attack_vector.get_splash(&attacker, handler.get_game(), attacker_pos, |_| FogIntensity::TrueSight);
            let defenders = filter_attack_targets(handler.get_game(), &attacker, defenders);
            displace(handler, &attacker, defenders);
        }
        if !charge_powers {
            hero_charge = 0;
            defenders = Vec::new();
        }
        after_attacking(handler, &attacker, attacker_pos, hero_charge, defenders);
        handler.get_observed_unit(attacker_id)
        .map(|(p, unload_index, _)| (*p, *unload_index))
    }

    // set path to None if this is a counter-attack
    fn execute_attack(&self, handler: &mut EventHandler<D>, attacker: &Unit<D>, attacker_pos: Point, path: Option<&Path<D>>, exhaust_after_attacking: bool, execute_scripts: bool) -> (u32, Vec<usize>, Vec<(i8, Unit<D>, u8)>) {
        let defenders = self.get_splash(attacker, handler.get_game(), attacker_pos, |_| FogIntensity::TrueSight);
        attack_targets(handler, attacker, attacker_pos, path, defenders, exhaust_after_attacking, execute_scripts)
    }
}

pub(crate) fn attack_targets<D: Direction>(handler: &mut EventHandler<D>, attacker: &Unit<D>, attacker_pos: Point, path: Option<&Path<D>>, targets: Vec<(Point, Option<D>, Rational32)>, exhaust_after_attacking: bool, execute_scripts: bool) -> (u32, Vec<usize>, Vec<(i8, Unit<D>, u8)>) {
    let (attacker_id, _) = handler.observe_unit(attacker_pos, None);
    let mut defenders = filter_attack_targets(handler.get_game(), attacker, targets);
    let mut hero_charge;
    let mut attacked_units;
    let mut killed_units;
    match attacker.displacement() {
        Displacement::None | Displacement::AfterCounter => {
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
                    if let Some((p, _)) = handler.get_map().get_neighbor(p, d) {
                        if handler.get_map().get_unit(p).is_some() {
                            collided.push((p, None, ratio));
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
            Some((p, None, _)) => handler.unit_status(*p, ActionStatus::Exhausted),
            Some((p, Some(index), _)) => handler.unit_status_boarded(*p, *index, ActionStatus::Exhausted),
            None => (),
        }
    }
    let mut counter_attackers = Vec::new();
    let mut defenders = Vec::new();
    for (defender_id, _, defender, damage) in &attacked_units {
        if attacker.get_team() != defender.get_team() && !counter_attackers.contains(defender_id) {
            counter_attackers.push(*defender_id);
        }
        defenders.push((attacker.get_owner_id(), defender.clone(), *damage));
    }
    if execute_scripts {
        for (defender_id, defender_pos, defender, damage) in &attacked_units {
            for script in attacker.get_attack_scripts(handler.get_game(), attacker_pos, defender, *defender_pos) {
                script.trigger(handler, handler.get_observed_unit_pos(attacker_id), attacker, *defender_pos, defender, handler.get_observed_unit_pos(*defender_id), *damage);
            }
        }
        for (defender_pos, defender) in killed_units {
            for script in attacker.get_kill_scripts(handler.get_game(), attacker_pos, &defender, defender_pos) {
                script.trigger(handler, handler.get_observed_unit_pos(attacker_id), attacker, defender_pos, &defender);
            }
        }
    }
    (hero_charge, counter_attackers, defenders)
}

fn filter_attack_targets<D: Direction>(game: &Game<D>, attacker: &Unit<D>, targets: Vec<(Point, Option<D>, Rational32)>) -> Vec<(Point, Option<D>, Rational32)> {
    targets.into_iter()
    .filter(|(p, _, _)| {
        if let Some(defender) = game.get_map().get_unit(*p) {
            attacker.could_attack(defender, true)
        } else {
            false
        }
    })
    .collect()
}

fn deal_damage<D: Direction>(handler: &mut EventHandler<D>, attacker: &Unit<D>, attacker_pos: Point, path: Option<&Path<D>>, targets: Vec<(Point, Option<D>, Rational32)>) -> (Vec<(Point, Option<D>, Rational32)>, u32, Vec<(usize, Point, Unit<D>, u8)>, Vec<(Point, Unit<D>)>) {
    let mut raw_damage = HashMap::new();
    let mut hero_charge = 0;
    let mut attack_script_targets = Vec::new();
    for (defender_pos, dir, factor) in targets.iter().cloned() {
        let defender = handler.get_map().get_unit(defender_pos).unwrap();
        let hp = defender.get_hp();
        if hp == 0 {
            continue;
        }
        let damage = calculate_attack_damage(handler.get_game(), attacker, attacker_pos, path, defender, defender_pos, factor);
        if damage == 0 {
            continue;
        }
        let defender = defender.clone();
        handler.effect_weapon(defender_pos, attacker.weapon());
        if damage > 0 {
            if attacker.get_team() != defender.get_team() {
                hero_charge += 1;
            }
            if !raw_damage.contains_key(&defender_pos) {
                attack_script_targets.push(defender_pos);
            }
            let previous_damage = raw_damage.remove(&defender_pos).unwrap_or(0);
            raw_damage.insert(defender_pos, previous_damage + damage as u16);
        } else {
            handler.unit_heal(defender_pos.clone(), (-damage) as u8);
        }
    }
    let attack_script_targets: Vec<(usize, Point, Unit<D>, u8)> = attack_script_targets.into_iter()
    .map(|p| {
        let raw_damage = *raw_damage.get(&p).unwrap();
        let unit = handler.get_map().get_unit(p).unwrap().clone();
        let hp = unit.get_hp() as u16;
        (handler.observe_unit(p, None).0, p, unit, hp.min(raw_damage) as u8)
    })
    //.filter(|(_, _, u, _)| u.get_team() != attacker.get_team())
    .collect();
    /*let defenders: Vec<(Unit<D>, u8)> = attack_script_targets.iter()
    .map(|(_, unit, damage)| {
        (unit.clone(), *damage)
    })
    .collect();*/
    handler.unit_mass_damage(&raw_damage);
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

fn calculate_attack_damage<D: Direction>(game: &Game<D>, attacker: &Unit<D>, attacker_pos: Point, path: Option<&Path<D>>, defender: &Unit<D>, defender_pos: Point, factor: Rational32) -> i32 {
    let is_counter = path.is_none();
    let defensive_terrain = game.get_map().get_terrain(defender_pos).unwrap();
    let terrain_defense = Rational32::from_integer(1) + defensive_terrain.defense_bonus(defender);
    /*for t in game.get_map().get_neighbors(defender_pos, crate::map::map::NeighborMode::Direct).into_iter().map(|p| game.get_map().get_terrain(p.point).unwrap()) {
        terrain_defense += t.adjacent_defense_bonus(defender);
    }*/
    let mut defense_bonus = Rational32::from_integer(1);
    defense_bonus += defender.get_commander(game).defense_bonus(defender, game, defender_pos, is_counter, attacker, attacker_pos);
    for (p, hero_unit, hero) in game.get_map().hero_influence_at(defender_pos, defender.get_owner_id()) {
        defense_bonus += game.environment().config.aura_defense_bonus(game, defender, defender_pos, &hero_unit, p, attacker, attacker_pos, hero.typ(), hero.is_power_active(), is_counter);
    }
    let defense_bonus = defense_bonus; // to make sure it's not updated in the for-loop on accident
    let attack = Rational32::from_integer(attacker.base_damage(defender.typ()).unwrap() as i32);
    /*if let Some(path) = path {
        attack *= attacker.attack_factor_from_path(game, path);
    } else {
        attack *= attacker.attack_factor_from_counter(game);
    }*/
    let mut attack_bonus = Rational32::from_integer(1);
    attack_bonus += attacker.get_commander(game).attack_bonus(attacker, game, attacker_pos, is_counter, defender, defender_pos);
    for (p, hero_unit, hero) in game.get_map().hero_influence_at(attacker_pos, attacker.get_owner_id()) {
        attack_bonus += game.environment().config.aura_attack_bonus(game, attacker, attacker_pos, &hero_unit, p, defender, defender_pos, hero.typ(), hero.is_power_active(), is_counter);
    }
    (Rational32::from_integer(attacker.get_hp() as i32) / 100 * attack * attack_bonus * factor / defense_bonus / terrain_defense)
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
            let mut line = handler.get_map().get_line(*pos, d, distance.abs() as usize + 1, NeighborMode::FollowPipes);
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
        for (p, _, _) in handler.get_map().hero_influence_at(attacker_pos, attacker.get_owner_id()) {
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

crate::listable_enum! {
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub enum WeaponType {
        MachineGun,
        Shells,
        AntiAir,
        Flame,
        Rocket,
        Torpedo,
        Rifle,
        Bombs,
        Bonk,
        // immobile ranged
        SurfaceMissiles,
        AirMissiles,
    }
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

crate::listable_enum! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Displacement {
        None,
        InsteadOfAttack,
        BeforeAttack,
        BetweenAttacks,
        AfterCounter,
    }
}

crate::listable_enum! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum AttackTargeting {
        Enemy,
        Friendly,
        Owned,
        All,
        // special case for King-Castling
        // same owner + both unmoved
        OwnedBothUnmoved,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use interfaces::game_interface::GameInterface;
    use interfaces::map_interface::MapInterface;

    use crate::config::config::Config;
    use crate::config::environment::Environment;
    use crate::game::commands::Command;
    use crate::game::fog::FogMode;
    use crate::game::settings::{GameSettings, PlayerSettings};
    use crate::map::direction::Direction4;
    use crate::map::map::Map;
    use crate::map::point::{Point, Position};
    use crate::map::point_map::PointMap;
    use crate::map::wrapping_map::*;
    use crate::terrain::TerrainType;
    use crate::units::commands::{UnitCommand, UnitAction};
    use crate::units::movement::Path;
    use crate::units::unit_types::UnitType;

    use super::*;


    #[test]
    fn terrain_defense() {
        let map = PointMap::new(4, 4, false);
        let environment = Environment {
            config: Arc::new(Config::test_config()),
            map_size: map.size(),
            settings: None,
        };
        let wmap: WrappingMap<Direction4> = WrappingMapBuilder::new(map, Vec::new()).build().unwrap();
        let mut map = Map::new2(wmap, &environment);
        map.set_terrain(Point::new(0, 0), TerrainType::Street.instance(&environment).build_with_defaults());
        map.set_terrain(Point::new(1, 0), TerrainType::Grass.instance(&environment).build_with_defaults());
        map.set_terrain(Point::new(2, 0), TerrainType::Forest.instance(&environment).build_with_defaults());
        map.set_terrain(Point::new(3, 0), TerrainType::Mountain.instance(&environment).build_with_defaults());
        map.set_unit(Point::new(0, 0), Some(UnitType::Bazooka.instance(&environment).set_owner_id(1).build_with_defaults()));
        map.set_unit(Point::new(1, 0), Some(UnitType::Bazooka.instance(&environment).set_owner_id(1).build_with_defaults()));
        map.set_unit(Point::new(2, 0), Some(UnitType::Bazooka.instance(&environment).set_owner_id(1).build_with_defaults()));
        map.set_unit(Point::new(3, 0), Some(UnitType::Bazooka.instance(&environment).set_owner_id(1).build_with_defaults()));
        map.set_unit(Point::new(0, 1), Some(UnitType::Bazooka.instance(&environment).set_owner_id(0).build_with_defaults()));
        map.set_unit(Point::new(1, 1), Some(UnitType::Bazooka.instance(&environment).set_owner_id(0).build_with_defaults()));
        map.set_unit(Point::new(2, 1), Some(UnitType::Bazooka.instance(&environment).set_owner_id(0).build_with_defaults()));
        map.set_unit(Point::new(3, 1), Some(UnitType::Bazooka.instance(&environment).set_owner_id(0).build_with_defaults()));
        let (mut game, _) = map.game_server(&GameSettings {
            name: "terrain_defense".to_string(),
            fog_mode: FogMode::Constant(crate::game::fog::FogSetting::None),
            players: vec![
                PlayerSettings::new(&environment.config, 0),
                PlayerSettings::new(&environment.config, 1),
            ],
        }, || 0.);
        for x in 0..4 {
            game.handle_command(Command::UnitCommand(UnitCommand {
                unload_index: None,
                path: Path::new(Point::new(x, 1)),
                action: UnitAction::Attack(AttackVector::Direction(Direction4::D90)),
            }), || 0.).unwrap();
        }
        let base_damage = 100. - game.get_map().get_unit(Point::new(0, 0)).unwrap().get_hp() as f32;
        assert_eq!(100 - (base_damage / 1.1).ceil() as u8, game.get_map().get_unit(Point::new(1, 0)).unwrap().get_hp());
        assert_eq!(100 - (base_damage / 1.2).ceil() as u8, game.get_map().get_unit(Point::new(2, 0)).unwrap().get_hp());
        assert_eq!(100 - (base_damage / 1.3).ceil() as u8, game.get_map().get_unit(Point::new(3, 0)).unwrap().get_hp());
    }

    #[test]
    fn displacement() {
        let map = PointMap::new(5, 4, false);
        let environment = Environment {
            config: Arc::new(Config::test_config()),
            map_size: map.size(),
            settings: None,
        };
        let wmap: WrappingMap<Direction4> = WrappingMapBuilder::new(map, Vec::new()).build().unwrap();
        let mut map = Map::new2(wmap, &environment);
        map.set_unit(Point::new(1, 0), Some(UnitType::Magnet.instance(&environment).set_owner_id(0).build_with_defaults()));
        map.set_unit(Point::new(3, 0), Some(UnitType::Sniper.instance(&environment).set_owner_id(0).build_with_defaults()));
        map.set_unit(Point::new(1, 1), Some(UnitType::Custom(0).instance(&environment).set_owner_id(0).build_with_defaults()));
        map.set_unit(Point::new(2, 1), Some(UnitType::Custom(0).instance(&environment).set_owner_id(1).build_with_defaults()));
        map.set_unit(Point::new(3, 1), Some(UnitType::Custom(0).instance(&environment).set_owner_id(1).build_with_defaults()));
        let (mut game, _) = map.game_server(&GameSettings {
            name: "displacement".to_string(),
            fog_mode: FogMode::Constant(crate::game::fog::FogSetting::None),
            players: vec![
                PlayerSettings::new(&environment.config, 0),
                PlayerSettings::new(&environment.config, 1),
            ],
        }, || 0.);
        game.handle_command(Command::UnitCommand(UnitCommand {
            unload_index: None,
            path: Path::new(Point::new(1, 0)),
            action: UnitAction::Attack(AttackVector::Direction(Direction4::D0)),
        }), || 0.).unwrap();
        assert_eq!(100, game.get_map().get_unit(Point::new(1, 0)).unwrap().get_hp());
        assert_eq!(100, game.get_map().get_unit(Point::new(2, 0)).unwrap().get_hp());
        assert_eq!(None, game.get_map().get_unit(Point::new(3, 0)));
        game.handle_command(Command::UnitCommand(UnitCommand {
            unload_index: None,
            path: Path::new(Point::new(1, 1)),
            action: UnitAction::Attack(AttackVector::Direction(Direction4::D0)),
        }), || 0.).unwrap();
        assert_eq!(70, game.get_map().get_unit(Point::new(0, 1)).unwrap().get_hp());
        for x in 1..=3 {
            assert_eq!(None, game.get_map().get_unit(Point::new(x, 1)));
        }
        assert_eq!(94, game.get_map().get_unit(Point::new(4, 1)).unwrap().get_hp());
    }
}
