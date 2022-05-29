use std::collections::{BinaryHeap, HashSet, HashMap};
use std::cmp::{Ordering, Reverse};
use std::fmt;

use crate::game::events::*;
use crate::map::wrapping_map::OrientedPoint;
use crate::player::*;
use crate::map::direction::Direction;
use crate::map::point::Point;
use crate::game::game::Game;
use crate::map::map::{NeighborMode, Map};

#[derive(Debug, PartialEq, Clone)]
pub enum UnitType {
    Normal(NormalUnit),
}
impl UnitType {
    pub fn get_owner(&self) -> Option<&Owner> {
        match self {
            Self::Normal(unit) => Some(&unit.owner),
        }
    }
    pub fn get_team<D: Direction>(&self, game: &Game<D>) -> Option<Team> {
        get_team(self.get_owner(), game)
    }
    pub fn get_hp(&self) -> u8 {
        match self {
            Self::Normal(unit) => unit.hp,
        }
    }
    pub fn can_act(&self, player: &Player) -> bool {
        match self {
            Self::Normal(unit) => !unit.exhausted && unit.owner == player.owner_id,
        }
    }
    pub fn movable_positions<D: Direction>(&self, game: &Game<D>, start: &Point, path_so_far: &Vec<Point>) -> HashSet<Point> {
        match self {
            Self::Normal(unit) => unit.movable_positions(game, start, path_so_far)
        }
    }
    pub fn shortest_path_to<D: Direction>(&self, game: &Game<D>, start: &Point, path_so_far: &Vec<Point>, goal: &Point) -> Option<Vec<Point>> {
        match self {
            Self::Normal(unit) => unit.shortest_path_to(game, start, path_so_far, goal)
        }
    }
    pub fn shortest_path_to_attack<D: Direction>(&self, game: &Game<D>, start: &Point, path_so_far: &Vec<Point>, goal: &Point) -> Option<Vec<Point>> {
        match self {
            Self::Normal(unit) => unit.shortest_path_to_attack(game, start, path_so_far, goal)
        }
    }
    pub fn is_position_targetable<D: Direction>(&self, game: &Game<D>, target: &Point) -> bool {
        match self {
            Self::Normal(unit) => unit.is_position_targetable(game, target)
        }
    }
    pub fn options_after_path<D: Direction>(&self, game: &Game<D>, start: &Point, path: &Vec<Point>) -> Vec<UnitAction> {
        match self {
            Self::Normal(unit) => unit.options_after_path(game, start, path)
        }
    }
    pub fn can_move_to<D: Direction>(&self, p: &Point, game: &Game<D>) -> bool {
        match self {
            Self::Normal(unit) => unit.can_move_to(p, game)
        }
    }
    pub fn check_path<D: Direction>(&self, game: &Game<D>, start: &Point, path: &Vec<Point>) -> Result<(), CommandError> {
        match self {
            Self::Normal(unit) => unit.check_path(game, start, path)
        }
    }
    pub fn get_armor(&self) -> (ArmorType, f32) {
        match self {
            Self::Normal(unit) => unit.typ.get_armor(),
        }
    }
    pub fn calculate_attack_damage<D: Direction>(&self, game: &Game<D>, pos: &Point, a_type: &NormalUnits, a_health: u8, a_player: Option<&Player>, is_counter: bool) -> Option<u16> {
        let (armor_type, defense) = self.get_armor();
        let terrain_defense = if let Some(t) = game.get_map().get_terrain(pos) {
            t.defense(self)
        } else {
            1.
        };
        let mut highest_damage: f32 = 0.;
        for (weapon, attack) in a_type.get_weapons() {
            if let Some(factor) = weapon.damage_factor(&armor_type) {
                let damage = a_health as f32 * attack * factor / defense / terrain_defense;
                highest_damage = highest_damage.max(damage);
            }
        }
        if highest_damage > 0. {
            Some(highest_damage.ceil() as u16)
        } else {
            None
        }
    }
}

pub fn get_team<D: Direction>(owner: Option<&Owner>, game: &Game<D>) -> Option<Team> {
    owner.and_then(|o| game.get_owning_player(o)).and_then(|p| Some(p.team))
}

#[derive(Debug, PartialEq, Clone)]
pub enum NormalUnits {
    Hovercraft,
    TransportHeli(Vec<NormalUnit>),
}
impl NormalUnits {
    pub fn get_attack_type(&self) -> AttackType {
        match self {
            NormalUnits::Hovercraft => AttackType::Adjacent,
            NormalUnits::TransportHeli(_) => AttackType::None,
        }
    }
    pub fn get_weapons(&self) -> Vec<(WeaponType, f32)> {
        match self {
            NormalUnits::Hovercraft => vec![(WeaponType::MachineGun, 1.)],
            NormalUnits::TransportHeli(_) => vec![],
        }
    }
    pub fn get_armor(&self) -> (ArmorType, f32) {
        match self {
            NormalUnits::Hovercraft => (ArmorType::Infantry, 1.5),
            NormalUnits::TransportHeli(_) => (ArmorType::Heli, 1.5),
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct NormalUnit {
    pub typ: NormalUnits,
    pub owner: Owner,
    pub hp: u8,
    pub exhausted: bool,
}
impl NormalUnit {
    pub fn new_instance(from: NormalUnits, color_id: u8) -> NormalUnit {
        NormalUnit {
            typ: from,
            owner: color_id,
            hp: 100,
            exhausted: false,
        }
    }
    pub fn get_movement(&self) -> (MovementType, u8) {
        let factor = 6;
        match self.typ {
            NormalUnits::Hovercraft => (MovementType::Hover, 3 * factor),
            NormalUnits::TransportHeli(_) => (MovementType::Heli, 6 * factor),
        }
    }
    pub fn can_attack_after_moving(&self) -> bool {
        match self.typ {
            NormalUnits::Hovercraft => true,
            NormalUnits::TransportHeli(_) => true,
        }
    }
    fn consider_path_so_far<D: Direction>(&self, game: &Game<D>, start: &Point, path_so_far: &Vec<Point>) -> (HashSet<Point>, MovementType, u8) {
        let (movement_type, mut max_cost) = self.get_movement();
        let mut blocked_positions = HashSet::new();
        if path_so_far.len() > 0 {
            blocked_positions.insert(start.clone());
            for step in path_so_far {
                blocked_positions.insert(step.clone());
                max_cost -= game.get_map().get_terrain(step).unwrap().movement_cost(&movement_type).unwrap();
            }
            blocked_positions.remove(path_so_far.last().unwrap());
        };
        (blocked_positions, movement_type, max_cost)
    }
    pub fn movable_positions<D: Direction>(&self, game: &Game<D>, start: &Point, path_so_far: &Vec<Point>) -> HashSet<Point> {
        let (blocked_positions, movement_type, max_cost) = self.consider_path_so_far(game, start, path_so_far);
        let start = path_so_far.last().unwrap_or(start);
        let mut result = HashSet::new();
        width_search(&movement_type, max_cost, game, start, blocked_positions, get_team(Some(&self.owner), game), |p, _| {
            result.insert(p.clone());
            false
        });
        result
    }
    pub fn shortest_path_to<D: Direction>(&self, game: &Game<D>, start: &Point, path_so_far: &Vec<Point>, goal: &Point) -> Option<Vec<Point>> {
        let (blocked_positions, movement_type, max_cost) = self.consider_path_so_far(game, start, path_so_far);
        let start = path_so_far.last().unwrap_or(start);
        let mut result = None;
        width_search(&movement_type, max_cost, game, start, blocked_positions, get_team(Some(&self.owner), game), |p, path| {
            if p == goal {
                result = Some(path.clone());
                true
            } else {
                false
            }
        });
        result
    }
    pub fn shortest_path_to_attack<D: Direction>(&self, game: &Game<D>, start: &Point, path_so_far: &Vec<Point>, goal: &Point) -> Option<Vec<Point>> {
        if !self.can_attack_after_moving() {
            // no need to look for paths if the unit can't attack after moving
            if path_so_far.len() == 0 && self.attackable_positions(game.get_map(), start, false).contains(goal) {
                return Some(vec![]);
            } else {
                return None;
            }
        }
        let (blocked_positions, movement_type, max_cost) = self.consider_path_so_far(game, start, path_so_far);
        let current_pos = path_so_far.last().unwrap_or(start);
        let mut result = None;
        width_search(&movement_type, max_cost, game, current_pos, blocked_positions, get_team(Some(&self.owner), game), |p, path| {
            if (p == start || self.can_stop_on(p, game)) && self.attackable_positions(game.get_map(), p, path.len() + path_so_far.len() > 0).contains(goal) {
                result = Some(path.clone());
                true
            } else {
                false
            }
        });
        result
    }
    // ignores fog
    pub fn is_position_targetable<D: Direction>(&self, game: &Game<D>, target: &Point) -> bool {
        if let Some(unit) = game.get_map().get_unit(target) {
            // todo: consider healing allies
            unit.get_team(game) != get_team(Some(&self.owner), game) && unit.calculate_attack_damage(game, target, &self.typ, self.hp, game.get_owning_player(&self.owner), false).is_some()
        } else {
            // todo: some units may be able to target "empty" fields
            false
        }
    }
    pub fn options_after_path<D: Direction>(&self, game: &Game<D>, start: &Point, path: &Vec<Point>) -> Vec<UnitAction> {
        let mut result = vec![];
        if path.len() == 0 || game.get_map().get_unit(path.last().unwrap()).is_none() {
            for target in self.attackable_positions(game.get_map(), path.last().unwrap_or(start), path.len() > 0) {
                if self.is_position_targetable(game, &target) {
                    result.push(UnitAction::Attack(target));
                }
            }
            result.push(UnitAction::Wait);
        }
        result
    }
    pub fn attackable_positions<D: Direction>(&self, map: &Map<D>, position: &Point, moved: bool) -> HashSet<Point> {
        let mut result = HashSet::new();
        if moved && !self.can_attack_after_moving() {
            return result;
        }
        match self.typ.get_attack_type() {
            AttackType::None => {},
            AttackType::Adjacent => {
                for p in map.get_neighbors(position, NeighborMode::FollowPipes) {
                    result.insert(p.point().clone());
                }
            }
            AttackType::Straight(min_range, max_range) => {
                for d in D::list() {
                    let mut current_pos = None;
                    for i in 0..max_range {
                        if let Some(dp) = map.get_neighbor(&current_pos.and_then(|dp: OrientedPoint<D>| Some(dp.point().clone())).unwrap_or(*position), &d) {
                            if i + 1 >= min_range {
                                result.insert(dp.point().clone());
                            }
                            current_pos = Some(dp);
                        } else {
                            break;
                        }
                    }
                }
            }
            AttackType::Ranged(min_range, max_range) => {
                // each point in a layer is probably in it 2 times
                let mut layers: Vec<HashSet<(Point, D, Option<D>)>> = vec![];
                let mut layer = HashSet::new();
                for dp in map.get_neighbors(position, NeighborMode::FollowPipes) {
                    layer.insert((dp.point().clone(), dp.direction().clone(), None));
                }
                layers.push(layer);
                while layers.len() < max_range as usize {
                    let mut layer = HashSet::new();
                    for (p, dir, dir_change) in layers.last().unwrap() {
                        if let Some(dp) = map.get_neighbor(p, dir) {
                            layer.insert((dp.point().clone(), dp.direction().clone(), dir_change.clone()));
                        }
                        let mut dir_changes = vec![];
                        if let Some(dir_change) = dir_change {
                            // if we already have 2 directions, only those 2 directions can find new points
                            dir_changes.push(dir_change.clone());
                        } else {
                            // since only one direction has been used so far, try both directions that are directly neighboring
                            let d = **D::list().last().unwrap();
                            dir_changes.push(d.opposite_angle());
                            dir_changes.push(d);
                        }
                        for dir_change in dir_changes {
                            if let Some(dp) = map.get_neighbor(p, &dir.rotate_by(&dir_change)) {
                                let mut dir_change = dir_change.clone();
                                if dp.mirrored() {
                                    dir_change = dir_change.opposite_angle();
                                }
                                layer.insert((dp.point().clone(), dp.direction().clone(), Some(dir_change)));
                            }
                        }
                    }
                    layers.push(layer);
                }
                for _ in min_range-1..max_range {
                    for (p, _, _) in layers.pop().unwrap() {
                        result.insert(p);
                    }
                }
            }
        }
        result
    }
    pub fn can_move_to<D: Direction>(&self, p: &Point, game: &Game<D>) -> bool {
        // doesn't check terrain
        if let Some(unit) = game.get_map().get_unit(p) {
            if !self.can_move_past(game, unit) {
                return false
            }
        }
        true
    }
    pub fn can_stop_on<D: Direction>(&self, p: &Point, game: &Game<D>) -> bool {
        // doesn't check terrain
        if let Some(unit) = game.get_map().get_unit(p) {
            false
        } else {
            true
        }
    }
    fn has_stealth(&self) -> bool {
        false
    }
    fn can_move_past<D: Direction>(&self, game: &Game<D>, other: &UnitType) -> bool {
        // should be false for enemy units unless self has stealth
        if self.has_stealth() {
            true
        } else {
            let self_team = game.get_owning_player(&self.owner).and_then(|p| Some(p.team));
            let other_team = other.get_owner().and_then(|o| game.get_owning_player(o).and_then(|p| Some(p.team)));
            self_team == other_team
        }
    }
    pub fn check_path<D: Direction>(&self, game: &Game<D>, start: &Point, path: &Vec<Point>) -> Result<(), CommandError> {
        let mut blocked = HashSet::new();
        blocked.insert(start.clone());
        let (movement_type, mut remaining_movement) = self.get_movement();
        let mut current = start;
        for p in path {
            // no point can be travelled to twice
            if blocked.contains(p) {
                return Err(CommandError::InvalidPath);
            }
            // check if that unit can move far enough
            if let Some(terrain) = game.get_map().get_terrain(p) {
                if let Some(cost) = terrain.movement_cost(&movement_type) {
                    if cost > remaining_movement {
                        return Err(CommandError::InvalidPath);
                    }
                    remaining_movement -= cost;
                } else {
                    return Err(CommandError::InvalidPath);
                }
            } else {
                // no terrain means the point is invalid
                return Err(CommandError::InvalidPath);
            }
            // the points in the path have to neighbor each other
            if None == game.get_map().get_neighbors(current, NeighborMode::UnitMovement).iter().find(|dp| dp.point() == p) {
                return Err(CommandError::InvalidPath);
            }
            // no visible unit should block movement
            if let Some(unit) = game.get_map().get_unit(p) {
                if game.has_vision_at(game.current_player(), p) && !self.can_move_past(game, unit) {
                    return Err(CommandError::InvalidPath);
                }
            }
            current = p;
            blocked.insert(p.clone());
        }
        Ok(())
    }
}

pub enum MovementType {
    Hover,
    Foot,
    Wheel,
    Treads,
    Heli,
}

#[derive(PartialEq, Eq)]
struct WidthSearch {
    path: Vec<Point>,
    path_cost: u8,
}
impl PartialOrd for WidthSearch {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.path_cost.cmp(&other.path_cost))
    }
}
impl Ord for WidthSearch {
    fn cmp(&self, other: &Self) -> Ordering {
        self.path_cost.cmp(&other.path_cost)
    }
}

// callback returns true if the search can be aborted
// if team is None, units will be ignored
fn width_search<D: Direction, F: FnMut(&Point, &Vec<Point>) -> bool>(movement_type: &MovementType, max_cost: u8, game: &Game<D>, start: &Point, mut blocked_positions: HashSet<Point>, team: Option<Team>, mut callback: F) {
    let mut next_checks = BinaryHeap::new();
    let mut add_point = |p: &Point, path_so_far: &Vec<Point>, cost_so_far: u8, next_checks: &mut BinaryHeap<Reverse<WidthSearch>>| {
        if blocked_positions.contains(p) {
            return false;
        }
        if callback(p, path_so_far) {
            return true;
        }
        blocked_positions.insert(p.clone());
        for neighbor in game.get_map().get_neighbors(p, NeighborMode::UnitMovement) {
            if !blocked_positions.contains(neighbor.point()) {
                if let Some(unit) = game.get_map().get_unit(neighbor.point()) {
                    if team.is_some() && unit.get_team(game) != team {
                        continue;
                    }
                }
                if let Some(cost) = game.get_map().get_terrain(neighbor.point()).unwrap().movement_cost(movement_type) {
                    if cost_so_far + cost <= max_cost {
                        let mut path = path_so_far.clone();
                        path.push(neighbor.point().clone());
                        next_checks.push(Reverse(WidthSearch{path, path_cost: cost_so_far + cost}));
                    }
                }
            }
        }
        false
    };
    add_point(start, &vec![], 0, &mut next_checks);
    while let Some(Reverse(check)) = next_checks.pop() {
        let finished = add_point(check.path.last().unwrap(), &check.path, check.path_cost, &mut next_checks);
        if finished {
            break;
        }
    }
}

pub enum AttackType {
    None,
    Adjacent,
    Ranged(u8, u8),
    Straight(u8, u8),
}

pub enum WeaponType {
    MachineGun,
    Shells,
    AntiAir,
    Flame,
    Rocket,
    Torpedo,
    Rifle,
    // immobile ranged
    SurfaceMissiles,
    AirMissiles,
}
impl WeaponType {
    pub fn damage_factor(&self, armor: &ArmorType) -> Option<f32> {
        match (self, armor) {
            (Self::MachineGun, ArmorType::Infantry) => Some(1.00),
            (Self::MachineGun, ArmorType::Light) => Some(0.30),
            (Self::MachineGun, ArmorType::Heavy) => Some(0.10),
            (Self::MachineGun, ArmorType::Heli) => Some(0.30),
            (Self::MachineGun, ArmorType::Plane) => None,
            (Self::MachineGun, ArmorType::Submarine) => Some(0.40),
            (Self::MachineGun, ArmorType::Structure) => Some(0.20),

            (Self::Shells, ArmorType::Infantry) => Some(0.90),
            (Self::Shells, ArmorType::Light) => Some(1.10),
            (Self::Shells, ArmorType::Heavy) => Some(1.00),
            (Self::Shells, ArmorType::Heli) => None,
            (Self::Shells, ArmorType::Plane) => None,
            (Self::Shells, ArmorType::Submarine) => Some(1.00),
            (Self::Shells, ArmorType::Structure) => Some(1.00),
            
            (Self::AntiAir, ArmorType::Heli) => Some(1.50),
            (Self::AntiAir, ArmorType::Plane) => Some(1.20),
            (Self::AntiAir, _) => None,

            (Self::Flame, ArmorType::Infantry) => Some(1.20),
            (Self::Flame, ArmorType::Light) => Some(0.35),
            (Self::Flame, ArmorType::Heavy) => Some(0.10),
            (Self::Flame, ArmorType::Heli) => Some(0.50),
            (Self::Flame, ArmorType::Plane) => None,
            (Self::Flame, ArmorType::Submarine) => Some(0.50),
            (Self::Flame, ArmorType::Structure) => Some(0.05),

            (Self::Rocket, ArmorType::Infantry) => Some(0.70),
            (Self::Rocket, ArmorType::Light) => Some(0.70),
            (Self::Rocket, ArmorType::Heavy) => Some(1.20),
            (Self::Rocket, ArmorType::Heli) => Some(1.10),
            (Self::Rocket, ArmorType::Plane) => None,
            (Self::Rocket, ArmorType::Submarine) => Some(1.00),
            (Self::Rocket, ArmorType::Structure) => Some(1.20),

            (Self::Torpedo, ArmorType::Light) => Some(1.30),
            (Self::Torpedo, ArmorType::Heavy) => Some(0.70),
            (Self::Torpedo, ArmorType::Submarine) => Some(1.00),
            (Self::Torpedo, ArmorType::Structure) => Some(1.00),
            (Self::Torpedo, _) => None,

            (Self::Rifle, ArmorType::Infantry) => Some(1.10),
            (Self::Rifle, ArmorType::Light) => Some(0.75),
            (Self::Rifle, ArmorType::Heavy) => Some(0.20),
            (Self::Rifle, ArmorType::Heli) => Some(0.75),
            (Self::Rifle, ArmorType::Plane) => Some(0.20),
            (Self::Rifle, ArmorType::Submarine) => Some(0.10),
            (Self::Rifle, ArmorType::Structure) => Some(0.10),

            (Self::SurfaceMissiles, ArmorType::Infantry) => Some(1.20),
            (Self::SurfaceMissiles, ArmorType::Light) => Some(1.20),
            (Self::SurfaceMissiles, ArmorType::Heavy) => Some(1.00),
            (Self::SurfaceMissiles, ArmorType::Heli) => None,
            (Self::SurfaceMissiles, ArmorType::Plane) => None,
            (Self::SurfaceMissiles, ArmorType::Submarine) => Some(1.00),
            (Self::SurfaceMissiles, ArmorType::Structure) => Some(0.80),

            (Self::AirMissiles, ArmorType::Heli) => Some(1.20),
            (Self::AirMissiles, ArmorType::Plane) => Some(1.00),
            (Self::AirMissiles, _) => None,
        }
    }
}

pub enum ArmorType {
	Infantry,
	Light,
	Heavy,
	Heli,
	Plane,
	Submarine,
	Structure,
}

#[derive(Debug, Clone)]
pub enum UnitAction {
    Wait,
    Attack(Point),
}
impl fmt::Display for UnitAction {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Wait => write!(f, "Wait"),
            Self::Attack(p) => write!(f, "Attack {:?}", p),
        }
    }
}

pub enum UnitCommand {
    MoveAttack(Point, Vec<Point>, Point),
    MoveWait(Point, Vec<Point>),
}
impl UnitCommand {
    fn check_unit_can_act<D: Direction>(game: &Game<D>, at: &Point) -> Result<(), CommandError> {
        if !game.has_vision_at(game.current_player(), at) {
            return Err(CommandError::NoVision);
        }
        if let Some(unit) = game.get_map().get_unit(at) {
            if Some(&game.current_player().owner_id) != unit.get_owner() {
                return Err(CommandError::NotYourUnit);
            }
            if !unit.can_act(game.current_player()) {
                return Err(CommandError::UnitCannotMove);
            }
            Ok(())
        } else {
            Err(CommandError::MissingUnit)
        }
    }
    fn check_unit_can_wait_after_path<D: Direction>(game: &Game<D>, start: &Point, path: &Vec<Point>) -> Result<(), CommandError> {
        if !game.get_map().wrapping_logic().pointmap().is_point_valid(start) {
            return Err(CommandError::InvalidPoint(start.clone()));
        }
        for p in path {
            if !game.get_map().wrapping_logic().pointmap().is_point_valid(p) {
                return Err(CommandError::InvalidPoint(p.clone()));
            }
        }
        Self::check_unit_can_act(game, start)?;
        let unit = game.get_map().get_unit(start).unwrap(); // unwrap because already checked by check_unit_can_act
        unit.check_path(game, start, path)?;
        if let Some(p) = path.last() {
            if let Some(_) = game.get_map().get_unit(p) {
                if game.has_vision_at(game.current_player(), p) {
                    return Err(CommandError::InvalidPath);
                }
            }
        }
        Ok(())
    }
    /**
     * only checks whether the command appears valid to the player that sent it
     */
    pub fn check_validity<D: Direction>(&self, game: &Game<D>) -> Result<(), CommandError> {
        match self {
            Self::MoveAttack(start, path, target) => {
                Self::check_unit_can_wait_after_path(game, start, path)?;
                if !game.get_map().wrapping_logic().pointmap().is_point_valid(target) {
                    return Err(CommandError::InvalidPoint(target.clone()));
                }
                if !game.has_vision_at(game.current_player(), target) {
                    return Err(CommandError::NoVision);
                }
                let unit = game.get_map().get_unit(start).unwrap(); // unwrap because already checked in check_unit_can_wait_after_path
                if !unit.is_position_targetable(game, target) {
                    return Err(CommandError::InvalidTarget);
                }
                match unit {
                    UnitType::Normal(unit) => {
                        if !unit.attackable_positions(game.get_map(), path.last().unwrap_or(start), path.len() > 1).contains(target) {
                            return Err(CommandError::InvalidTarget);
                        }
                        if let Some(defender) = game.get_map().get_unit(&target) {
                            let damage = defender.calculate_attack_damage(game, &target, &unit.typ, unit.hp, game.get_owning_player(&unit.owner), false);
                            if damage.is_none() {
                                return Err(CommandError::InvalidTarget);
                            }
                        }
                        Ok(())
                    }
                }
            }
            Self::MoveWait(start, path) => {
                Self::check_unit_can_wait_after_path(game, start, path)
            }
        }
    }
    // returns the point the unit ended on
    fn apply_path<D: Direction>(game: &Game<D>, start: Point, path: Vec<Point>, result: &mut Vec<Event>) -> Point {
        let unit = game.get_map().get_unit(&start).unwrap();
        let mut path_taken = vec![];
        for p in path {
            if !unit.can_move_to(&p, game) {
                break;
            }
            path_taken.push(p);
        }
        let end = path_taken.last().unwrap_or(&start).clone();
        if path_taken.len() > 0 {
            result.push(Event::UnitPath(start, path_taken));
        }
        end
    }
    pub fn calculate_hp_after_attack<D: Direction>(game: &Game<D>, attacker: &UnitType, attacker_pos: &Point, target: &Point, events: &mut Vec<Event>) -> HashMap<Point, i16> {
        let mut hp_values = HashMap::new();
        match attacker {
            UnitType::Normal(attacker) => {
                if let Some(defender) = game.get_map().get_unit(target) {
                    let damage = defender.calculate_attack_damage(game, target, &attacker.typ, attacker.hp, game.get_owning_player(&attacker.owner), false);
                    if let Some(damage) = damage {
                        let hp: i16 = *hp_values.get(target).unwrap_or(&(defender.get_hp() as i16));
                        if hp > 0 {
                            events.push(Event::UnitHpChange(target.clone(), -(damage.min(hp as u16) as i8)));
                        }
                        hp_values.insert(target.clone(), hp - damage as i16);
                    }
                }
            }
        }
        // counter attack
        let counter_hp = *hp_values.get(target).unwrap_or(&0);
        if counter_hp > 0 {
            match game.get_map().get_unit(target).unwrap() {
                UnitType::Normal(counter_attacker) => {
                    let damage = attacker.calculate_attack_damage(game, attacker_pos, &counter_attacker.typ, counter_hp as u8, game.get_owning_player(&counter_attacker.owner), true);
                    if let Some(damage) = damage {
                        let hp: i16 = *hp_values.get(attacker_pos).unwrap_or(&(attacker.get_hp() as i16));
                        if hp > 0 {
                            events.push(Event::UnitHpChange(attacker_pos.clone(), -(damage.min(hp as u16) as i8)));
                        }
                        hp_values.insert(attacker_pos.clone(), hp - damage as i16);
                    }
                }
            }
        }
        hp_values
    }
    pub fn apply<D: Direction>(self, game: &Game<D>) -> Vec<Event> {
        let mut result = vec![];
        match self {
            Self::MoveAttack(start, path, target) => {
                let intended_end = path.last().unwrap_or(&start).clone();
                let end = Self::apply_path(game, start, path, &mut result);
                if end == intended_end {
                    // no fog trap
                    // the attacker hasn't actually been moved on the board yet.
                    if let Some(unit) = game.get_map().get_unit(&start) {
                        let hp_values = Self::calculate_hp_after_attack(game, unit, &end, &target, &mut result);
                        for (point, hp) in hp_values.into_iter() {
                            if hp <= 0 {
                                let unit = game.get_map().get_unit(&point).expect(&format!("expected a unit at {:?} to die!", point));
                                result.push(Event::UnitDeath(point, unit.clone()));
                            }
                        }
                    }
                }
                result.push(Event::UnitExhaust(end));
            }
            Self::MoveWait(start, path) => {
                let end = Self::apply_path(game, start, path, &mut result);
                result.push(Event::UnitExhaust(end));
            }
        }
        result
    }
}
