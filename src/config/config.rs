use std::collections::HashMap;
use std::collections::HashSet;

use interfaces::ConfigInterface;
use num_rational::Rational32;

use crate::commander::Commander;
use crate::game::fog::VisionMode;
use crate::commander::commander_type::CommanderType;
use crate::game::game::Game;
use crate::map::direction::Direction;
use crate::map::point::Point;
use crate::script::attack::AttackScript;
use crate::script::kill::KillScript;
use crate::script::unit::UnitScript;
use crate::terrain::AmphibiousTyping;
use crate::terrain::ExtraMovementOptions;
use crate::terrain::TerrainType;
use crate::terrain::attributes::TerrainAttributeKey;
use crate::units::combat::*;
use crate::units::movement::MovementType;
use crate::units::unit::Unit;
use crate::units::unit_types::UnitType;
use crate::units::attributes::*;
use crate::units::hero::*;

use super::hero_type_config::HeroTypeConfig;
use super::commander_power_config::CommanderPowerConfig;
use super::commander_type_config::CommanderTypeConfig;
use super::commander_unit_config::CommanderPowerUnitConfig;
use super::movement_type_config::MovementPattern;
use super::terrain_type_config::TerrainTypeConfig;
use super::unit_filter::*;
use super::unit_type_config::UnitTypeConfig;

const DEFAULT_SPLASH: [Rational32; 1] = [Rational32::new_raw(1, 1)];

pub struct Config {
    pub(super) name: String,
    // units
    pub(super) unit_types: Vec<UnitType>,
    pub(super) units: HashMap<UnitType, UnitTypeConfig>,
    pub(super) unit_transports: HashMap<UnitType, Vec<UnitType>>,
    pub(super) unit_attributes: HashMap<UnitType, Vec<AttributeKey>>,
    pub(super) unit_hidden_attributes: HashMap<UnitType, Vec<AttributeKey>>,
    pub(super) attack_damage: HashMap<UnitType, HashMap<UnitType, u16>>,
    // heroes
    pub(super) hero_types: Vec<HeroType>,
    pub(super) heroes: HashMap<HeroType, HeroTypeConfig>,
    pub(super) hero_units: HashMap<HeroType, HashSet<UnitType>>,
    pub(super) hero_powered_units: HashMap<HeroType, HashMap<Option<bool>, Vec<CommanderPowerUnitConfig>>>,
    pub(super) max_hero_charge: u8,
    // terrain
    pub(super) terrain_types: Vec<TerrainType>,
    pub(super) terrains: HashMap<TerrainType, TerrainTypeConfig>,
    pub(super) terrain_attributes: HashMap<TerrainType, Vec<TerrainAttributeKey>>,
    pub(super) terrain_hidden_attributes: HashMap<TerrainType, Vec<TerrainAttributeKey>>,
    pub(super) movement_cost: HashMap<TerrainType, HashMap<MovementType, Rational32>>,
    pub(super) attack_bonus: HashMap<TerrainType, HashMap<MovementType, Rational32>>,
    pub(super) defense_bonus: HashMap<TerrainType, HashMap<MovementType, Rational32>>,
    pub(super) build_or_repair: HashMap<TerrainType, Vec<UnitType>>,
    pub(super) max_capture_resistance: u8,
    pub(super) terrain_max_anger: u8,
    pub(super) terrain_max_built_this_turn: u8,
    // commanders
    pub(super) commander_types: Vec<CommanderType>,
    pub(super) commanders: HashMap<CommanderType, CommanderTypeConfig>,
    pub(super) commander_powers: HashMap<CommanderType, Vec<CommanderPowerConfig>>,
    pub(super) default_unit_overrides: Vec<CommanderPowerUnitConfig>,
    pub(super) commander_units: HashMap<CommanderType, HashMap<Option<u8>, Vec<CommanderPowerUnitConfig>>>,
    pub(super) commander_unit_attributes: HashMap<CommanderType, Vec<(UnitTypeFilter, Vec<AttributeKey>, Vec<AttributeKey>)>>,
    pub(super) max_commander_charge: u32,
}

impl ConfigInterface for Config {}

impl Config {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn max_player_count(&self) -> i8 {
        16
    }

    // units

    pub fn unit_count(&self) -> usize {
        self.units.len()
    }

    pub fn unit_types(&self) -> &[UnitType] {
        &self.unit_types
    }

    pub(super) fn unit_config(&self, typ: UnitType) -> &UnitTypeConfig {
        self.units.get(&typ).expect(&format!("Environment doesn't contain unit type {typ:?}"))
    }

    pub fn unit_name(&self, typ: UnitType) -> &str {
        &self.unit_config(typ).name
    }

    pub fn unit_needs_owner(&self, typ: UnitType) -> bool {
        self.unit_config(typ).needs_owner
    }

    pub fn movement_pattern(&self, typ: UnitType) -> MovementPattern {
        self.unit_config(typ).movement_pattern
    }

    pub fn movement_type(&self, typ: UnitType, amphibious: Amphibious) -> MovementType {
        match amphibious {
            Amphibious::OnLand => self.unit_config(typ).movement_type,
            Amphibious::InWater => self.unit_config(typ).water_movement_type.unwrap_or(self.unit_config(typ).movement_type),
        }
    }

    pub fn movement_points(&self, typ: UnitType) -> Rational32 {
        self.unit_config(typ).movement_points
    }

    pub fn has_stealth(&self, typ: UnitType) -> bool {
        self.unit_config(typ).stealthy
    }

    pub fn can_be_moved_through(&self, typ: UnitType) -> bool {
        self.unit_config(typ).can_be_moved_through
    }

    pub fn can_take(&self, typ: UnitType) -> bool {
        self.unit_config(typ).can_take
    }

    pub fn can_be_taken(&self, typ: UnitType) -> bool {
        self.unit_config(typ).can_be_taken
    }

    pub fn weapon(&self, typ: UnitType) -> WeaponType {
        self.unit_config(typ).weapon
    }

    pub fn can_attack(&self, typ: UnitType) -> bool {
        self.attack_damage.contains_key(&typ)
    }

    pub fn can_attack_after_moving(&self, typ: UnitType) -> bool {
        self.unit_config(typ).can_attack_after_moving
    }

    pub fn attack_pattern(&self, typ: UnitType) -> AttackType {
        self.unit_config(typ).attack_pattern
    }

    pub fn attack_targeting(&self, typ: UnitType) -> AttackTargeting {
        self.unit_config(typ).attack_targets
    }

    pub fn base_damage(&self, attacker: UnitType, defender: UnitType) -> Option<u16> {
        self.attack_damage.get(&attacker)?.get(&defender).cloned()
    }

    pub fn splash_damage(&self, typ: UnitType) -> &[Rational32] {
        if self.unit_config(typ).splash_damage.len() == 0 {
            &DEFAULT_SPLASH
        } else {
            &self.unit_config(typ).splash_damage
        }
    }

    pub fn can_build_units(&self, typ: UnitType) -> bool {
        self.unit_config(typ).can_build_units
    }

    pub fn base_cost(&self, typ: UnitType) -> i32 {
        self.unit_config(typ).cost as i32
    }

    pub fn displacement(&self, typ: UnitType) -> Displacement {
        self.unit_config(typ).displacement
    }

    pub fn displacement_distance(&self, typ: UnitType) -> i8 {
        self.unit_config(typ).displacement_distance
    }

    pub fn can_be_displaced(&self, typ: UnitType) -> bool {
        self.unit_config(typ).can_be_displaced
    }

    pub fn vision_mode(&self, typ: UnitType) -> VisionMode {
        self.unit_config(typ).vision_mode
    }

    pub fn vision_range(&self, typ: UnitType) -> usize {
        self.unit_config(typ).vision
    }

    pub(crate) fn unit_specific_attributes(&self, typ: UnitType) -> &[AttributeKey] {
        self.unit_attributes.get(&typ).expect(&format!("Environment doesn't contain unit type {typ:?}"))
    }

    pub(crate) fn unit_specific_hidden_attributes(&self, typ: UnitType) -> &[AttributeKey] {
        self.unit_hidden_attributes.get(&typ).expect(&format!("Environment doesn't contain unit type {typ:?}"))
    }

    pub fn unit_transportable(&self, typ: UnitType) -> &[UnitType] {
        if let Some(transportable) = self.unit_transports.get(&typ) {
            transportable
        } else {
            &[]
        }
    }

    // hero

    pub fn hero_count(&self) -> usize {
        self.heroes.len()
    }

    pub fn hero_types(&self) -> &[HeroType] {
        &self.hero_types
    }

    pub(super) fn hero_config(&self, typ: HeroType) -> &HeroTypeConfig {
        self.heroes.get(&typ).expect(&format!("Environment doesn't contain hero type {typ:?}"))
    }

    pub fn hero_price(&self, typ: HeroType, unit: UnitType) -> Option<i32> {
        if self.hero_units.get(&typ)?.contains(&unit) {
            Some(self.hero_config(typ).price as i32 + 
            (Rational32::from_integer(self.base_cost(unit)) * self.hero_config(typ).relative_price).to_integer())
        } else {
            None
        }
    }

    pub fn max_hero_charge(&self) -> u8 {
        self.max_hero_charge
    }

    pub fn hero_charge(&self, typ: HeroType) -> u8 {
        self.hero_config(typ).charge
    }

    pub fn hero_aura_range(&self, typ: HeroType) -> u8 {
        self.hero_config(typ).aura_range
    }

    pub fn hero_transport_capacity(&self, typ: HeroType) -> u8 {
        self.hero_config(typ).transport_capacity
    }

    pub(super) fn hero_unit_configs<'a, D: Direction>(&'a self, game: &'a Game<D>, hero: HeroType, power: bool, unit: &'a Unit<D>, unit_pos: Point, hero_unit: &'a Unit<D>, hero_pos: Point, other_unit: Option<(&'a Unit<D>, Point)>) -> impl Iterator<Item = &'a CommanderPowerUnitConfig> {
        let mut slices = vec![&self.default_unit_overrides];
        // should always be true
        if let Some(configs) = self.hero_powered_units.get(&hero) {
            if let Some(neutral) = configs.get(&None) {
                slices.push(neutral);
            }
            if let Some(power) = configs.get(&Some(power)) {
                slices.push(power);
            }
        }
        slices.into_iter()
        .flatten()
        .filter(move |config| {
            config.affects.iter().all(|filter| filter.check(self, game, unit, unit_pos, Some((hero_unit, hero_pos)), other_unit))
        })
    }

    pub fn aura_attack_bonus<D: Direction>(&self, game: &Game<D>, unit: &Unit<D>, unit_pos: Point, hero_unit: &Unit<D>, hero_pos: Point, other_unit: &Unit<D>, other_pos: Point, hero: HeroType, power: bool, is_counter: bool) -> Rational32 {
        let mut result = Rational32::from_integer(0);
        for config in self.hero_unit_configs(game, hero, power, unit, unit_pos, hero_unit, hero_pos, Some((other_unit, other_pos))) {
            if is_counter {
                result += config.bonus_counter_attack;
            } else {
                result += config.bonus_attack;
            }
        }
        result
    }

    pub fn aura_defense_bonus<D: Direction>(&self, game: &Game<D>, unit: &Unit<D>, unit_pos: Point, hero_unit: &Unit<D>, hero_pos: Point, other_unit: &Unit<D>, other_pos: Point, hero: HeroType, power: bool, is_counter: bool) -> Rational32 {
        let mut result = Rational32::from_integer(0);
        for config in self.hero_unit_configs(game, hero, power, unit, unit_pos, hero_unit, hero_pos, Some((other_unit, other_pos))) {
            if is_counter {
                result += config.bonus_counter_defense;
            } else {
                result += config.bonus_defense;
            }
        }
        result
    }

    pub fn hero_attribute_overrides<'a, D: Direction>(&'a self, game: &'a Game<D>, unit: &'a Unit<D>, unit_pos: Point, hero_unit: &'a Unit<D>, hero_pos: Point, hero: &'a Hero) -> impl Iterator<Item = &'a AttributeOverride> {
        self.hero_unit_configs(game, hero.typ(), hero.is_power_active(), unit, unit_pos, hero_unit, hero_pos, None)
        .flat_map(|config| {
            &config.build_overrides
        })
    }

    pub fn hero_start_turn_scripts<'a, D: Direction>(&'a self, game: &'a Game<D>, unit: &'a Unit<D>, unit_pos: Point, hero_unit: &'a Unit<D>, hero_pos: Point, hero: &'a Hero) -> impl Iterator<Item = &'a UnitScript> {
        self.hero_unit_configs(game, hero.typ(), hero.is_power_active(), unit, unit_pos, hero_unit, hero_pos, None)
        .flat_map(|config| {
            &config.on_start_turn
        })
    }

    pub fn hero_end_turn_scripts<'a, D: Direction>(&'a self, game: &'a Game<D>, unit: &'a Unit<D>, unit_pos: Point, hero_unit: &'a Unit<D>, hero_pos: Point, hero: &'a Hero) -> impl Iterator<Item = &'a UnitScript> {
        self.hero_unit_configs(game, hero.typ(), hero.is_power_active(), unit, unit_pos, hero_unit, hero_pos, None)
        .flat_map(|config| {
            &config.on_end_turn
        })
    }

    pub fn hero_death_scripts<'a, D: Direction>(&'a self, game: &'a Game<D>, unit: &'a Unit<D>, unit_pos: Point, hero_unit: &'a Unit<D>, hero_pos: Point, hero: &'a Hero) -> impl Iterator<Item = &'a UnitScript> {
        self.hero_unit_configs(game, hero.typ(), hero.is_power_active(), unit, unit_pos, hero_unit, hero_pos, None)
        .flat_map(|config| {
            &config.on_death
        })
    }

    pub fn hero_attack_scripts<'a, D: Direction>(&'a self, game: &'a Game<D>, unit: &'a Unit<D>, unit_pos: Point, hero_unit: &'a Unit<D>, hero_pos: Point, other_unit: &'a Unit<D>, other_pos: Point, hero: &'a Hero) -> impl Iterator<Item = &'a AttackScript> {
        self.hero_unit_configs(game, hero.typ(), hero.is_power_active(), unit, unit_pos, hero_unit, hero_pos, Some((other_unit, other_pos)))
        .flat_map(|config| {
            &config.on_attack
        })
    }

    pub fn hero_kill_scripts<'a, D: Direction>(&'a self, game: &'a Game<D>, unit: &'a Unit<D>, unit_pos: Point, hero_unit: &'a Unit<D>, hero_pos: Point, other_unit: &'a Unit<D>, other_pos: Point, hero: &'a Hero) -> impl Iterator<Item = &'a KillScript> {
        self.hero_unit_configs(game, hero.typ(), hero.is_power_active(), unit, unit_pos, hero_unit, hero_pos, Some((other_unit, other_pos)))
        .flat_map(|config| {
            &config.on_kill
        })
    }

    // terrain

    pub fn terrain_count(&self) -> usize {
        self.terrains.len()
    }

    pub fn terrain_types(&self) -> &[TerrainType] {
        &self.terrain_types
    }

    pub(super) fn terrain_config(&self, typ: TerrainType) -> &TerrainTypeConfig {
        self.terrains.get(&typ).expect(&format!("Environment doesn't contain terrain type {typ:?}"))
    }

    pub fn terrain_name(&self, typ: TerrainType) -> &str {
        &self.terrain_config(typ).name
    }

    pub fn terrain_needs_owner(&self, typ: TerrainType) -> bool {
        self.terrain_config(typ).needs_owner
    }

    pub fn max_capture_resistance(&self) -> u8 {
        self.max_capture_resistance
    }

    pub fn terrain_capture_resistance(&self, typ: TerrainType) -> u8 {
        self.terrain_config(typ).capture_resistance
    }

    pub fn terrain_amphibious(&self, typ: TerrainType) -> Option<AmphibiousTyping> {
        self.terrain_config(typ).update_amphibious
    }

    pub fn terrain_chess(&self, typ: TerrainType) -> bool {
        self.terrain_config(typ).chess
    }

    pub fn terrain_max_built_this_turn(&self) -> u8 {
        self.terrain_max_built_this_turn
    }

    pub fn terrain_max_builds_per_turn(&self, typ: TerrainType) -> u8 {
        self.terrain_config(typ).max_builds_per_turn
    }

    pub fn terrain_max_anger(&self) -> u8 {
        self.terrain_max_anger
    }

    pub fn terrain_anger(&self, typ: TerrainType) -> u8 {
        self.terrain_config(typ).max_anger
    }

    pub fn terrain_path_extra(&self, typ: TerrainType) -> ExtraMovementOptions {
        self.terrain_config(typ).extra_movement_options
    }

    pub fn terrain_movement_cost(&self, typ: TerrainType, movement_type: MovementType) -> Option<Rational32> {
        self.movement_cost.get(&typ)
        .and_then(|map| map.get(&movement_type))
        .cloned()
    }

    pub fn terrain_attack_bonus(&self, typ: TerrainType, movement_type: MovementType) -> Rational32 {
        self.attack_bonus.get(&typ)
        .and_then(|map| map.get(&movement_type))
        .cloned()
        .unwrap_or(Rational32::from_integer(0))
    }

    pub fn terrain_defense_bonus(&self, typ: TerrainType, movement_type: MovementType) -> Rational32 {
        self.defense_bonus.get(&typ)
        .and_then(|map| map.get(&movement_type))
        .cloned()
        .unwrap_or(Rational32::from_integer(0))
    }

    pub fn terrain_vision_range(&self, typ: TerrainType) -> Option<u8> {
        self.terrain_config(typ).vision_range
    }

    pub fn terrain_income_factor(&self, typ: TerrainType) -> i16 {
        self.terrain_config(typ).income_factor
    }

    pub fn terrain_can_build(&self, typ: TerrainType) -> bool {
        self.terrain_config(typ).can_build
    }

    pub fn terrain_can_repair(&self, typ: TerrainType) -> bool {
        self.terrain_config(typ).can_repair
    }

    pub fn terrain_sells_hero(&self, typ: TerrainType) -> bool {
        self.terrain_config(typ).can_sell_hero
    }

    pub fn terrain_build_or_repair(&self, typ: TerrainType) -> &[UnitType] {
        if let Some(units) = self.build_or_repair.get(&typ) {
            &units
        } else {
            &[]
        }
    }

    pub(crate) fn terrain_specific_attributes(&self, typ: TerrainType) -> &[TerrainAttributeKey] {
        self.terrain_attributes.get(&typ).expect(&format!("Environment doesn't contain terrain type {typ:?}"))
    }

    pub(crate) fn terrain_specific_hidden_attributes(&self, typ: TerrainType) -> &[TerrainAttributeKey] {
        self.terrain_hidden_attributes.get(&typ).expect(&format!("Environment doesn't contain terrain type {typ:?}"))
    }

    // commanders

    pub fn commander_count(&self) -> usize {
        self.commanders.len()
    }

    pub fn commander_types(&self) -> &[CommanderType] {
        &self.commander_types
    }

    pub(super) fn commander_config(&self, typ: CommanderType) -> &CommanderTypeConfig {
        self.commanders.get(&typ).expect(&format!("Environment doesn't contain commander type {typ:?}"))
    }

    pub fn commander_name(&self, typ: CommanderType) -> &str {
        &self.commander_config(typ).name
    }

    pub(crate) fn commander_attributes(&self, typ: CommanderType, unit: UnitType) -> &[AttributeKey] {
        if let Some(attributes) = self.commander_unit_attributes.get(&typ) {
            for (filter, attributes, _) in attributes {
                if filter.check(self, unit) {
                    return &attributes;
                }
            }
        }
        &[]
    }

    pub(crate) fn commander_attributes_hidden_by_fog(&self, typ: CommanderType, unit: UnitType) -> &[AttributeKey] {
        if let Some(attributes) = self.commander_unit_attributes.get(&typ) {
            for (filter, _, attributes) in attributes {
                if filter.check(self, unit) {
                    return &attributes;
                }
            }
        }
        &[]
    }

    pub fn max_commander_charge(&self) -> u32 {
        self.max_commander_charge
    }

    pub fn commander_max_charge(&self, typ: CommanderType) -> u32 {
        self.commander_config(typ).max_charge
    }

    pub fn commander_powers(&self, typ: CommanderType) -> &[CommanderPowerConfig] {
        if let Some(powers) = self.commander_powers.get(&typ) {
            powers
        } else {
            &[]
        }
    }

    // commander unit

    pub(super) fn commander_unit_configs<'a, D: Direction>(&'a self, commander: &'a Commander, unit: &'a Unit<D>, game: &'a Game<D>, pos: Point, other_unit: Option<(&'a Unit<D>, Point)>) -> impl Iterator<Item = &'a CommanderPowerUnitConfig> {
        let mut slices = vec![&self.default_unit_overrides];
        // should always be true
        if let Some(configs) = self.commander_units.get(&commander.typ()) {
            if let Some(neutral) = configs.get(&None) {
                slices.push(neutral);
            }
            if let Some(power) = configs.get(&Some(commander.get_active_power() as u8)) {
                slices.push(power);
            }
        }
        slices.into_iter()
        .flatten()
        .filter(move |config| {
            config.affects.iter().all(|filter| filter.check(self, game, unit, pos, None, other_unit))
        })
    }

    pub fn commander_unit_visibility<D: Direction>(&self, commander: &Commander, unit: &Unit<D>, game: &Game<D>, pos: Point) -> UnitVisibility {
        let mut result = self.unit_config(unit.typ()).visibility;
        for config in self.commander_unit_configs(commander, unit, game, pos, None) {
            if let Some(visibility) = config.visibility {
                result = visibility;
            }
        }
        // TODO: hero
        result
    }

    pub fn commander_unit_attribute_overrides<'a, D: Direction>(&'a self, commander: &'a Commander, unit: &'a Unit<D>, game: &'a Game<D>, pos: Point) -> impl Iterator<Item = &'a AttributeOverride> {
        self.commander_unit_configs(commander, unit, game, pos, None)
        .flat_map(|config| {
            &config.build_overrides
        })
    }

    pub fn commander_unit_start_turn_effects<D: Direction>(&self, commander: &Commander, unit: &Unit<D>, game: &Game<D>, pos: Point) -> Vec<UnitScript> {
        let mut result = Vec::new();
        for config in self.commander_unit_configs(commander, unit, game, pos, None) {
            result.extend(config.on_start_turn.iter().cloned())
        }
        result
    }

    pub fn commander_unit_end_turn_effects<D: Direction>(&self, commander: &Commander, unit: &Unit<D>, game: &Game<D>, pos: Point) -> Vec<UnitScript> {
        let mut result = Vec::new();
        for config in self.commander_unit_configs(commander, unit, game, pos, None) {
            result.extend(config.on_end_turn.iter().cloned())
        }
        result
    }

    pub fn commander_unit_death_effects<D: Direction>(&self, commander: &Commander, unit: &Unit<D>, game: &Game<D>, pos: Point) -> Vec<UnitScript> {
        let mut result = Vec::new();
        for config in self.commander_unit_configs(commander, unit, game, pos, None) {
            result.extend(config.on_death.iter().cloned())
        }
        result
    }

    pub fn commander_unit_attack_effects<D: Direction>(&self, commander: &Commander, unit: &Unit<D>, game: &Game<D>, pos: Point, other_unit: &Unit<D>, other_pos: Point) -> Vec<AttackScript> {
        let mut result = Vec::new();
        for config in self.commander_unit_configs(commander, unit, game, pos, Some((other_unit, other_pos))) {
            result.extend(config.on_attack.iter().cloned())
        }
        result
    }

    pub fn commander_unit_kill_effects<D: Direction>(&self, commander: &Commander, unit: &Unit<D>, game: &Game<D>, pos: Point, other_unit: &Unit<D>, other_pos: Point) -> Vec<KillScript> {
        let mut result = Vec::new();
        for config in self.commander_unit_configs(commander, unit, game, pos, Some((other_unit, other_pos))) {
            result.extend(config.on_kill.iter().cloned())
        }
        result
    }

    pub fn commander_movement_bonus<D: Direction>(&self, commander: &Commander, unit: &Unit<D>, game: &Game<D>, pos: Point) -> Rational32 {
        let mut result = Rational32::from_integer(0);
        for config in self.commander_unit_configs(commander, unit, game, pos, None) {
            result += config.bonus_movement_points;
        }
        result
    }

    pub fn commander_attack_bonus<D: Direction>(&self, commander: &Commander, unit: &Unit<D>, game: &Game<D>, pos: Point, is_counter: bool, other_unit: &Unit<D>, other_pos: Point) -> Rational32 {
        let mut result = Rational32::from_integer(0);
        for config in self.commander_unit_configs(commander, unit, game, pos, Some((other_unit, other_pos))) {
            if is_counter {
                result += config.bonus_counter_attack;
            } else {
                result += config.bonus_attack;
            }
        }
        result
    }

    pub fn commander_defense_bonus<D: Direction>(&self, commander: &Commander, unit: &Unit<D>, game: &Game<D>, pos: Point, is_counter: bool, other_unit: &Unit<D>, other_pos: Point) -> Rational32 {
        let mut result = Rational32::from_integer(0);
        for config in self.commander_unit_configs(commander, unit, game, pos, Some((other_unit, other_pos))) {
            if is_counter {
                result += config.bonus_counter_defense;
            } else {
                result += config.bonus_defense;
            }
        }
        result
    }
}
