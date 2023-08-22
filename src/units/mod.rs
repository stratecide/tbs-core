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
use crate::game::fog::FogIntensity;
use crate::game::fog::FogSetting;
use crate::player::*;
use crate::map::direction::Direction;
use crate::map::point::Point;
use crate::game::game::Game;
use crate::map::map::Map;
use crate::terrain::Terrain;

use self::chess::*;
use self::structures::*;
use self::mercenary::*;
use self::normal_units::*;
use self::movement::*;
use self::combat::*;
use self::commands::*;

pub type Hp = U<100>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum UnitVisibility {
    Stealth,
    Normal,
    AlwaysVisible,
}

#[derive(Debug, PartialEq, Eq, Clone, Zippable, Hash)]
#[zippable(bits = 3)]
pub enum UnitType<D: Direction> {
    Normal(NormalUnit),
    Chess(ChessUnit<D>),
    Structure(Structure<D>),
    Unknown, // half-hidden unit due to light fog
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
            Self::Unknown => "???",
        }
    }

    pub fn get_owner(&self) -> Option<Owner> {
        match self {
            Self::Normal(unit) => Some(unit.owner),
            Self::Chess(unit) => Some(unit.owner),
            Self::Structure(unit) => unit.get_owner(),
            Self::Unknown => None,
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
            Self::Unknown => return 100,
        } as u8
    }

    pub fn set_hp(&mut self, hp: u8) {
        let hp = hp.min(100).into();
        match self {
            Self::Normal(unit) => unit.data.hp = hp,
            Self::Chess(unit) => unit.hp = hp,
            Self::Structure(unit) => unit.hp = hp,
            Self::Unknown => (),
        }
    }

    pub fn is_exhausted(&self) -> bool {
        match self {
            Self::Normal(unit) => unit.data.exhausted,
            Self::Chess(unit) => unit.exhausted,
            Self::Structure(struc) => struc.exhausted,
            Self::Unknown => false,
        }
    }

    pub fn set_exhausted(&mut self, exhausted: bool) {
        match self {
            Self::Normal(unit) => unit.data.exhausted = exhausted,
            Self::Chess(unit) => unit.exhausted = exhausted,
            Self::Structure(struc) => struc.exhausted = exhausted,
            Self::Unknown => (),
        }
    }

    pub fn can_capture(&self) -> bool {
        match self {
            Self::Normal(unit) => unit.can_capture(),
            _ => false,
        }
    }

    pub fn is_capturing(&self) -> bool {
        match self {
            Self::Normal(unit) => unit.action_status == UnitActionStatus::Capturing,
            _ => false,
        }
    }

    pub fn can_act(&self, player: Owner) -> bool {
        match self {
            Self::Normal(unit) => unit.can_act(player),
            Self::Chess(unit) => return !unit.exhausted && unit.owner == player,
            Self::Structure(structure) => return structure.can_act(player),
            Self::Unknown => false,
        }
    }

    pub fn get_boarded(&self) -> Vec<NormalUnit> {
        match self {
            Self::Normal(unit) => unit.get_boarded(),
            Self::Chess(_) => Vec::new(),
            Self::Structure(struc) => struc.get_boarded(),
            Self::Unknown => Vec::new(),
        }
    }

    pub fn get_boarded_mut(&mut self) -> Vec<&mut UnitData> {
        match self {
            Self::Normal(unit) => unit.get_boarded_mut(),
            Self::Chess(_) => Vec::new(),
            Self::Structure(struc) => struc.get_boarded_mut(),
            Self::Unknown => Vec::new(),
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

    pub fn transformed_by_movement(&self, map: &Map<D>, from: Point, to: Point) -> Option<Self> {
        match self {
            UnitType::Normal(u @ NormalUnit { typ: NormalUnits::Hovercraft(on_sea), .. }) => {
                let prev_terrain = map.get_terrain(from).unwrap();
                let movement_type = u.get_movement(prev_terrain).0;
                let terrain = map.get_terrain(to).unwrap();
                let movement_type2 = terrain.update_movement_type(movement_type, prev_terrain).unwrap();
                let on_sea2 = movement_type2 != MovementType::Hover(HoverMode::Land);
                if *on_sea != on_sea2 {
                    let mut new = u.clone();
                    new.typ = NormalUnits::Hovercraft(on_sea2);
                    Some(new.as_unit())
                } else {
                    None
                }
            }
            _ => None
        }
    }

    pub fn movable_positions(&self, game: &Game<D>, path_so_far: &Path<D>) -> HashSet<Point> {
        movement_area_game(game, self, path_so_far, 1)
        .keys()
        .cloned()
        .collect()
    }

    pub fn shortest_path_to(&self, game: &Game<D>, path_so_far: &Path<D>, goal: Point) -> Option<Path<D>> {
        search_path(game, self, path_so_far, None, |_path, p, can_stop_here| {
            if goal == p {
                PathSearchFeedback::Found
            } else if can_stop_here {
                PathSearchFeedback::Continue
            } else {
                PathSearchFeedback::ContinueWithoutStopping
            }
        })
    }

    pub fn shortest_path_to_attack(&self, game: &Game<D>, path_so_far: &Path<D>, goal: Point) -> Option<Path<D>> {
        match self {
            Self::Normal(unit) => unit.shortest_path_to_attack(game, path_so_far, goal),
            Self::Chess(_) => self.shortest_path_to(game, path_so_far, goal),
            Self::Structure(_) => None,
            Self::Unknown => None,
        }
    }

    pub fn options_after_path(&self, game: &Game<D>, path: &Path<D>) -> Vec<UnitAction<D>> {
        match self {
            Self::Normal(unit) => unit.options_after_path(game, path),
            Self::Chess(unit) => unit.options_after_path(game, path),
            Self::Structure(structure) => {
                structure.available_options(game)
            },
            Self::Unknown => Vec::new(),
        }
    }

    pub fn get_armor(&self) -> (ArmorType, f32) {
        match self {
            Self::Normal(unit) => unit.get_armor(),
            Self::Chess(unit) => unit.typ.get_armor(),
            Self::Structure(unit) => unit.typ.get_armor(),
            Self::Unknown => (ArmorType::Unknown, 1.0),
        }
    }

    pub fn killable_by_chess(&self, team: Team, game: &Game<D>) -> bool {
        match self {
            _ => self.get_team(game) != ClientPerspective::Team(*team as u8),
        }
    }

    pub fn can_be_moved_through(&self, by: &NormalUnit, game: &Game<D>) -> bool {
        match self {
            Self::Unknown |
            Self::Normal(_) => by.has_stealth() && !game.is_foggy() || self.get_team(game) == by.get_team(game),
            Self::Chess(_) => false,
            Self::Structure(_) => false,
        }
    }

    pub fn can_be_taken_by_chess(&self, game: &Game<D>, attacking_owner: Owner) -> bool {
        match self {
            Self::Normal(_) => self.get_team(game) != game.get_team(Some(attacking_owner)),
            Self::Chess(_) => self.get_team(game) != game.get_team(Some(attacking_owner)),
            Self::Structure(_) => false,
            Self::Unknown => true,
        }
    }

    // set path to None if this is a counter-attack
    pub fn calculate_attack_damage(&self, game: &Game<D>, pos: Point, attacker_pos: Point, attacker: &NormalUnit, path: Option<&Path<D>>) -> Option<(WeaponType, u16)> {
        let is_counter = path.is_none();
        let (armor_type, defense) = self.get_armor();
        let terrain = game.get_map().get_terrain(pos).unwrap();
        let terrain_defense = 1. + terrain.defense_bonus(self);
        let in_water = terrain.is_water();
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
        for (weapon, mut attack) in attacker.get_weapons() {
            if let Some(path) = path {
                attack *= attacker.attack_factor_from_path(game, path);
            } else {
                attack *= attacker.attack_factor_from_counter(game);
            }
            if let Some(factor) = weapon.damage_factor(&armor_type, in_water) {
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

    pub fn has_vision_from_path_intermediates(&self) -> bool {
        match self {
            Self::Normal(_) => true,
            _ => false
        }
    }

    fn true_vision_range(&self, _game: &Game<D>, _pos: Point) -> usize {
        1
    }

    fn vision_range(&self, game: &Game<D>, pos: Point) -> usize {
        match self {
            Self::Normal(unit) => unit.vision_range(game, pos),
            Self::Chess(_) => 0,
            Self::Structure(_) => 0,
            Self::Unknown => 0,
        }
    }

    pub fn get_vision(&self, game: &Game<D>, pos: Point) -> HashMap<Point, FogIntensity> {
        match self {
            Self::Chess(unit) => unit.get_vision(game, pos),
            Self::Unknown => HashMap::new(),
            _ => {
                let mut result = HashMap::new();
                result.insert(pos, FogIntensity::TrueSight);
                let vision_range = self.vision_range(game, pos);
                let normal_range = match game.get_fog_setting() {
                    FogSetting::ExtraDark(_) => 0,
                    FogSetting::Fade1(_) => 1.max(vision_range) - 1,
                    FogSetting::Fade2(_) => 2.max(vision_range) - 2,
                    _ => vision_range
                };
                let layers = game.get_map().range_in_layers(pos, vision_range);
                for (i, layer) in layers.into_iter().enumerate() {
                    for p in layer {
                        let vision = if i < self.true_vision_range(game, pos) {
                            FogIntensity::TrueSight
                        } else if i < normal_range {
                            FogIntensity::NormalVision
                        } else {
                            FogIntensity::Light
                        };
                        result.insert(p, vision.min(result.get(&p).cloned().unwrap_or(FogIntensity::Dark)));
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
            Self::Unknown => HashSet::new(),
        }
    }

    pub fn can_pull(&self) -> bool {
        match self {
            Self::Normal(unit) => unit.can_pull(),
            Self::Chess(_) => false,
            Self::Structure(_) => false,
            Self::Unknown => false,
        }
    }

    pub fn can_be_pulled(&self, _map: &Map<D>, _pos: Point) -> bool {
        true
    }

    pub fn can_attack_unit(&self, game: &Game<D>, target: &UnitType<D>, target_pos: Point) -> bool {
        match self {
            Self::Normal(unit) => unit.can_attack_unit(game, target, target_pos),
            _ => false
        }
    }

    pub fn threatens(&self, game: &Game<D>, target: &UnitType<D>, target_pos: Point) -> bool {
        self.get_team(game) != target.get_team(game) && match self {
            Self::Normal(unit) => unit.threatens(game, target, target_pos),
            Self::Chess(unit) => unit.threatens(game, target),
            Self::Structure(_unit) => false, // TODO: should return yes if in range
            Self::Unknown => false,
        }
    }

    pub fn make_attack_info(&self, game: &Game<D>, pos: Point, target: Point) -> Option<AttackInfo<D>> {
        match self {
            Self::Normal(unit) => unit.make_attack_info(game, pos, target),
            _ => None
        }
    }

    pub fn fog_replacement(&self, terrain: &Terrain<D>, intensity: FogIntensity) -> Option<Self> {
        match self {
            Self::Structure(struc) => struc.fog_replacement(intensity).and_then(|s| Some(Self::Structure(s))),
            Self::Normal(_unit) => {
                match intensity {
                    FogIntensity::TrueSight => Some(self.clone()),
                    FogIntensity::NormalVision => {
                        if match self.visibility() {
                            UnitVisibility::Stealth => false,
                            UnitVisibility::Normal => !terrain.hides_unit(self),
                            UnitVisibility::AlwaysVisible => true,
                        } {
                            Some(self.clone())
                        } else {
                            None
                        }
                    }
                    FogIntensity::Light => {
                        match self.visibility() {
                            UnitVisibility::Stealth => None,
                            UnitVisibility::Normal => {
                                if terrain.hides_unit(self) {
                                    None
                                } else {
                                    Some(UnitType::Unknown)
                                }
                            }
                            UnitVisibility::AlwaysVisible => Some(self.clone()),
                        }
                    }
                    FogIntensity::Dark => {
                        // normal units don't have AlwaysVisible so far, but doesn't hurt
                        if self.visibility() == UnitVisibility::AlwaysVisible {
                            Some(self.clone())
                        } else {
                            None
                        }
                    }
                }
            }
            Self::Chess(_unit) => {
                match intensity {
                    FogIntensity::NormalVision |
                    FogIntensity::TrueSight => Some(self.clone()),
                    FogIntensity::Light => Some(UnitType::Unknown),
                    FogIntensity::Dark => None,
                }
            }
            Self::Unknown => match intensity {
                FogIntensity::Dark => None,
                _ => Some(self.clone()),
            }
        }
    }
    pub fn visibility(&self) -> UnitVisibility {
        match self {
            Self::Normal(unit) => {
                if unit.has_stealth() {
                    UnitVisibility::Stealth
                } else {
                    UnitVisibility::Normal
                }
            }
            Self::Chess(_) => UnitVisibility::Normal,
            Self::Structure(_) => UnitVisibility::AlwaysVisible,
            Self::Unknown => UnitVisibility::Normal,
        }
    }

    pub fn type_value(&self) -> u16 {
        match self {
            Self::Normal(unit) => unit.typ.value(),
            Self::Chess(unit) => unit.typ.value(),
            Self::Structure(structure) => structure.typ.value(),
            Self::Unknown => 0,
        }
    }

    pub fn value(&self, game: &Game<D>, _co: &Commander) -> usize {
        (match self {
            Self::Normal(unit) => unit.value(game),
            Self::Chess(unit) => unit.typ.value(),
            Self::Structure(structure) => structure.typ.value(),
            Self::Unknown => return 0,
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


