pub mod chess;
pub mod structures;
pub mod mercenary;
pub mod normal_trait;
pub mod normal_units;
pub mod commands;
pub mod transportable;
pub mod movement;
pub mod combat;

use std::collections::{HashSet, HashMap};

use zipper::*;
use zipper::zipper_derive::*;

use crate::game::events::*;
use crate::player::*;
use crate::map::direction::Direction;
use crate::map::point::Point;
use crate::game::game::Game;
use crate::map::map::Map;

use self::chess::*;
use self::structures::*;
use self::mercenary::*;
use self::normal_units::*;
use self::normal_trait::*;
use self::movement::*;
use self::combat::*;
use self::transportable::*;
use self::commands::*;

pub type Hp = U8<100>;

#[derive(Debug, PartialEq, Clone, Zippable)]
#[zippable(bits = 3)]
pub enum UnitType<D: Direction> {
    Normal(NormalUnit),
    Mercenary(Mercenary),
    Chess(ChessUnit),
    Structure(Structure::<D>),
}
impl<D: Direction> UnitType<D> {
    pub fn as_normal_trait(&self) -> Option<&dyn NormalUnitTrait<D>> {
        match self {
            Self::Normal(unit) => Some(unit.as_trait()),
            Self::Mercenary(merc) => Some(merc.as_trait()),
            _ => None,
        }
    }
    pub fn as_transportable(&self) -> Option<TransportableTypes> {
        match self {
            Self::Normal(u) => Some(TransportableTypes::Normal(u.clone())),
            Self::Mercenary(u) => Some(TransportableTypes::Mercenary(u.clone())),
            _ => None,
        }
    }
    pub fn normal(typ: NormalUnits, owner: Owner) -> Self {
        Self::Normal(NormalUnit::new_instance(typ, owner))
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
        game.get_team(self.get_owner())
    }
    pub fn get_hp(&self) -> u8 {
        *match self {
            Self::Normal(unit) => unit.hp,
            Self::Mercenary(unit) => unit.unit.hp,
            Self::Chess(unit) => unit.hp,
            Self::Structure(unit) => unit.hp,
        }
    }
    pub fn set_hp(&mut self, hp: u8) {
        let hp = hp.min(100).try_into().unwrap();
        match self {
            Self::Normal(unit) => unit.hp = hp,
            Self::Mercenary(unit) => unit.unit.hp = hp,
            Self::Chess(unit) => unit.hp = hp,
            Self::Structure(unit) => unit.hp = hp,
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
    pub fn boardable_by(&self, unit: &TransportableTypes) -> bool {
        if self.get_owner() != Some(unit.get_owner()) {
            return false;
        }
        let boarded_count = self.get_boarded().len() as u8;
        let normal_typ = match unit {
            TransportableTypes::Normal(u) => &u.typ,
            TransportableTypes::Mercenary(m) => &m.unit.typ,
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
    pub fn shortest_path_to_attack(&self, game: &Game<D>, start: &Point, path_so_far: &Vec<Point>, goal: &Point) -> Option<Vec<Point>> {
        match self {
            Self::Normal(unit) => unit.shortest_path_to_attack(game, start, path_so_far, goal),
            Self::Mercenary(unit) => unit.shortest_path_to_attack(game, start, path_so_far, goal),
            Self::Chess(_) => None,
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
    pub fn calculate_attack_damage(&self, game: &Game<D>, pos: &Point, attacker_pos: &Point, attacker: &dyn NormalUnitTrait<D>, is_counter: bool) -> Option<(WeaponType, u16)> {
        let (armor_type, defense) = self.get_armor();
        let terrain_defense = if let Some(t) = game.get_map().get_terrain(pos) {
            t.defense(self)
        } else {
            1.
        };
        let mut highest_damage: f32 = 0.;
        let mut used_weapon = None;
        for (weapon, attack) in attacker.get_weapons() {
            if let Some(factor) = weapon.damage_factor(&armor_type) {
                let mut damage = attacker.get_hp() as f32 * attack * factor / defense / terrain_defense;
                damage *= game.get_owning_player(attacker.get_owner()).unwrap().commander.attack_bonus(game, attacker, is_counter);
                if let Some(owner) = self.get_owner().and_then(|owner| game.get_owning_player(owner)) {
                    damage /= owner.commander.defense_bonus(game, self, is_counter);
                }
                for (_, merc) in game.get_map().mercenary_influence_at(attacker_pos, Some(attacker.get_owner())) {
                    damage *= merc.attack_bonus(attacker, is_counter);
                }
                for (_, merc) in game.get_map().mercenary_influence_at(pos, self.get_owner()) {
                    damage /= merc.defense_bonus(self, is_counter);
                }
                if damage > highest_damage {
                    highest_damage = damage;
                    used_weapon = Some(weapon);
                }
            }
        }
        used_weapon.and_then(|weapon| Some((weapon, highest_damage.ceil() as u16)))
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
                let layers = game.get_map().range_in_layers(pos, self.vision_range(game, pos));
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
    pub fn can_pull(&self) -> bool {
        match self {
            Self::Normal(unit) => unit.can_pull(),
            Self::Mercenary(merc) => merc.unit.can_pull(),
            Self::Chess(_) => false,
            Self::Structure(_) => false,
        }
    }
    pub fn can_be_pulled(&self, _map: &Map<D>, _pos: &Point) -> bool {
        true
    }
    pub fn can_attack_unit_type(&self, game: &Game<D>, target: &UnitType<D>) -> bool {
        if let Some(unit) = self.as_normal_trait() {
            unit.can_attack_unit_type(game, target)
        } else {
            false
        }
    }
    pub fn make_attack_info(&self, game: &Game<D>, pos: &Point, target: &Point) -> Option<AttackInfo<D>> {
        if let Some(unit) = self.as_normal_trait() {
            unit.make_attack_info(game, pos, target)
        } else {
            None
        }
    }
    pub fn fog_replacement(&self) -> Option<Self> {
        None
    }
    pub fn type_value(&self) -> u16 {
        match self {
            UnitType::Normal(unit) => unit.typ.value(),
            UnitType::Mercenary(merc) => merc.unit.typ.value(),
            UnitType::Chess(unit) => unit.typ.value(),
            UnitType::Structure(structure) => structure.typ.value(),
        }
    }
}


