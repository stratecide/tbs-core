pub mod chess;
pub mod structures;
pub mod mercenary;

use std::collections::{BinaryHeap, HashSet, HashMap};
use std::cmp::{Ordering, Reverse};
use std::fmt;

use crate::game::events::*;
use crate::map::wrapping_map::{OrientedPoint};
use crate::player::*;
use crate::map::direction::Direction;
use crate::map::point::Point;
use crate::game::game::Game;
use crate::map::map::{NeighborMode, Map};
use crate::terrain::*;
use self::chess::*;
use self::structures::*;
use self::mercenary::*;

#[derive(Debug, PartialEq, Clone)]
pub enum UnitType<D: Direction> {
    Normal(NormalUnit),
    Mercenary(Mercenary),
    Chess(ChessUnit),
    Structure(Structure<D>),
}
impl<D: Direction> UnitType<D> {
    pub fn as_normal_trait(&self) -> Option<&dyn NormalUnitTrait<D>> {
        match self {
            Self::Normal(unit) => Some(unit.as_trait()),
            Self::Mercenary(merc) => Some(merc.as_trait()),
            _ => None,
        }
    }
    pub fn as_transportable(self) -> Option<TransportableTypes> {
        match self {
            Self::Normal(u) => Some(TransportableTypes::Normal(u)),
            Self::Mercenary(u) => Some(TransportableTypes::Mercenary(u)),
            _ => None,
        }
    }
    pub fn name(&self) -> &'static str {
        match self {
            Self::Normal(unit) => unit.typ.name(),
            Self::Mercenary(merc) => merc.typ.name(),
            Self::Chess(unit) => unit.typ.name(),
            Self::Structure(unit) => unit.typ.name(),
        }
    }
    pub fn get_owner(&self) -> Option<&Owner> {
        match self {
            Self::Normal(unit) => Some(&unit.owner),
            Self::Mercenary(unit) => Some(&unit.unit.owner),
            Self::Chess(unit) => Some(&unit.owner),
            Self::Structure(unit) => unit.owner.as_ref(),
        }
    }
    pub fn get_team(&self, game: &Game<D>) -> Option<Team> {
        get_team(self.get_owner(), game)
    }
    pub fn get_hp(&self) -> u8 {
        match self {
            Self::Normal(unit) => unit.hp,
            Self::Mercenary(unit) => unit.unit.hp,
            Self::Chess(unit) => unit.hp,
            Self::Structure(unit) => unit.hp,
        }
    }
    pub fn get_hp_mut(&mut self) -> &mut u8 {
        match self {
            Self::Normal(unit) => &mut unit.hp,
            Self::Mercenary(unit) => &mut unit.unit.hp,
            Self::Chess(unit) => &mut unit.hp,
            Self::Structure(unit) => &mut unit.hp,
        }
    }
    pub fn is_exhausted(&self) -> bool {
        match self {
            Self::Normal(unit) => unit.exhausted,
            Self::Mercenary(merc) => merc.unit.exhausted,
            Self::Chess(unit) => unit.exhausted,
            Self::Structure(_) => false,
        }
    }
    pub fn set_exhausted(&mut self, exhausted: bool) {
        match self {
            Self::Normal(unit) => unit.exhausted = exhausted,
            Self::Mercenary(merc) => merc.unit.exhausted = exhausted,
            Self::Chess(unit) => unit.exhausted = exhausted,
            Self::Structure(_) => {},
        }
    }
    pub fn can_act(&self, player: &Player) -> bool {
        let u: &dyn NormalUnitTrait<D> = match self {
            Self::Normal(unit) => unit.as_trait(),
            Self::Mercenary(unit) => unit.as_trait(),
            Self::Chess(unit) => return !unit.exhausted && unit.owner == player.owner_id,
            Self::Structure(_) => return false,
        };
        u.can_act(player)
    }
    pub fn get_boarded(&self) -> Vec<&TransportableTypes> {
        match self {
            Self::Normal(unit) => unit.typ.get_boarded(),
            Self::Mercenary(merc) => merc.unit.typ.get_boarded(),
            Self::Chess(_) => vec![],
            Self::Structure(_struc) => vec![],
        }
    }
    pub fn get_boarded_mut(&mut self) -> Vec<&mut TransportableTypes> {
        match self {
            Self::Normal(unit) => unit.typ.get_boarded_mut(),
            Self::Mercenary(merc) => merc.unit.typ.get_boarded_mut(),
            Self::Chess(_) => vec![],
            Self::Structure(_struc) => vec![],
        }
    }
    pub fn unboard(&mut self, index: u8) {
        match self {
            Self::Normal(unit) => unit.typ.unboard(index),
            Self::Mercenary(merc) => merc.unit.typ.unboard(index),
            _ => {}
        }
    }
    pub fn boardable_by(&self, unit: TransportableTypes) -> bool {
        if self.get_owner() != Some(unit.get_owner()) {
            return false;
        }
        let boarded_count = self.get_boarded().len() as u8;
        let normal_typ = match unit {
            TransportableTypes::Normal(u) => u.typ,
            TransportableTypes::Mercenary(m) => m.unit.typ,
        };
        match self {
            Self::Normal(u) => boarded_count < u.typ.transport_capacity() && u.typ.could_transport(&normal_typ),
            Self::Mercenary(m) => boarded_count < m.unit.typ.transport_capacity() && m.unit.typ.could_transport(&normal_typ),
            _ => false,
        }
    }
    pub fn board(&mut self, index: u8, unit: TransportableTypes) {
        match self {
            Self::Normal(u) => u.typ.board(index, unit),
            Self::Mercenary(merc) => merc.unit.typ.board(index, unit),
            _ => {}
        }
    }
    pub fn movable_positions(&self, game: &Game<D>, start: &Point, path_so_far: &Vec<Point>) -> HashSet<Point> {
        match self {
            Self::Normal(unit) => unit.movable_positions(game, start, path_so_far),
            Self::Mercenary(unit) => unit.movable_positions(game, start, path_so_far),
            Self::Chess(unit) => unit.movable_positions(game, start, path_so_far),
            Self::Structure(_) => HashSet::new(),
        }
    }
    pub fn shortest_path_to(&self, game: &Game<D>, start: &Point, path_so_far: &Vec<Point>, goal: &Point) -> Option<Vec<Point>> {
        match self {
            Self::Normal(unit) => unit.shortest_path_to(game, start, path_so_far, goal),
            Self::Mercenary(unit) => unit.shortest_path_to(game, start, path_so_far, goal),
            Self::Chess(unit) => unit.shortest_path_to(game, start, path_so_far, goal),
            Self::Structure(_) => None,
        }
    }
    pub fn options_after_path(&self, game: &Game<D>, start: &Point, path: &Vec<Point>) -> Vec<UnitAction<D>> {
        match self {
            Self::Normal(unit) => unit.options_after_path(game, start, path),
            Self::Mercenary(unit) => unit.options_after_path(game, start, path),
            Self::Chess(_) => vec![UnitAction::Wait],
            Self::Structure(_) => vec![],
        }
    }
    pub fn get_armor(&self) -> (ArmorType, f32) {
        match self {
            Self::Normal(unit) => unit.typ.get_armor(),
            Self::Mercenary(unit) => unit.get_armor(),
            Self::Chess(unit) => unit.typ.get_armor(),
            Self::Structure(unit) => unit.typ.get_armor(),
        }
    }
    pub fn killable_by_chess(&self, team: Team, game: &Game<D>) -> bool {
        match self {
            _ => self.get_team(game) != Some(team),
        }
    }
    pub fn can_be_moved_through(&self, by: &dyn NormalUnitTrait<D>, game: &Game<D>) -> bool {
        match self {
            Self::Normal(_) => by.has_stealth() || self.get_team(game) == by.get_team(game),
            Self::Mercenary(_) => by.has_stealth() || self.get_team(game) == by.get_team(game),
            Self::Chess(_) => false,
            Self::Structure(_) => false,
        }
    }
    pub fn calculate_attack_damage(&self, game: &Game<D>, pos: &Point, attacker_pos: &Point, attacker: &dyn NormalUnitTrait<D>, is_counter: bool) -> Option<u16> {
        let (armor_type, defense) = self.get_armor();
        let terrain_defense = if let Some(t) = game.get_map().get_terrain(pos) {
            t.defense(self)
        } else {
            1.
        };
        let mut highest_damage: f32 = 0.;
        for (weapon, attack) in attacker.get_weapons() {
            if let Some(factor) = weapon.damage_factor(&armor_type) {
                let mut damage = attacker.get_hp() as f32 * attack * factor / defense / terrain_defense;
                for (_, merc) in game.get_map().mercenary_influence_at(attacker_pos, Some(attacker.get_owner())) {
                    damage *= merc.attack_bonus(attacker, is_counter);
                }
                for (_, merc) in game.get_map().mercenary_influence_at(pos, self.get_owner()) {
                    damage /= merc.defense_bonus(self, is_counter);
                }
                highest_damage = highest_damage.max(damage);
            }
        }
        if highest_damage > 0. {
            Some(highest_damage.ceil() as u16)
        } else {
            None
        }
    }
    fn true_vision_range(&self, _game: &Game<D>, _pos: &Point) -> usize {
        1
    }
    fn vision_range(&self, _game: &Game<D>, _pos: &Point) -> usize {
        2
    }
    pub fn get_vision(&self, game: &Game<D>, pos: &Point) -> HashSet<Point> {
        match self {
            Self::Chess(unit) => unit.get_vision(game, pos),
            _ => {
                let mut result = HashSet::new();
                result.insert(pos.clone());
                let layers = range_in_layers(game.get_map(), pos, self.vision_range(game, pos));
                for (i, layer) in layers.into_iter().enumerate() {
                    for (p, _, _) in layer {
                        if i < self.true_vision_range(game, pos) || !game.get_map().get_terrain(&p).unwrap().requires_true_sight() {
                            result.insert(p);
                        }
                    }
                }
                result
            }
        }
    }
}

pub fn get_team<D: Direction>(owner: Option<&Owner>, game: &Game<D>) -> Option<Team> {
    owner.and_then(|o| game.get_owning_player(o)).and_then(|p| Some(p.team))
}


#[derive(Debug, PartialEq, Clone)]
pub enum TransportableTypes {
    Normal(NormalUnit),
    Mercenary(Mercenary),
}
impl TransportableTypes {
    pub fn as_unit<D: Direction>(self) -> UnitType<D> {
        match self {
            Self::Normal(u) => UnitType::Normal(u),
            Self::Mercenary(u) => UnitType::Mercenary(u),
        }
    }
    pub fn as_trait<D: Direction>(&self) -> &dyn NormalUnitTrait<D> {
        match self {
            Self::Normal(u) => u.as_trait(),
            Self::Mercenary(u) => u.as_trait(),
        }
    }
    pub fn get_owner(&self) -> &Owner {
        match self {
            Self::Normal(u) => &u.owner,
            Self::Mercenary(m) => &m.unit.owner,
        }
    }
    pub fn is_exhausted(&self) -> bool {
        match self {
            Self::Normal(unit) => unit.exhausted,
            Self::Mercenary(merc) => merc.unit.exhausted,
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum NormalUnits {
    Hovercraft,
    TransportHeli(Vec<TransportableTypes>),
    DragonHead,
    Artillery,
}
impl NormalUnits {
    pub fn name(&self) -> &'static str {
        match self {
            NormalUnits::Hovercraft => "Hovercraft",
            NormalUnits::TransportHeli(_) => "Transport Helicopter",
            NormalUnits::DragonHead => "Dragon Head",
            NormalUnits::Artillery => "Artillery",
        }
    }
    pub fn get_boarded(&self) -> Vec<&TransportableTypes> {
        match self {
            NormalUnits::TransportHeli(units) => units.iter().collect(),
            _ => vec![],
        }
    }
    pub fn get_boarded_mut(&mut self) -> Vec<&mut TransportableTypes> {
        match self {
            NormalUnits::TransportHeli(units) => units.iter_mut().collect(),
            _ => vec![],
        }
    }
    pub fn transport_capacity(&self) -> u8 {
        match self {
            NormalUnits::TransportHeli(_) => 1,
            _ => 0,
        }
    }
    pub fn could_transport(&self, unit: &NormalUnits) -> bool {
        match self {
            NormalUnits::TransportHeli(_) => {
                match unit {
                    NormalUnits::Hovercraft => true,
                    _ => false,
                }
            }
            _ => false
        }
    }
    pub fn unboard(&mut self, index: u8) {
        let units = match self {
            NormalUnits::TransportHeli(units) => units,
            _ => return,
        };
        if units.len() > index as usize {
            units.remove(index as usize);
        }
    }
    pub fn board(&mut self, index: u8, unit: TransportableTypes) {
        let units = match self {
            NormalUnits::TransportHeli(units) => units,
            _ => return,
        };
        if units.len() >= index as usize {
            units.insert(index as usize, unit);
        }
    }
    pub fn get_attack_type(&self) -> AttackType {
        match self {
            NormalUnits::Hovercraft => AttackType::Adjacent,
            NormalUnits::TransportHeli(_) => AttackType::None,
            NormalUnits::DragonHead => AttackType::Straight(1, 2),
            NormalUnits::Artillery => AttackType::Ranged(2, 3),
        }
    }
    pub fn get_weapons(&self) -> Vec<(WeaponType, f32)> {
        match self {
            NormalUnits::Hovercraft => vec![(WeaponType::MachineGun, 1.)],
            NormalUnits::TransportHeli(_) => vec![],
            NormalUnits::DragonHead => vec![(WeaponType::Flame, 1.)],
            NormalUnits::Artillery => vec![(WeaponType::SurfaceMissiles, 1.)],
        }
    }
    pub fn get_armor(&self) -> (ArmorType, f32) {
        match self {
            NormalUnits::Hovercraft => (ArmorType::Infantry, 1.5),
            NormalUnits::TransportHeli(_) => (ArmorType::Heli, 1.5),
            NormalUnits::DragonHead => (ArmorType::Light, 1.5),
            NormalUnits::Artillery => (ArmorType::Light, 1.5),
        }
    }
    pub fn value(&self) -> u16 {
        match self {
            NormalUnits::Hovercraft => 100,
            NormalUnits::TransportHeli(_) => 500,
            NormalUnits::DragonHead => 400,
            NormalUnits::Artillery => 600,
        }
    }
}


pub trait NormalUnitTrait<D: Direction> {
    fn as_trait(&self) -> &dyn NormalUnitTrait<D>;
    fn as_transportable(self) -> TransportableTypes;
    fn get_hp(&self) -> u8;
    fn get_weapons(&self) -> Vec<(WeaponType, f32)>;
    fn get_owner(&self) -> &Owner;
    fn get_team(&self, game: &Game<D>) -> Option<Team>;
    fn can_act(&self, player: &Player) -> bool;
    fn get_movement(&self) -> (MovementType, u8);
    fn has_stealth(&self) -> bool;
    fn shortest_path_to(&self, game: &Game<D>, start: &Point, path_so_far: &Vec<Point>, goal: &Point) -> Option<Vec<Point>> {
        let (blocked_positions, movement_type, max_cost) = self.consider_path_so_far(game, start, path_so_far);
        let start = path_so_far.last().unwrap_or(start);
        let mut result = None;
        width_search(&movement_type, max_cost, game, start, blocked_positions, Some(self.as_trait()), |p, path| {
            if p == goal {
                result = Some(path.clone());
                true
            } else {
                false
            }
        });
        result
    }
    fn options_after_path(&self, game: &Game<D>, start: &Point, path: &Vec<Point>) -> Vec<UnitAction<D>>;
    fn can_stop_on(&self, p: &Point, game: &Game<D>) -> bool {
        // doesn't check terrain
        if let Some(_) = game.get_map().get_unit(p) {
            false
        } else {
            true
        }
    }
    fn can_attack_after_moving(&self) -> bool;
    fn shortest_path_to_attack(&self, game: &Game<D>, start: &Point, path_so_far: &Vec<Point>, goal: &Point) -> Option<Vec<Point>> {
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
        width_search(&movement_type, max_cost, game, current_pos, blocked_positions, Some(self.as_trait()), |p, path| {
            if (p == start || self.can_stop_on(p, game)) && self.attackable_positions(game.get_map(), p, path.len() + path_so_far.len() > 0).contains(goal) {
                result = Some(path.clone());
                true
            } else {
                false
            }
        });
        result
    }
    fn can_move_to(&self, p: &Point, game: &Game<D>) -> bool {
        // doesn't check terrain
        if let Some(unit) = game.get_map().get_unit(p) {
            if !unit.can_be_moved_through(self.as_trait(), game) {
                return false
            }
        }
        true
    }
    fn consider_path_so_far(&self, game: &Game<D>, start: &Point, path_so_far: &Vec<Point>) -> (HashSet<Point>, MovementType, u8) {
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
    fn movable_positions(&self, game: &Game<D>, start: &Point, path_so_far: &Vec<Point>) -> HashSet<Point> {
        let (blocked_positions, movement_type, max_cost) = self.consider_path_so_far(game, start, path_so_far);
        let start = path_so_far.last().unwrap_or(start);
        let mut result = HashSet::new();
        width_search(&movement_type, max_cost, game, start, blocked_positions, Some(self.as_trait()), |p, _| {
            result.insert(p.clone());
            false
        });
        result
    }
    fn check_path(&self, game: &Game<D>, start: &Point, path: &Vec<Point>) -> Result<Vec<Point>, CommandError> {
        let mut blocked = HashSet::new();
        blocked.insert(start.clone());
        let (movement_type, mut remaining_movement) = self.get_movement();
        let mut current = start;
        let mut path_taken = vec![];
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
                if game.has_vision_at(Some(game.current_player().team), p) && !unit.can_be_moved_through(self.as_trait(), game) {
                    return Err(CommandError::InvalidPath);
                }
            }
            if !self.can_move_to(&p, game) {
                break;
            }
            current = p;
            path_taken.push(p.clone());
            blocked.insert(p.clone());
        }
        Ok(path_taken)
    }
    fn get_attack_type(&self) -> AttackType;
    fn is_position_targetable(&self, game: &Game<D>, target: &Point) -> bool;
    fn can_attack_unit_type(&self, game: &Game<D>, target: &UnitType<D>) -> bool;
    fn attackable_positions(&self, map: &Map<D>, position: &Point, moved: bool) -> HashSet<Point>;
    // the result-vector should never contain the same point multiple times
    fn attack_splash(&self, map: &Map<D>, from: &Point, to: &AttackInfo<D>) -> Result<Vec<Point>, CommandError>;
    fn make_attack_info(&self, map: &Map<D>, from: &Point, to: &Point) -> Option<AttackInfo<D>>;
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
    pub fn can_capture(&self) -> bool {
        match self.typ {
            NormalUnits::Hovercraft => true,
            _ => false,
        }
    }
}
impl<D: Direction> NormalUnitTrait<D> for NormalUnit {
    fn as_trait(&self) -> &dyn NormalUnitTrait<D> {
        self
    }
    fn as_transportable(self) -> TransportableTypes {
        TransportableTypes::Normal(self)
    }
    fn get_hp(&self) -> u8 {
        self.hp
    }
    fn get_weapons(&self) -> Vec<(WeaponType, f32)> {
        self.typ.get_weapons()
    }
    fn get_owner(&self) -> &Owner {
        &self.owner
    }
    fn get_team(&self, game: &Game<D>) -> Option<Team> {
        get_team(Some(&self.owner), game)
    }
    fn can_act(&self, player: &Player) -> bool {
        !self.exhausted && player.owner_id == self.owner
    }
    fn get_movement(&self) -> (MovementType, u8) {
        let factor = 6;
        match self.typ {
            NormalUnits::Hovercraft => (MovementType::Hover, 3 * factor),
            NormalUnits::TransportHeli(_) => (MovementType::Heli, 6 * factor),
            NormalUnits::DragonHead => (MovementType::Wheel, 6 * factor),
            NormalUnits::Artillery => (MovementType::Treads, 5 * factor),
        }
    }
    fn has_stealth(&self) -> bool {
        false
    }
    fn options_after_path(&self, game: &Game<D>, start: &Point, path: &Vec<Point>) -> Vec<UnitAction<D>> {
        let mut result = vec![];
        let destination = path.last().unwrap_or(start).clone();
        if path.len() == 0 || game.get_map().get_unit(&destination).is_none() {
            for target in self.attackable_positions(game.get_map(), &destination, path.len() > 0) {
                if self.is_position_targetable(game, &target) {
                    if let Some(attack_info) = self.make_attack_info(game.get_map(), &destination, &target) {
                        result.push(UnitAction::Attack(attack_info));
                    }
                }
            }
            if self.can_capture() {
                match game.get_map().get_terrain(&destination) {
                    Some(Terrain::Realty(_, owner)) => {
                        if Some(game.get_owning_player(&self.owner).unwrap().team) != owner.and_then(|o| game.get_owning_player(&o)).and_then(|p| Some(p.team)) {
                            result.push(UnitAction::Capture);
                        }
                    }
                    _ => {}
                }
            }
            result.push(UnitAction::Wait);
        } else if path.len() > 0 {
            if let Some(transporter) = game.get_map().get_unit(&destination) {
                // TODO: this is called indirectly by mercenaries, so using ::Normal isn't necessarily correct
                if transporter.boardable_by(TransportableTypes::Normal(self.clone())) {
                    result.push(UnitAction::Enter);
                }
            }
        }
        result
    }
    fn can_attack_after_moving(&self) -> bool {
        match self.typ {
            NormalUnits::Hovercraft => true,
            NormalUnits::TransportHeli(_) => true,
            NormalUnits::DragonHead => true,
            NormalUnits::Artillery => false,
        }
    }
    fn get_attack_type(&self) -> AttackType {
        self.typ.get_attack_type()
    }
    // ignores fog
    fn is_position_targetable(&self, game: &Game<D>, target: &Point) -> bool {
        if let Some(unit) = game.get_map().get_unit(target) {
            self.can_attack_unit_type(game, unit)
        } else {
            false
        }
    }
    fn can_attack_unit_type(&self, game: &Game<D>, target: &UnitType<D>) -> bool {
        let this: &dyn NormalUnitTrait<D> = self.as_trait();
        target.get_team(game) != self.get_team(game) && this.get_weapons().iter().any(|(weapon, _)| weapon.damage_factor(&target.get_armor().0).is_some())
    }
    fn attackable_positions(&self, map: &Map<D>, position: &Point, moved: bool) -> HashSet<Point> {
        let mut result = HashSet::new();
        let this: &dyn NormalUnitTrait<D> = self.as_trait();
        if moved && !this.can_attack_after_moving() {
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
                let mut layers = range_in_layers(map, position, max_range as usize);
                for _ in min_range-1..max_range {
                    for (p, _, _) in layers.pop().unwrap() {
                        result.insert(p);
                    }
                }
            }
        }
        result
    }
    fn attack_splash(&self, map: &Map<D>, from: &Point, to: &AttackInfo<D>) -> Result<Vec<Point>, CommandError> {
        match (&self.typ, to) {
            (NormalUnits::DragonHead, AttackInfo::Direction(dir)) => {
                if let Some(dp) = map.get_neighbor(from, dir) {
                    let mut result = vec![dp.point().clone()];
                    if let Some(dp) = map.get_neighbor(dp.point(), dir) {
                        result.push(dp.point().clone());
                    }
                    Ok(result)
                } else {
                    Err(CommandError::InvalidTarget)
                }
            }
            (NormalUnits::DragonHead, AttackInfo::Point(_)) => {
                Err(CommandError::InvalidTarget)
            }
            (_, AttackInfo::Point(p)) => Ok(vec![p.clone()]),
            _ => Err(CommandError::InvalidTarget),
        }
    }
    fn make_attack_info(&self, map: &Map<D>, from: &Point, to: &Point) -> Option<AttackInfo<D>> {
        match self.typ.get_attack_type() {
            AttackType::Straight(min, max) => {
                for d in D::list() {
                    let mut current = OrientedPoint::new(*from, false, *d);
                    for i in 0..max {
                        if let Some(dp) = map.get_neighbor(current.point(), current.direction()) {
                            current = dp;
                            if i >= min - 1 && current.point() == to {
                                return Some(AttackInfo::Direction(*d));
                            }
                        } else {
                            break;
                        }
                    }
                }
                None
            }
            _ => Some(AttackInfo::Point(*to)),
        }
    }
}

fn check_normal_unit_can_act<D: Direction>(game: &Game<D>, at: &Point, unload_index: Option<u8>) -> Result<(), CommandError> {
    if !game.has_vision_at(Some(game.current_player().team), at) {
        return Err(CommandError::NoVision);
    }
    let unit = game.get_map().get_unit(&at).ok_or(CommandError::MissingUnit)?;
    let unit: &dyn NormalUnitTrait<D> = if let Some(index) = unload_index {
        unit.get_boarded().get(index as usize).ok_or(CommandError::MissingBoardedUnit)?.as_trait()
    } else {
        unit.as_normal_trait().ok_or(CommandError::UnitTypeWrong)?
    };
    if &game.current_player().owner_id != unit.get_owner() {
        return Err(CommandError::NotYourUnit);
    }
    if !unit.can_act(game.current_player()) {
        return Err(CommandError::UnitCannotMove);
    }
    Ok(())
}

pub fn range_in_layers<D: Direction>(map: &Map<D>, center: &Point, range: usize) -> Vec<HashSet<(Point, D, Option<D>)>> {
    let mut layers: Vec<HashSet<(Point, D, Option<D>)>> = vec![];
    let mut layer = HashSet::new();
    for dp in map.get_neighbors(center, NeighborMode::FollowPipes) {
        layer.insert((dp.point().clone(), dp.direction().clone(), None));
    }
    layers.push(layer);
    while layers.len() < range as usize {
        let mut layer = HashSet::new();
        for (p, dir, dir_change) in layers.last().unwrap() {
            if let Some(dp) = map.get_neighbor(p, dir) {
                let dir_change = match (dp.mirrored(), dir_change) {
                    (_, None) => None,
                    (true, Some(angle)) => Some(angle.opposite_angle()),
                    (false, Some(angle)) => Some(angle.clone()),
                };
                layer.insert((dp.point().clone(), dp.direction().clone(), dir_change));
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
                    let dir = dp.direction().rotate_by(&dir_change.opposite_angle());
                    layer.insert((dp.point().clone(), dir, Some(dir_change)));
                }
            }
        }
        layers.push(layer);
    }
    layers
}

pub enum MovementType {
    Hover,
    Foot,
    Wheel,
    Treads,
    Heli,
    Chess,
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
fn width_search<D: Direction, F: FnMut(&Point, &Vec<Point>) -> bool>(movement_type: &MovementType, max_cost: u8, game: &Game<D>, start: &Point, mut blocked_positions: HashSet<Point>, unit: Option<&dyn NormalUnitTrait<D>>, mut callback: F) {
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
                match (unit, game.get_map().get_unit(neighbor.point())) {
                    (Some(mover), Some(other)) => {
                        if !other.can_be_moved_through(mover, game) {
                            continue;
                        }
                    }
                    (_, _) => {}
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
pub enum UnitAction<D: Direction> {
    Wait,
    Enter,
    Capture,
    Attack(AttackInfo<D>),
    MercenaryPowerSimple(String),
}
impl<D: Direction> fmt::Display for UnitAction<D> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Wait => write!(f, "Wait"),
            Self::Enter => write!(f, "Enter"),
            Self::Capture => write!(f, "Capture"),
            Self::Attack(p) => write!(f, "Attack {:?}", p),
            Self::MercenaryPowerSimple(name) => write!(f, "Activate \"{}\"", name),
        }
    }
}

#[derive(Debug, Clone)]
pub enum AttackInfo<D: Direction> {
    Point(Point),
    Direction(D)
}

pub enum UnitCommand<D: Direction> {
    MoveAttack(Point, Option<u8>, Vec<Point>, AttackInfo<D>),
    MoveCapture(Point, Option<u8>, Vec<Point>),
    MoveWait(Point, Option<u8>, Vec<Point>),
    MoveAboard(Point, Option<u8>, Vec<Point>),
    MoveChess(Point, ChessCommand<D>),
    MercenaryPowerSimple(Point, Option<u8>),
}
impl<D: Direction> UnitCommand<D> {
    fn check_unit_path(game: &Game<D>, start: &Point, unload_index: Option<u8>, path: &Vec<Point>) -> Result<Vec<Point>, CommandError> {
        if !game.get_map().wrapping_logic().pointmap().is_point_valid(start) {
            return Err(CommandError::InvalidPoint(start.clone()));
        }
        for p in path {
            if !game.get_map().wrapping_logic().pointmap().is_point_valid(p) {
                return Err(CommandError::InvalidPoint(p.clone()));
            }
        }
        check_normal_unit_can_act(game, start, unload_index)?;
        let unit = game.get_map().get_unit(&start).ok_or(CommandError::MissingUnit)?;
        let unit: &dyn NormalUnitTrait<D> = if let Some(index) = unload_index {
            unit.get_boarded().get(index as usize).ok_or(CommandError::MissingBoardedUnit)?.as_trait()
        } else {
            unit.as_normal_trait().ok_or(CommandError::UnitTypeWrong)?
        };
        unit.check_path(game, start, path)
    }
    fn check_unit_can_wait_after_path(game: &Game<D>, start: &Point, unload_index: Option<u8>, path: &Vec<Point>) -> Result<Vec<Point>, CommandError> {
        let result = Self::check_unit_path(game, start, unload_index, path);
        if let Some(p) = path.last() {
            if let Some(_) = game.get_map().get_unit(p) {
                if game.has_vision_at(Some(game.current_player().team), p) {
                    return Err(CommandError::InvalidPath);
                }
            }
        }
        result
    }
    fn apply_path_with_event<F: FnOnce(UnitType<D>, Vec<Option<Point>>) -> Event<D>>(handler: &mut EventHandler<D>, start: Point, unload_index: Option<u8>, path_taken: Vec<Point>, f: F) {
        let mut unit = handler.get_map().get_unit(&start).unwrap().clone();
        if let Some(index) = unload_index {
            unit = unit.get_boarded()[index as usize].clone().as_unit();
        }
        if path_taken.len() > 0 {
            let mut event_path:Vec<Option<Point>> = path_taken.iter().map(|p| Some(p.clone())).collect();
            event_path.insert(0, Some(start.clone()));
            let event = f(unit.clone(), event_path);
            handler.add_event(event);
            let team = handler.get_game().current_player().team;
            if Some(team) == unit.get_team(handler.get_game()) {
                let mut vision_changes = HashSet::new();
                for p in &path_taken {
                    for p in unit.get_vision(handler.get_game(), p) {
                        if !handler.get_game().has_vision_at(Some(team), &p) {
                            vision_changes.insert(p);
                        }
                    }
                }
                if vision_changes.len() > 0 {
                    handler.add_event(Event::PureFogChange(Some(team), vision_changes));
                }
            }
        }
    }
    fn apply_path(handler: &mut EventHandler<D>, start: Point, unload_index: Option<u8>, path_taken: Vec<Point>) {
        Self::apply_path_with_event(handler, start, unload_index, path_taken, |unit, path| {
            Event::UnitPath(unload_index, path, unit)
        })
    }
    pub fn calculate_attack(handler: &mut EventHandler<D>, attacker_pos: &Point, target: &AttackInfo<D>, is_counter: bool) -> Result<Vec<Point>, CommandError> {
        let attacker = handler.get_map().get_unit(attacker_pos).and_then(|u| Some(u.clone()));
        let attacker: &dyn NormalUnitTrait<D> = match &attacker {
            Some(UnitType::Normal(unit)) => Ok(unit.as_trait()),
            Some(UnitType::Mercenary(unit)) => Ok(unit.as_trait()),
            Some(UnitType::Chess(_)) => Err(CommandError::UnitTypeWrong),
            Some(UnitType::Structure(_)) => Err(CommandError::UnitTypeWrong),
            None => Err(CommandError::MissingUnit),
        }?;
        let mut potential_counters = vec![];
        let mut recalculate_fog = false;
        let mut charges = HashMap::new();
        for target in attacker.attack_splash(handler.get_map(), attacker_pos, target)? {
            if let Some(defender) = handler.get_map().get_unit(&target) {
                let damage = defender.calculate_attack_damage(handler.get_game(), &target, attacker_pos, attacker, is_counter);
                if let Some(damage) = damage {
                    let hp = defender.get_hp();
                    if !is_counter && defender.get_owner() != Some(attacker.get_owner()) {
                        for (p, _) in handler.get_map().mercenary_influence_at(attacker_pos, Some(attacker.get_owner())) {
                            let change = if &p == attacker_pos {
                                3
                            } else {
                                1
                            };
                            charges.insert(p, charges.get(&p).unwrap_or(&0) + change);
                        }
                    }
                    handler.add_event(Event::UnitHpChange(target.clone(), -(damage.min(hp as u16) as i8), -(damage as i16)));
                    if damage >= hp as u16 {
                        handler.add_event(Event::UnitDeath(target, handler.get_map().get_unit(&target).unwrap().clone()));
                        recalculate_fog = true;
                    } else {
                        potential_counters.push(target);
                    }
                }
            }
        }
        for (p, change) in charges {
            if let Some(UnitType::Mercenary(merc)) = handler.get_map().get_unit(&p) {
                let change = change.min(merc.typ.max_charge() as i16 - change).max(-(merc.charge as i16));
                if change != 0 {
                    handler.add_event(Event::MercenaryCharge(p, change as i8));
                }
            }
        }
        if recalculate_fog {
            handler.recalculate_fog(true);
        }
        Ok(potential_counters)
    }
    pub fn handle_attack(handler: &mut EventHandler<D>, attacker_pos: &Point, target: &AttackInfo<D>) -> Result<(), CommandError> {
        let potential_counters = Self::calculate_attack(handler, attacker_pos, target, false)?;
        // counter attack
        for p in &potential_counters {
            let unit: &dyn NormalUnitTrait<D> = match handler.get_map().get_unit(p) {
                Some(UnitType::Normal(unit)) => unit.as_trait(),
                Some(UnitType::Mercenary(unit)) => unit.as_trait(),
                Some(UnitType::Chess(_)) => continue,
                Some(UnitType::Structure(_)) => continue,
                None => continue,
            };
            if !handler.get_game().has_vision_at(unit.get_team(handler.get_game()), attacker_pos) {
                continue;
            }
            if !unit.is_position_targetable(handler.get_game(), attacker_pos) {
                continue;
            }
            if !unit.attackable_positions(handler.get_map(), &p, false).contains(attacker_pos) {
                continue;
            }
            // todo: if a straight attacker is counter-attacking another straight attacker, it should first try to reverse the direction
            let attack_info = unit.make_attack_info(handler.get_map(), p, attacker_pos).unwrap();
            // this may return an error, but we don't care about that
            Self::calculate_attack(handler, p, &attack_info, true).ok();
        }

        Ok(())
    }
    pub fn convert(self, handler: &mut EventHandler<D>) -> Result<(), CommandError> {
        match self {
            Self::MoveAttack(start, unload_index, path, target) => {
                let intended_end = path.last().unwrap_or(&start).clone();
                let path = Self::check_unit_can_wait_after_path(handler.get_game(), &start, unload_index, &path)?;
                let unit = handler.get_map().get_unit(&start).ok_or(CommandError::MissingUnit)?;
                let unit: &dyn NormalUnitTrait<D> = if let Some(index) = unload_index {
                    unit.get_boarded().get(index as usize).ok_or(CommandError::MissingBoardedUnit)?.as_trait()
                } else {
                    unit.as_normal_trait().ok_or(CommandError::UnitTypeWrong)?
                };
                match &target {
                    AttackInfo::Point(target) => {
                        if !handler.get_map().wrapping_logic().pointmap().is_point_valid(target) {
                            return Err(CommandError::InvalidPoint(target.clone()));
                        }
                        match unit.get_attack_type() {
                            AttackType::Straight(_, _) => return Err(CommandError::InvalidTarget),
                            _ => {}
                        }
                        if !handler.get_game().has_vision_at(Some(handler.get_game().current_player().team), target) {
                            return Err(CommandError::NoVision);
                        }
                        if !unit.is_position_targetable(handler.get_game(), target) {
                            return Err(CommandError::InvalidTarget);
                        }
                        if !unit.attackable_positions(handler.get_map(), &intended_end, path.len() > 0).contains(target) {
                            return Err(CommandError::InvalidTarget);
                        }
                    }
                    AttackInfo::Direction(_) => {
                        match unit.get_attack_type() {
                            AttackType::Straight(_, _) => {},
                            _ => return Err(CommandError::InvalidTarget),
                        }
                    }
                }
                let end = path.last().unwrap_or(&start).clone();
                Self::apply_path(handler, start, unload_index, path);
                // checks fog trap
                if end == intended_end {
                    Self::handle_attack(handler, &end, &target)?;
                }
                if handler.get_game().get_map().get_unit(&end).is_some() {
                    // ensured that the unit didn't die from counter attack
                    handler.add_event(Event::UnitExhaust(end));
                }
            }
            Self::MoveCapture(start, unload_index, path) => {
                let intended_end = path.last().unwrap_or(&start).clone();
                let path = Self::check_unit_can_wait_after_path(handler.get_game(), &start, unload_index, &path)?;
                let end = path.last().unwrap_or(&start).clone();
                Self::apply_path(handler, start, unload_index, path);
                if end == intended_end {
                    let unit = handler.get_map().get_unit(&end).unwrap().as_normal_trait().ok_or(CommandError::UnitTypeWrong)?;
                    let terrain = handler.get_map().get_terrain(&end).unwrap().clone();
                    match &terrain {
                        Terrain::Realty(realty, owner) => {
                            if Some(handler.get_game().get_owning_player(unit.get_owner()).unwrap().team) != owner.and_then(|o| handler.get_game().get_owning_player(&o)).and_then(|p| Some(p.team)) {
                                handler.add_event(Event::TerrainChange(end, terrain.clone(), Terrain::Realty(realty.clone(), Some(*unit.get_owner()))));
                            }
                        }
                        _ => {}
                    }
                }
                handler.add_event(Event::UnitExhaust(end));
            }
            Self::MoveWait(start, unload_index, path) => {
                let path = Self::check_unit_can_wait_after_path(handler.get_game(), &start, unload_index, &path)?;
                let end = path.last().unwrap_or(&start).clone();
                Self::apply_path(handler, start, unload_index, path);
                handler.add_event(Event::UnitExhaust(end));
            }
            Self::MoveAboard(start, unload_index, path) => {
                let intended_end = path.last().unwrap_or(&start).clone();
                let path = Self::check_unit_path(handler.get_game(), &start, unload_index, &path)?;
                if !handler.get_game().has_vision_at(Some(handler.get_game().current_player().team), &intended_end) {
                    return Err(CommandError::NoVision);
                }
                let end = path.last().unwrap_or(&start).clone();
                if end == intended_end {
                    let unit = handler.get_map().get_unit(&start).ok_or(CommandError::MissingUnit)?;
                    if let Some(index) = unload_index {
                        unit.get_boarded().get(index as usize).ok_or(CommandError::MissingBoardedUnit)?.as_trait()
                    } else {
                        unit.as_normal_trait().ok_or(CommandError::UnitTypeWrong)?
                    };
                    let transporter = handler.get_map().get_unit(&end).ok_or(CommandError::MissingUnit)?;
                    if !transporter.boardable_by(unit.clone().as_transportable().ok_or(CommandError::UnitCannotBeBoarded)?) {
                        return Err(CommandError::UnitCannotBeBoarded);
                    }
                    let load_index = transporter.get_boarded().len() as u8;
                    Self::apply_path_with_event(handler, start, unload_index, path, |unit, path| {
                        Event::UnitPathInto(unload_index, path, unit)
                    });
                    handler.add_event(Event::UnitExhaustBoarded(end, load_index));
                } else {
                    // stopped by fog, so the unit doesn't get aboard the transport
                    Self::apply_path(handler, start, unload_index, path);
                    handler.add_event(Event::UnitExhaust(end));
                }
            }
            Self::MoveChess(start, chess_command) => {
                check_chess_unit_can_act(handler.get_game(), &start)?;
                match handler.get_map().get_unit(&start) {
                    Some(UnitType::Chess(unit)) => {
                        let unit = unit.clone();
                        chess_command.convert(start, &unit, handler)?;
                    },
                    _ => return Err(CommandError::UnitTypeWrong),
                }
            }
            Self::MercenaryPowerSimple(pos, unload_index) => {
                if !handler.get_map().wrapping_logic().pointmap().is_point_valid(&pos) {
                    return Err(CommandError::InvalidPoint(pos));
                }
                if !handler.get_game().has_vision_at(Some(handler.get_game().current_player().team), &pos) {
                    return Err(CommandError::NoVision);
                }
                match handler.get_map().get_unit(&pos) {
                    Some(UnitType::Mercenary(merc)) => {
                        if merc.can_use_simple_power(handler.get_game(), &pos) {
                            let change = -(merc.charge as i8);
                            handler.add_event(Event::MercenaryCharge(pos, change));
                            handler.add_event(Event::MercenaryPowerSimple(pos));
                        } else {
                            return Err(CommandError::PowerNotUsable);
                        }
                    },
                    _ => return Err(CommandError::UnitTypeWrong),
                }
            }
        }
        Ok(())
    }
}
