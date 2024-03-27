use std::collections::{HashSet, HashMap};
use num_rational::Rational32;
use zipper_derive::Zippable;
use zipper::Exportable;

use crate::config::parse::{parse_tuple2, string_base, FromConfig};
use crate::config::ConfigParseError;
use crate::game::events::Effect;
use crate::game::game::Game;
use crate::game::event_handler::EventHandler;
use crate::game::game_view::GameView;
use crate::map::map_view::MapView;
use crate::map::point::Point;
use crate::config::environment::Environment;
use crate::map::direction::Direction;
use crate::map::map::NeighborMode;
use crate::map::wrapping_map::{OrientedPoint, Distortion};

use super::attributes::{AttributeKey, ActionStatus};
use super::hero::Hero;
use super::movement::{get_diagonal_neighbor, PathStep, TBallast, TemporaryBallast};
use super::{unit::Unit, movement::Path};

crate::listable_enum! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub(crate) enum AttackTypeKey {
        None,
        Adjacent,
        Ranged,
        Straight,
        Triangle,
    }
}

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

impl FromConfig for AttackType {
    fn from_conf(s: &str) -> Result<(Self, &str), ConfigParseError> {
        let (base, mut remainder) = string_base(s);
        Ok((match base {
            "None" => Self::None,
            "Adjacent" => Self::Adjacent,
            s @ "Ranged" | s @ "Straight" | s @ "Triangle" => {
                let (min, additional, r) = parse_tuple2::<u8, u8>(remainder)?;
                remainder = r;
                if min > 50 || additional > 50 || min + additional > 50 {
                    return Err(ConfigParseError::NumberTooBig(s.to_string()));
                }
                match s {
                    "Ranged" => Self::Ranged(min, min + additional),
                    "Straight" => Self::Straight(min, min + additional),
                    "Triangle" => Self::Triangle(min, min + additional),
                    _ => panic!("impossible AttackType error")
                }
            }
            invalid => return Err(ConfigParseError::UnknownEnumMember(invalid.to_string())),
        }, remainder))
    }
}

impl AttackType {
    pub(crate) fn key(&self) -> AttackTypeKey {
        match self {
            Self::None => AttackTypeKey::None,
            Self::Adjacent => AttackTypeKey::Adjacent,
            Self::Ranged(_, _) => AttackTypeKey::Ranged,
            Self::Straight(_, _) => AttackTypeKey::Straight,
            Self::Triangle(_, _) => AttackTypeKey::Triangle,
        }
    }
}

fn attack_area_cannon<D: Direction, F: FnMut(Point, D, usize)>(map: &impl MapView<D>, point: Point, dir: D, min_range: usize, max_range: usize, mut callback: F) {
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
    pub fn find(attacker: &Unit<D>, game: &impl GameView<D>, pos: Point, target: Option<Point>, transporter: Option<(&Unit<D>, Point)>, temporary_ballast: &[TBallast<D>]) -> HashSet<Self> {
        let heroes: Vec<(Unit<D>, Hero, Point, Option<usize>)> = Hero::hero_influence_at(game, pos, attacker.get_owner_id());
        let splash_damage = attacker.get_splash_damage(game, pos, &heroes, temporary_ballast);
        let displacement_distance = attacker.displacement_distance(game, pos, transporter, &heroes, temporary_ballast);
        let team = attacker.get_team();
        let valid_target: Box<dyn Fn(Point, usize, Option<D>) -> bool> = if let Some(target) = target {
            let defender = match game.get_visible_unit(team, target) {
                None => return HashSet::new(),
                Some(defender) => defender,
            };
            if !attacker.could_attack(&defender, false) {
                return HashSet::new();
            }
            Box::new(move |p, splash_index, displacement_direction| {
                if p != target {
                    return false;
                }
                if splash_damage[splash_index] != Rational32::from_integer(0) && attacker.base_damage(defender.typ()) != Some(0) {
                    true
                } else if attacker.displacement() == Displacement::None {
                    false
                } else if let Some(displacement_direction) = displacement_direction {
                    // check if displacement would succeed
                    let target_points = if displacement_distance < 0 {
                        game.get_line(target, displacement_direction.opposite_direction(), (-displacement_distance) as usize + 1, NeighborMode::FollowPipes)
                    } else {
                        game.get_line(target, displacement_direction, displacement_distance as usize + splash_damage.len() - splash_index, NeighborMode::FollowPipes)
                    };
                    target_points
                    .into_iter()
                    .skip(1)
                    .any(|dp| {
                        // not blocked by the attacker or any other unit
                        dp.point != pos && game.get_unit(dp.point).is_none()
                    })
                } else {
                    false
                }
            })
        } else {
            Box::new(|target, splash_index, displacement_direction| {
                let defender = match game.get_visible_unit(team, target) {
                    Some(u) => u,
                    _ => return false,
                };
                if !attacker.could_attack(&defender, false) {
                    return false;
                }
                if splash_damage[splash_index] != Rational32::from_integer(0) && attacker.base_damage(defender.typ()) != Some(0) {
                    true
                } else if attacker.displacement() == Displacement::None {
                    false
                } else if let Some(displacement_direction) = displacement_direction {
                    // check if displacement would succeed
                    let target_points = if displacement_distance < 0 {
                        game.get_line(target, displacement_direction.opposite_direction(), (-displacement_distance) as usize + 1, NeighborMode::FollowPipes)
                    } else {
                        game.get_line(target, displacement_direction, displacement_distance as usize + splash_damage.len() - splash_index, NeighborMode::FollowPipes)
                    };
                    target_points
                    .into_iter()
                    .skip(1)
                    .any(|dp| {
                        // not blocked by the attacker or any other unit
                        dp.point != pos && game.get_unit(dp.point).is_none()
                    })
                } else {
                    false
                }
            })
        };
        Self::_search(attacker, game, pos, transporter, &heroes, temporary_ballast, valid_target)
    }

    // doesn't check if there's a unit at the target position that can be attacked
    pub fn search(attacker: &Unit<D>, game: &impl GameView<D>, pos: Point, target: Option<Point>, transporter: Option<(&Unit<D>, Point)>, temporary_ballast: &[TBallast<D>]) -> HashSet<Self> {
        let heroes: Vec<(Unit<D>, Hero, Point, Option<usize>)> = Hero::hero_influence_at(game, pos, attacker.get_owner_id());
        let splash_damage = attacker.get_splash_damage(game, pos, &heroes, temporary_ballast);
        let displacement_distance = attacker.displacement_distance(game, pos, transporter, &heroes, temporary_ballast);
        Self::_search(attacker, game, pos, transporter, &heroes, temporary_ballast, |p, splash_index, dir| {
            (target == None || target == Some(p))
            && (splash_damage[splash_index] != Rational32::from_integer(0) || displacement_distance != 0 && dir.is_some())
        })
    }

    fn _search(attacker: &Unit<D>, game: &impl GameView<D>, pos: Point, transporter: Option<(&Unit<D>, Point)>, heroes: &[(Unit<D>, Hero, Point, Option<usize>)], temporary_ballast: &[TBallast<D>], valid_target: impl Fn(Point, usize, Option<D>) -> bool) -> HashSet<Self> {
        // TODO: check if target is protected by terrain (e.g. tank can only attack stranded Submarines)
        let mut result = HashSet::new();
        //let splash_damage = attacker.get_splash_damage();
        match attacker.attack_pattern() {
            AttackType::None => (),
            AttackType::Straight(min_range, max_range) => {
                let min_range = game.environment().config.unit_range(game, attacker, pos, transporter, heroes, temporary_ballast, true, min_range) as usize;
                let max_range = game.environment().config.unit_range(game, attacker, pos, transporter, heroes, temporary_ballast, false, max_range) as usize;
                for d in D::list() {
                    if Self::straight_splash(attacker, game, pos, d, min_range, max_range, heroes, temporary_ballast, &valid_target).iter()
                    .enumerate()
                    .any(|(i, (dp, _))| valid_target(dp.point, i, Some(dp.direction))) {
                        result.insert(Self::Direction(d));
                    }
                }
            }
            AttackType::Adjacent => {
                for d in D::list() {
                    if let Some((p, distortion)) = game.get_neighbor(pos, d) {
                        // TODO: ignores splash
                        if valid_target(p, 0, Some(distortion.update_direction(d))) {
                            result.insert(Self::Direction(d));
                        }
                    }
                }
            }
            AttackType::Ranged(min_range, max_range) => {
                let min_range = game.environment().config.unit_range(game, attacker, pos, transporter, &heroes, temporary_ballast, true, min_range) as usize;
                let max_range = game.environment().config.unit_range(game, attacker, pos, transporter, &heroes, temporary_ballast, false, max_range) as usize;
                // each point in a layer is probably in it 2 times
                let mut layers = game.range_in_layers(pos, max_range as usize);
                for _ in min_range-1..max_range {
                    for p in layers.pop().unwrap() {
                        // TODO: ignores splash
                        if valid_target(p, 0, None) {
                            result.insert(Self::Point(p));
                        }
                    }
                }
            }
            AttackType::Triangle(min_range, max_range) => {
                if attacker.has_attribute(AttributeKey::Direction) {
                    let direction = attacker.get_direction();
                    if let Some((point, distortion)) = game.get_neighbor(pos, direction) {
                        let min_range = game.environment().config.unit_range(game, attacker, pos, transporter, &heroes, temporary_ballast, true, min_range) as usize;
                        let max_range = game.environment().config.unit_range(game, attacker, pos, transporter, &heroes, temporary_ballast, false, max_range) as usize;
                        let direction = distortion.update_direction(direction);
                        attack_area_cannon(game, point, direction, min_range, max_range, |point, d, splash_index| {
                            if valid_target(point, splash_index, Some(d)) {
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

    fn straight_splash(attacker: &Unit<D>, game: &impl GameView<D>, pos: Point, dir: D, min: usize, max: usize, heroes: &[(Unit<D>, Hero, Point, Option<usize>)], temporary_ballast: &[TBallast<D>], valid_target: impl Fn(Point, usize, Option<D>) -> bool) -> Vec<(OrientedPoint<D>, Rational32)> {
        let splash_damage = attacker.get_splash_damage(game, pos, heroes, temporary_ballast);
        let points: Vec<_> = game.get_line(pos, dir, max + splash_damage.len(), NeighborMode::FollowPipes)
        .into_iter()
        .skip(1) // skip pos
        //.map(|dp| dp.point)
        .collect();
        let blocking_unit = points.iter().enumerate().position(|(i, dp)| {
            i < max && game.get_unit(dp.point).is_some()
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
            valid_target(dp.point, i, Some(dp.direction))
        }) {
            points.into_iter()
            .skip(blocking_unit.unwrap_or(max - 1))
            .zip(splash_damage.iter().cloned())
            .collect()
        } else {
            Vec::new()
        }
    }

    fn adjacent_splash(attacker: &Unit<D>, game: &impl GameView<D>, pos: Point, dir: D, heroes: &[(Unit<D>, Hero, Point, Option<usize>)], temporary_ballast: &[TBallast<D>]) -> Vec<(Point, Option<D>, Rational32)> {
        let splash_damage = attacker.get_splash_damage(game, pos, heroes, temporary_ballast);
        let mut result = Vec::new();
        if let Some((pos, distortion)) = game.get_neighbor(pos, dir) {
            result.push((pos, Some(distortion.update_direction(dir)), splash_damage[0]));
        }
        let mut clockwise = dir;
        let mut counter_clockwise = dir;
        for i in 1..splash_damage.len() {
            clockwise = clockwise.rotate(true);
            counter_clockwise = counter_clockwise.rotate(false);
            if let Some((pos, distortion)) = game.get_neighbor(pos, clockwise) {
                result.push((pos, Some(distortion.update_direction(clockwise)), splash_damage[i]));
            }
            if let Some((pos, distortion)) = game.get_neighbor(pos, counter_clockwise) {
                result.push((pos, Some(distortion.update_direction(counter_clockwise)), splash_damage[i]));
            }
        }
        result
    }

    fn ranged_splash(attacker: &Unit<D>, game: &impl GameView<D>, pos: Point, heroes: &[(Unit<D>, Hero, Point, Option<usize>)], temporary_ballast: &[TBallast<D>]) -> Vec<(Point, Rational32)> {
        let splash_damage = attacker.get_splash_damage(game, pos, heroes, temporary_ballast);
        let mut result = vec![(pos, splash_damage[0])];
        for (i, ring) in game.range_in_layers(pos, splash_damage.len() - 1).into_iter().enumerate() {
            for p in ring {
                result.push((p, splash_damage[i + 1]));
            }
        }
        result
    }

    fn triangle_splash(attacker: &Unit<D>, game: &impl GameView<D>, pos: Point, dir: D, heroes: &[(Unit<D>, Hero, Point, Option<usize>)], temporary_ballast: &[TBallast<D>]) -> Vec<(Point, Option<D>, Rational32)> {
        let splash_damage = attacker.get_splash_damage(game, pos, heroes, temporary_ballast);
        let mut result = Vec::new();
        attack_area_cannon(game, pos, dir, 0, (splash_damage.len() + 1) / 2, |pos, dir, splash_index| {
            if splash_index < splash_damage.len() {
                result.push((pos, Some(dir), splash_damage[splash_index]));
            }
        });
        result
    }

    pub fn get_splash(&self, attacker: &Unit<D>, game: &impl GameView<D>, pos: Point, heroes: &[(Unit<D>, Hero, Point, Option<usize>)], temporary_ballast: &[TBallast<D>]) -> Vec<(Point, Option<D>, Rational32)> {
        match (attacker.attack_pattern(), self) {
            (AttackType::None, _) => Vec::new(),
            (AttackType::Straight(min, max), Self::Direction(dir)) => {
                Self::straight_splash(attacker, game, pos, *dir, min as usize, max as usize, heroes, temporary_ballast, |_, _, _| true).into_iter()
                .map(|(dp, ratio)| (dp.point, Some(dp.direction), ratio))
                .collect()
            }
            (AttackType::Adjacent, Self::Direction(dir)) => {
                Self::adjacent_splash(attacker, game, pos, *dir, heroes, temporary_ballast)
            }
            (AttackType::Ranged(_min, _max), Self::Point(p)) => {
                Self::ranged_splash(attacker, game, *p, heroes, temporary_ballast).into_iter()
                .map(|(p, ratio)| (p, None, ratio))
                .collect()
            }
            (AttackType::Triangle(_min, _max), Self::DirectedPoint(pos, dir)) => {
                Self::triangle_splash(attacker, game, *pos, *dir, heroes, temporary_ballast)
            }
            _ => panic!("AttackPattern is incompatible with AttackVector"),
        }
    }

    /**
     * returns the new position of the attacker or None if the attacker died
     */
    pub fn execute(&self, handler: &mut EventHandler<D>, attacker_pos: Point, path: Option<(&Path<D>, Option<(&Unit<D>, Point)>, &[TBallast<D>])>, exhaust_after_attacking: bool, execute_scripts: bool, charge_powers: bool) -> Option<(Point, Option<usize>)> {
        let (attacker_id, _) = handler.observe_unit(attacker_pos, None);
        let attacker = handler.get_map().get_unit(attacker_pos).cloned().unwrap();
        let mut after_battle_displacements = Vec::new();
        if attacker.displacement() == Displacement::AfterCounter {
            after_battle_displacements.push((self.clone(), attacker.clone(), attacker_pos, path.clone()));
        }
        // unit already moved, so drag_along is None
        let attacker_heroes: Vec<_> = Hero::hero_influence_at(handler.get_game(), attacker_pos, attacker.get_owner_id());
        let environment = handler.environment().clone();
        let attacker_hero_ids = attacker_heroes.iter()
        .filter(|(_, hero, _, _)| hero.can_gain_charge(&environment))
        .map(|(_, _, p, unload_index)| handler.observe_unit(*p, *unload_index).0)
        .collect();
        let (
            mut hero_charge,
            counter_attackers,
            mut commander_charge,
        ) = self.execute_attack(handler, &attacker, attacker_pos, path, &attacker_heroes, exhaust_after_attacking, execute_scripts);
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
                let mut attack_vectors: Vec<_> = AttackVector::find(&unit, handler.get_game(), unit_pos, Some(attacker_pos), None, &[])
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
                        after_battle_displacements.push((attack_vector.clone(), unit.clone(), unit_pos, None));
                    }
                    // unit doesn't move, so drag_along is None
                    let heroes: Vec<_> = Hero::hero_influence_at(handler.get_game(), unit_pos, unit.get_owner_id());
                    let (_, _, d2) = attack_vector.execute_attack(handler, &unit, unit_pos, None, &heroes, false, execute_scripts);
                    commander_charge.extend(d2.into_iter());
                }
            }
        }
        for (attack_vector, attacker, attacker_pos, path) in after_battle_displacements {
            let heroes = Hero::hero_influence_at(handler.get_game(), attacker_pos, attacker.get_owner_id());
            let temporary_ballast = path.map(|(_, _, t)| t).unwrap_or(&[]);
            let defenders = attack_vector.get_splash(&attacker, handler.get_game(), attacker_pos, &heroes, temporary_ballast);
            let defenders = filter_attack_targets(handler.get_game(), &attacker, defenders);
            // unit doesn't move, so drag_along is None
            displace(handler, &attacker, attacker_pos, path, defenders, &heroes);
        }
        if !charge_powers {
            hero_charge = 0;
            commander_charge = Vec::new();
        }
        after_attacking(handler, attacker_id, hero_charge, attacker_hero_ids, commander_charge);
        handler.get_observed_unit(attacker_id)
        .map(|(p, unload_index, _)| (*p, *unload_index))
    }

    // set path to None if this is a counter-attack
    fn execute_attack(&self, handler: &mut EventHandler<D>, attacker: &Unit<D>, attacker_pos: Point, path: Option<(&Path<D>, Option<(&Unit<D>, Point)>, &[TBallast<D>])>, attacker_heroes: &[(Unit<D>, Hero, Point, Option<usize>)], exhaust_after_attacking: bool, execute_scripts: bool) -> (u32, Vec<usize>, Vec<(i8, i8, u32)>) {
        let temporary_ballast = path.map(|(_, _, t)| t).unwrap_or(&[]);
        let defenders = self.get_splash(attacker, handler.get_game(), attacker_pos, attacker_heroes, temporary_ballast);
        attack_targets(handler, attacker, attacker_pos, path, defenders, attacker_heroes, exhaust_after_attacking, execute_scripts)
    }
}

pub(crate) fn attack_targets<D: Direction>(handler: &mut EventHandler<D>, attacker: &Unit<D>, attacker_pos: Point, path: Option<(&Path<D>, Option<(&Unit<D>, Point)>, &[TBallast<D>])>, targets: Vec<(Point, Option<D>, Rational32)>, attacker_heroes: &[(Unit<D>, Hero, Point, Option<usize>)], exhaust_after_attacking: bool, execute_scripts: bool) -> (u32, Vec<usize>, Vec<(i8, i8, u32)>) {
    let (attacker_id, _) = handler.observe_unit(attacker_pos, None);
    let mut defenders = filter_attack_targets(handler.get_game(), attacker, targets);
    let mut hero_charge;
    let mut attacked_units;
    let mut killed_units;
    match attacker.displacement() {
        Displacement::None | Displacement::AfterCounter => {
            (defenders, hero_charge, attacked_units, killed_units) = deal_damage(handler, attacker, attacker_pos, path, defenders, attacker_heroes);
        }
        Displacement::BeforeAttack => {
            defenders = displace(handler, attacker, attacker_pos, path, defenders, attacker_heroes);
            (defenders, hero_charge, attacked_units, killed_units) = deal_damage(handler, attacker, attacker_pos, path, defenders, attacker_heroes);
        }
        Displacement::BetweenAttacks => {
            (defenders, hero_charge, attacked_units, killed_units) = deal_damage(handler, attacker, attacker_pos, path, defenders, attacker_heroes);
            defenders = displace(handler, attacker, attacker_pos, path, defenders, attacker_heroes);
        }
        Displacement::InsteadOfAttack => {
            defenders = displace(handler, attacker, attacker_pos, path, defenders, attacker_heroes);
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
            (defenders, hero_charge, attacked_units, killed_units) = deal_damage(handler, attacker, attacker_pos, path, collided, attacker_heroes);
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
    for (defender_id, defender_pos, defender, damage) in &attacked_units {
        if attacker.get_team() != defender.get_team() {
            defenders.push((
                attacker.get_owner_id(),
                defender.get_owner_id(),
                (Rational32::from_integer(*damage as i32) * Rational32::from_integer(defender.full_price(handler.get_game(), *defender_pos, None, &[])) / Rational32::from_integer(100))
                .ceil().to_integer().max(0) as u32,
            ));
            if !counter_attackers.contains(defender_id) {
                counter_attackers.push(*defender_id);
            }
        }
    }
    if execute_scripts {
        let temporary_ballast = path.map(|(_, _, t)| t).unwrap_or(&[]);
        let transporter = path.and_then(|(_, t, _)| t);
        /*let (, temporary_ballast) = match path {
            Some((_, tratransporternsporter, temporary_ballast)) => (transporter, temporary_ballast),
            _ => (None, &[])
        };*/
        let mut scripts = Vec::new();
        for (defender_id, defender_pos, defender, damage) in attacked_units {
            let script = attacker.on_attack(handler.get_game(), attacker_pos, &defender, defender_pos, transporter, attacker_heroes, temporary_ballast);
            scripts.push((script, defender_id, defender_pos, defender, damage));
        }
        for (scripts, defender_id, defender_pos, defender, damage) in scripts {
            for script in scripts {
                script.trigger(handler, handler.get_observed_unit_pos(attacker_id), attacker, defender_pos, &defender, handler.get_observed_unit_pos(defender_id), damage);
            }
        }
        let mut scripts = Vec::new();
        for (defender_pos, defender) in killed_units {
            let script = attacker.on_kill(handler.get_game(), attacker_pos, &defender, defender_pos, transporter, attacker_heroes, temporary_ballast);
            scripts.push((script, defender_pos, defender));
        }
        for (scripts, defender_pos, defender) in scripts {
            for script in scripts {
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

fn deal_damage<D: Direction>(handler: &mut EventHandler<D>, attacker: &Unit<D>, attacker_pos: Point, path: Option<(&Path<D>, Option<(&Unit<D>, Point)>, &[TBallast<D>])>, targets: Vec<(Point, Option<D>, Rational32)>, attacker_heroes: &[(Unit<D>, Hero, Point, Option<usize>)]) -> (Vec<(Point, Option<D>, Rational32)>, u32, Vec<(usize, Point, Unit<D>, u8)>, Vec<(Point, Unit<D>)>) {
    let mut raw_damage = HashMap::new();
    let mut hero_charge = 0;
    let mut attack_script_targets = Vec::new();
    for (defender_pos, dir, factor) in targets.iter().cloned() {
        let defender = handler.get_map().get_unit(defender_pos).unwrap();
        let hp = defender.get_hp();
        if hp == 0 {
            continue;
        }
        let damage = calculate_attack_damage(handler.get_game(), attacker, attacker_pos, path, defender, defender_pos, factor, attacker_heroes);
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
    handler.trigger_all_unit_scripts(
        |game, unit, unit_pos, transporter, heroes| {
            if deaths.contains(&unit_pos) {
                unit.on_death(game, unit_pos, transporter, Some((attacker, attacker_pos)), heroes, &[])
            } else {
                Vec::new()
            }
        },
        |handler| handler.unit_mass_death(&deaths),
        |handler, script, unit_pos, unit, _observation_id| {
            script.trigger(handler, unit_pos, unit);
        }
    );
    (filter_attack_targets(handler.get_game(), attacker, targets), hero_charge, attack_script_targets, dead_units)
}

fn calculate_attack_damage<D: Direction>(game: &Game<D>, attacker: &Unit<D>, attacker_pos: Point, path: Option<(&Path<D>, Option<(&Unit<D>, Point)>, &[TBallast<D>])>, defender: &Unit<D>, defender_pos: Point, factor: Rational32, attacker_heroes: &[(Unit<D>, Hero, Point, Option<usize>)]) -> i32 {
    let is_counter = path.is_none();
    let defensive_terrain = game.get_map().get_terrain(defender_pos).unwrap();
    let terrain_defense = Rational32::from_integer(1) + defensive_terrain.defense_bonus(defender);
    let base_attack = Rational32::from_integer(attacker.base_damage(defender.typ()).unwrap() as i32);
    /*for t in game.get_map().get_neighbors(defender_pos, crate::map::map::NeighborMode::Direct).into_iter().map(|p| game.get_map().get_terrain(p.point).unwrap()) {
        terrain_defense += t.adjacent_defense_bonus(defender);
    }*/
    let defender_heroes = Hero::hero_influence_at(game, defender_pos, defender.get_owner_id());
    let defense_bonus = game.environment().config.unit_defense(
        game,
        defender,
        defender_pos,
        attacker,
        attacker_pos,
        defender_heroes.as_slice(),
        is_counter,
    );
    let attack_bonus = game.environment().config.unit_attack(
        game,
        attacker,
        attacker_pos,
        defender,
        defender_pos,
        attacker_heroes,
        path.map(|(_, _, tb)| tb).unwrap_or(&[]),
        is_counter,
    );
    (Rational32::from_integer(attacker.get_hp() as i32) / 100 * base_attack * attack_bonus * factor / defense_bonus / terrain_defense)
    .ceil().to_integer()
}

fn displace<D: Direction>(handler: &mut EventHandler<D>, attacker: &Unit<D>, attacker_pos: Point, path: Option<(&Path<D>, Option<(&Unit<D>, Point)>, &[TBallast<D>])>, mut targets: Vec<(Point, Option<D>, Rational32)>, heroes: &[(Unit<D>, Hero, Point, Option<usize>)]) -> Vec<(Point, Option<D>, Rational32)> {
    let transporter = path.and_then(|(_, transporter, _)| transporter);
    let temporary_ballast = path.map(|(_, _, tb)| tb).unwrap_or(&[]);
    let distance = attacker.displacement_distance(handler.get_game(), attacker_pos, transporter, heroes, temporary_ballast);
    if distance == 0 {
        return targets;
    }
    for (pos, dir, _) in targets.iter_mut().rev() {
        if let Some(d) = dir {
            if !handler.get_map().get_unit(*pos).unwrap().can_be_displaced() {
                *dir = None;
                continue;
            }
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

pub(crate) fn after_attacking<D: Direction>(handler: &mut EventHandler<D>, attacker_id: usize, hero_charge: u32, hero_ids: Vec<usize>, commander_charge: Vec<(i8, i8, u32)>) {
    // add charge to heroes
    if hero_charge > 0 {
        for id in hero_ids {
            if let Some((p, unload_index, _)) = handler.get_observed_unit(id) {
                let change = if id == attacker_id {
                    3
                } else {
                    1
                } * hero_charge;
                handler.hero_charge_add(*p, *unload_index, change.min(u8::MAX as u32) as u8);
            }
        }
    }
    // add charge to commanders
    if commander_charge.len() > 0 {
        let mut charges = HashMap::new();
        for (attacker_owner, defender_owner, charge) in commander_charge {
            //let commander_charge = Rational32::from_integer(damage as i32) * Rational32::from_integer(defender.full_price(handler.get_game(), defender_po)) / Rational32::from_integer(100);
            let old_charge = charges.remove(&defender_owner).unwrap_or(0);
            charges.insert(defender_owner, old_charge + charge);
            let old_charge = charges.remove(&attacker_owner).unwrap_or(0);
            charges.insert(attacker_owner, old_charge + charge / 2);
        }
        for (owner, commander_charge) in charges {
            handler.commander_charge_add(owner, commander_charge);
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
            _ => Effect::ShellFire(p),
        }
    }
}

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
        let wmap: WrappingMap<Direction4> = WMBuilder::new(map).build();
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
        let wmap: WrappingMap<Direction4> = WMBuilder::new(map).build();
        let mut map = Map::new2(wmap, &environment);
        map.set_unit(Point::new(1, 0), Some(UnitType::Magnet.instance(&environment).set_owner_id(0).build_with_defaults()));
        map.set_unit(Point::new(3, 0), Some(UnitType::Sniper.instance(&environment).set_owner_id(0).build_with_defaults()));
        map.set_unit(Point::new(1, 1), Some(UnitType::Custom(0).instance(&environment).set_owner_id(0).build_with_defaults()));
        map.set_unit(Point::new(2, 1), Some(UnitType::Custom(0).instance(&environment).set_owner_id(1).build_with_defaults()));
        map.set_unit(Point::new(3, 1), Some(UnitType::Custom(0).instance(&environment).set_owner_id(1).build_with_defaults()));
        map.set_unit(Point::new(1, 2), Some(UnitType::WarShip.instance(&environment).set_owner_id(1).build_with_defaults()));
        let (mut game, _) = map.game_server(&GameSettings {
            name: "displacement".to_string(),
            fog_mode: FogMode::Constant(crate::game::fog::FogSetting::None),
            players: vec![
                PlayerSettings::new(&environment.config, 0),
                PlayerSettings::new(&environment.config, 1),
            ],
        }, || 0.);
        let unchanged = game.clone();

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

        // WarShip can't be displaced
        let mut game = unchanged.clone();
        game.handle_command(Command::UnitCommand(UnitCommand {
            unload_index: None,
            path: Path::new(Point::new(1, 1)),
            action: UnitAction::Attack(AttackVector::Direction(Direction4::D270)),
        }), || 0.).unwrap();
        assert!(game.get_map().get_unit(Point::new(1, 2)).unwrap().get_hp() < 100);
    }
}
