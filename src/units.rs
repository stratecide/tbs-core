pub mod chess;
pub mod structures;
pub mod mercenary;
pub mod normal_units;
pub mod commands;
pub mod movement;
pub mod combat;

use std::collections::{HashSet, HashMap};

use interfaces::game_interface::ClientPerspective;
use zipper::*;
use zipper::zipper_derive::*;

use crate::commanders::Commander;
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
use self::movement::*;
use self::combat::*;
use self::commands::*;

pub type Hp = U8<100>;

#[derive(Debug, PartialEq, Clone, Zippable)]
#[zippable(bits = 3)]
pub enum UnitType<D: Direction> {
    Normal(NormalUnit),
    Chess(ChessUnit::<D>),
    Structure(Structure::<D>),
}
impl<D: Direction> UnitType<D> {
    pub fn normal(typ: NormalUnits, owner: Owner) -> Self {
        Self::Normal(NormalUnit::new_instance(typ, owner))
    }
    pub fn chess(typ: ChessUnits<D>, owner: Owner) -> Self {
        Self::Chess(ChessUnit::new_instance(typ, owner))
    }

    pub fn cast_normal(&self) -> Option<NormalUnit> {
        match self {
            Self::Normal(unit) => Some(unit.clone()),
            _ => None
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Normal(unit) => unit.typ.name(),
            Self::Chess(unit) => unit.typ.name(),
            Self::Structure(unit) => unit.typ.name(),
        }
    }
    pub fn get_owner(&self) -> Option<Owner> {
        match self {
            Self::Normal(unit) => Some(unit.owner),
            Self::Chess(unit) => Some(unit.owner),
            Self::Structure(unit) => unit.get_owner(),
        }
    }
    pub fn get_team(&self, game: &Game<D>) -> ClientPerspective {
        game.get_team(self.get_owner())
    }
    pub fn get_hp(&self) -> u8 {
        *match self {
            Self::Normal(unit) => unit.data.hp,
            Self::Chess(unit) => unit.hp,
            Self::Structure(unit) => unit.hp,
        }
    }
    pub fn set_hp(&mut self, hp: u8) {
        let hp = hp.min(100).try_into().unwrap();
        match self {
            Self::Normal(unit) => unit.data.hp = hp,
            Self::Chess(unit) => unit.hp = hp,
            Self::Structure(unit) => unit.hp = hp,
        }
    }
    pub fn is_exhausted(&self) -> bool {
        match self {
            Self::Normal(unit) => unit.data.exhausted,
            Self::Chess(unit) => unit.exhausted,
            Self::Structure(_) => false,
        }
    }
    pub fn set_exhausted(&mut self, exhausted: bool) {
        match self {
            Self::Normal(unit) => unit.data.exhausted = exhausted,
            Self::Chess(unit) => unit.exhausted = exhausted,
            Self::Structure(struc) => struc.exhausted = exhausted,
        }
    }

    pub fn can_act(&self, player: &Player) -> bool {
        match self {
            Self::Normal(unit) => unit.can_act(player),
            Self::Chess(unit) => return !unit.exhausted && unit.owner == player.owner_id,
            Self::Structure(structure) => return structure.can_act(player),
        }
    }

    pub fn get_boarded(&self) -> Vec<NormalUnit> {
        match self {
            Self::Normal(unit) => unit.get_boarded(),
            Self::Chess(_) => vec![],
            Self::Structure(struc) => struc.get_boarded(),
        }
    }

    pub fn get_boarded_mut(&mut self) -> Vec<&mut UnitData> {
        match self {
            Self::Normal(unit) => unit.get_boarded_mut(),
            Self::Chess(_) => vec![],
            Self::Structure(struc) => struc.get_boarded_mut(),
        }
    }

    pub fn unboard(&mut self, index: u8) {
        match self {
            Self::Normal(unit) => unit.unboard(index),
            Self::Structure(unit) => unit.unboard(index),
            _ => {}
        }
    }

    pub fn boardable_by(&self, unit: &NormalUnit) -> bool {
        if self.get_owner() != Some(unit.get_owner()) {
            return false;
        }
        let boarded_count = self.get_boarded().len() as u8;
        match self {
            Self::Normal(u) => boarded_count < u.typ.transport_capacity() && u.typ.could_transport(&unit.typ),
            Self::Structure(u) => boarded_count < u.typ.transport_capacity() && u.typ.could_transport(&unit.typ),
            _ => false,
        }
    }
    
    pub fn board(&mut self, index: u8, unit: NormalUnit) {
        match self {
            Self::Normal(u) => u.board(index, unit),
            Self::Structure(u) => u.board(index, unit),
            _ => {}
        }
    }

    pub fn movable_positions(&self, game: &Game<D>, path_so_far: &Path<D>) -> HashSet<Point> {
        match self {
            Self::Normal(unit) => unit.movable_positions(game, path_so_far),
            Self::Chess(unit) => unit.movable_positions(game, path_so_far),
            Self::Structure(_) => HashSet::new(),
        }
    }
    pub fn shortest_path_to(&self, game: &Game<D>, path_so_far: &Path<D>, goal: Point) -> Option<Path<D>> {
        match self {
            Self::Normal(unit) => unit.shortest_path_to(game, path_so_far, goal),
            Self::Chess(unit) => unit.shortest_path_to(game, path_so_far, goal),
            Self::Structure(_) => None,
        }
    }
    pub fn shortest_path_to_attack(&self, game: &Game<D>, path_so_far: &Path<D>, goal: Point) -> Option<Path<D>> {
        match self {
            Self::Normal(unit) => unit.shortest_path_to_attack(game, path_so_far, goal),
            Self::Chess(unit) => unit.shortest_path_to_attack(game, path_so_far, goal),
            Self::Structure(_) => None,
        }
    }
    pub fn options_after_path(&self, game: &Game<D>, path: &Path<D>) -> Vec<UnitAction<D>> {
        match self {
            Self::Normal(unit) => unit.options_after_path(game, path),
            Self::Chess(unit) => unit.options_after_path(game, path),
            Self::Structure(structure) => {
                structure.available_options(game)
            },
        }
    }
    pub fn get_armor(&self) -> (ArmorType, f32) {
        match self {
            Self::Normal(unit) => unit.get_armor(),
            Self::Chess(unit) => unit.typ.get_armor(),
            Self::Structure(unit) => unit.typ.get_armor(),
        }
    }
    pub fn killable_by_chess(&self, team: Team, game: &Game<D>) -> bool {
        match self {
            _ => self.get_team(game) != ClientPerspective::Team(*team),
        }
    }
    pub fn can_be_moved_through(&self, by: &NormalUnit, game: &Game<D>) -> bool {
        match self {
            Self::Normal(_) => by.has_stealth() || self.get_team(game) == by.get_team(game),
            Self::Chess(_) => false,
            Self::Structure(_) => false,
        }
    }
    pub fn calculate_attack_damage(&self, game: &Game<D>, pos: Point, attacker_pos: Point, attacker: &NormalUnit, is_counter: bool) -> Option<(WeaponType, u16)> {
        let (armor_type, defense) = self.get_armor();
        let terrain_defense = if let Some(t) = game.get_map().get_terrain(pos) {
            1. + t.defense_bonus(self)
        } else {
            1.
        };

        let mut defense_bonus = 1.;
        if let Some(owner) = self.get_owner().and_then(|owner| game.get_owning_player(owner)) {
            defense_bonus += owner.commander.defense_bonus(game, self, is_counter);
        }
        for (p, merc) in game.get_map().mercenary_influence_at(pos, self.get_owner()) {
            if p != pos {
                defense_bonus += merc.defense_bonus(self, is_counter);
            }
        }
        let defense_bonus = defense_bonus; // to make sure it's not updated in the for-loop on accident

        let mut highest_damage: f32 = 0.;
        let mut used_weapon = None;
        for (weapon, attack) in attacker.get_weapons() {
            if let Some(factor) = weapon.damage_factor(&armor_type) {
                let mut attack_bonus = 1.;
                attack_bonus += game.get_owning_player(attacker.get_owner()).unwrap().commander.attack_bonus(game, attacker, is_counter);
                for (p, merc) in game.get_map().mercenary_influence_at(attacker_pos, Some(attacker.get_owner())) {
                    // merc shouldn't be buffed twice
                    if p != attacker_pos {
                        attack_bonus += merc.attack_bonus(attacker, is_counter);
                    }
                }
                let damage = attacker.get_hp() as f32 * attack * attack_bonus * factor / defense / defense_bonus / terrain_defense;
                if damage > highest_damage {
                    highest_damage = damage;
                    used_weapon = Some(weapon);
                }
            }
        }
        used_weapon.and_then(|weapon| Some((weapon, highest_damage.ceil() as u16)))
    }
    fn true_vision_range(&self, _game: &Game<D>, _pos: Point) -> usize {
        1
    }
    fn vision_range(&self, _game: &Game<D>, _pos: Point) -> usize {
        2
    }
    pub fn get_vision(&self, game: &Game<D>, pos: Point) -> HashSet<Point> {
        match self {
            Self::Chess(unit) => unit.get_vision(game, pos),
            _ => {
                let mut result = HashSet::new();
                result.insert(pos.clone());
                let layers = game.get_map().range_in_layers(pos, self.vision_range(game, pos));
                for (i, layer) in layers.into_iter().enumerate() {
                    for (p, _, _) in layer {
                        if i < self.true_vision_range(game, pos) || !game.get_map().get_terrain(p).unwrap().requires_true_sight() {
                            result.insert(p);
                        }
                    }
                }
                result
            }
        }
    }
    pub fn attackable_positions(&self, game: &Game<D>, position: Point, moved: bool) -> HashSet<Point> {
        match self {
            Self::Normal(u) => u.attackable_positions(game, position, moved),
            Self::Chess(u) => u.attackable_positions(game, position, moved),
            Self::Structure(u) => u.attackable_positions(game, position, moved),
        }
    }
    pub fn can_pull(&self) -> bool {
        match self {
            Self::Normal(unit) => unit.can_pull(),
            Self::Chess(_) => false,
            Self::Structure(_) => false,
        }
    }
    pub fn can_be_pulled(&self, _map: &Map<D>, _pos: Point) -> bool {
        true
    }
    pub fn can_attack_unit(&self, game: &Game<D>, target: &UnitType<D>) -> bool {
        match self {
            Self::Normal(unit) => unit.can_attack_unit(game, target),
            _ => false
        }
    }
    pub fn threatens(&self, game: &Game<D>, target: &UnitType<D>) -> bool {
        self.get_team(game) != target.get_team(game) && match self {
            Self::Normal(unit) => unit.threatens(game, target),
            Self::Chess(unit) => unit.threatens(game, target),
            Self::Structure(_unit) => false,
        }
    }
    pub fn make_attack_info(&self, game: &Game<D>, pos: Point, target: Point) -> Option<AttackInfo<D>> {
        match self {
            Self::Normal(unit) => unit.make_attack_info(game, pos, target),
            _ => None
        }
    }
    pub fn fog_replacement(&self) -> Option<Self> {
        None
    }
    pub fn type_value(&self) -> u16 {
        match self {
            Self::Normal(unit) => unit.typ.value(),
            Self::Chess(unit) => unit.typ.value(),
            Self::Structure(structure) => structure.typ.value(),
        }
    }
    pub fn value(&self, game: &Game<D>, _co: &Commander) -> usize {
        (match self {
            Self::Normal(unit) => unit.value(game),
            Self::Chess(unit) => unit.typ.value(),
            Self::Structure(structure) => structure.typ.value(),
        }) as usize * self.get_hp() as usize / 100
    }
    pub fn update_used_mercs(&self, mercs: &mut HashSet<MercenaryOption>) {
        for boarded in self.get_boarded() {
            boarded.update_used_mercs(mercs);
        }
        match self {
            Self::Normal(unit) => {
                unit.update_used_mercs(mercs)
            }
            _ => {}
        }
    }
    pub fn insert_drone_ids(&self, existing_ids: &mut HashSet<u16>) {
        match self {
            Self::Normal(unit) => unit.typ.insert_drone_ids(existing_ids),
            Self::Structure(_structure) => (), // TODO: drone tower
            _ => (),
        }
    }
}


