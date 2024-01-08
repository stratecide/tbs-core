use std::collections::HashMap;
use std::collections::HashSet;
use std::error::Error;

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

use super::ConfigParseError;
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
    // units
    unit_types: Vec<UnitType>,
    units: HashMap<UnitType, UnitTypeConfig>,
    unit_transports: HashMap<UnitType, Vec<UnitType>>,
    unit_attributes: HashMap<UnitType, Vec<AttributeKey>>,
    unit_hidden_attributes: HashMap<UnitType, Vec<AttributeKey>>,
    attack_damage: HashMap<UnitType, HashMap<UnitType, u16>>,
    // heroes
    hero_types: Vec<HeroType>,
    heroes: HashMap<HeroType, HeroTypeConfig>,
    hero_units: HashMap<HeroType, HashSet<UnitType>>,
    max_hero_charge: u8,
    // terrain
    terrain_types: Vec<TerrainType>,
    terrains: HashMap<TerrainType, TerrainTypeConfig>,
    terrain_attributes: HashMap<TerrainType, Vec<TerrainAttributeKey>>,
    terrain_hidden_attributes: HashMap<TerrainType, Vec<TerrainAttributeKey>>,
    movement_cost: HashMap<TerrainType, HashMap<MovementType, Rational32>>,
    attack_bonus: HashMap<TerrainType, HashMap<MovementType, Rational32>>,
    defense_bonus: HashMap<TerrainType, HashMap<MovementType, Rational32>>,
    hides_unit: HashMap<TerrainType, HashSet<UnitType>>,
    build_or_repair: HashMap<TerrainType, Vec<UnitType>>,
    max_capture_resistance: u8,
    terrain_max_anger: u8,
    terrain_max_built_this_turn: u8,
    // commanders
    commander_types: Vec<CommanderType>,
    commanders: HashMap<CommanderType, CommanderTypeConfig>,
    commander_powers: HashMap<CommanderType, Vec<CommanderPowerConfig>>,
    default_unit_overrides: Vec<CommanderPowerUnitConfig>,
    commander_units: HashMap<CommanderType, Vec<CommanderPowerUnitConfig>>,
    commander_unit_attributes: HashMap<CommanderType, Vec<(UnitTypeFilter, Vec<AttributeKey>, Vec<AttributeKey>)>>,
    max_commander_charge: u32,
}

impl ConfigInterface for Config {}

impl Config {
    pub fn parse(
        // units
        unit_type_config: &str,
        unit_transport_config: &str,
        unit_attribute_config: &str,
        attack_damage_config: &str,
        // heroes
        hero_type_config: &str,
        hero_unit_config: &str,
        // terrain
        terrain_type_config: &str,
        terrain_attribute_config: &str,
        terrain_movement_config: &str,
        terrain_attack_config: &str,
        terrain_defense_config: &str,
        terrain_hiding_config: &str,
        terrain_build_repair_config: &str,
        // commanders
        commander_type_config: &str,
        commander_power_config: &str,
        commander_power_unit_config: &str,
        commander_unit_attribute_config: &str,
    ) -> Result<Self, Box<dyn Error>> {
        let mut result = Self {
            // units
            unit_types: Vec::new(),
            units: HashMap::new(),
            unit_transports: HashMap::new(),
            unit_attributes: HashMap::new(),
            unit_hidden_attributes: HashMap::new(),
            attack_damage: HashMap::new(),
            // heroes
            hero_types: Vec::new(),
            heroes: HashMap::new(),
            hero_units: HashMap::new(),
            max_hero_charge: 0,
            // terrain
            terrain_types: Vec::new(),
            terrains: HashMap::new(),
            terrain_attributes: HashMap::new(),
            terrain_hidden_attributes: HashMap::new(),
            movement_cost: HashMap::new(),
            attack_bonus: HashMap::new(),
            defense_bonus: HashMap::new(),
            hides_unit: HashMap::new(),
            build_or_repair: HashMap::new(),
            max_capture_resistance: 0,
            terrain_max_anger: 0,
            terrain_max_built_this_turn: 0,
            // commanders
            commander_types: Vec::new(),
            commanders: HashMap::new(),
            commander_powers: HashMap::new(),
            default_unit_overrides: Vec::new(),
            commander_units: HashMap::new(),
            commander_unit_attributes: HashMap::new(),
            max_commander_charge: 0,
        };
        // simple unit data
        let mut reader = csv::Reader::from_reader(unit_type_config.as_bytes());
        for line in reader.deserialize() {
            let line: UnitTypeConfig = line?;
            result.unit_types.push(line.id);
            result.units.insert(line.id, line);
        }
        // unit transport
        let mut reader = csv::Reader::from_reader(unit_transport_config.as_bytes());
        let mut transported: Vec<UnitType> = Vec::new();
        for header in reader.headers()?.into_iter().skip(1) {
            transported.push(serde_json::from_str(header)?);
        }
        for line in reader.records() {
            let mut line = line.into_iter();
            let typ: UnitType = match line.next() {
                Some(t) => serde_json::from_str(t.as_slice())?,
                _ => continue,
            };
            let mut values: Vec<UnitType> = Vec::new();
            for (i, val) in line.enumerate() {
                if val.as_slice().len() > 0 && i < transported.len() {
                    values.push(transported[i]);
                }
            }
            if transported.len() > 0 {
                result.unit_transports.insert(typ, values);
            }
        }
        // unit attributes
        let mut reader = csv::Reader::from_reader(unit_attribute_config.as_bytes());
        let mut attributes: Vec<AttributeKey> = Vec::new();
        for header in reader.headers()?.into_iter().skip(1) {
            attributes.push(serde_json::from_str(header)?);
        }
        for (l, line) in reader.records().enumerate() {
            let mut line = line.into_iter();
            let typ: UnitType = match line.next() {
                Some(t) => serde_json::from_str(t.as_slice())?,
                _ => continue,
            };
            let mut values: Vec<AttributeKey> = Vec::new();
            let mut hidden: Vec<AttributeKey> = Vec::new();
            for (i, val) in line.enumerate() {
                match val.as_slice() {
                    "true" => values.push(attributes[i]),
                    "false" => {
                        values.push(attributes[i]);
                        hidden.push(attributes[i]);
                    }
                    "" => (),
                    e => return Err(Box::new(ConfigParseError::InvalidCellData("unit_attribute_config", l, i, e.to_string()))),
                }
            }
            result.unit_attributes.insert(typ, values);
            result.unit_hidden_attributes.insert(typ, hidden);
        }
        // attack damage
        let mut reader = csv::Reader::from_reader(attack_damage_config.as_bytes());
        // TODO: ensure uniqueness of column and row IDs
        let mut defenders: Vec<UnitType> = Vec::new();
        for header in reader.headers()?.into_iter().skip(1) {
            defenders.push(serde_json::from_str(header)?);
        }
        for line in reader.records() {
            let mut line = line.into_iter();
            let typ: UnitType = match line.next() {
                Some(t) => serde_json::from_str(t.as_slice())?,
                _ => continue,
            };
            let mut values = HashMap::new();
            for (i, val) in line.enumerate() {
                if val.as_slice().len() > 0 && i < defenders.len() {
                    values.insert(defenders[i], val.as_slice().parse()?);
                }
            }
            if defenders.len() > 0 {
                result.attack_damage.insert(typ, values);
            }
        }

        // simple hero data
        let mut reader = csv::Reader::from_reader(hero_type_config.as_bytes());
        for line in reader.deserialize() {
            let line: HeroTypeConfig = line?;
            result.hero_types.push(line.id);
            result.max_hero_charge = result.max_hero_charge.max(line.charge);
            result.heroes.insert(line.id, line);
        }
        if result.max_hero_charge > i8::MAX as u8 {
            // TODO: return error
        }
        // unit is allowed to have that hero
        let mut reader = csv::Reader::from_reader(hero_unit_config.as_bytes());
        // TODO: ensure uniqueness of column and row IDs
        let mut units: Vec<UnitType> = Vec::new();
        for header in reader.headers()?.into_iter().skip(1) {
            units.push(serde_json::from_str(header)?);
        }
        for line in reader.records() {
            let mut line = line.into_iter();
            let typ: HeroType = match line.next() {
                Some(t) => serde_json::from_str(t.as_slice())?,
                _ => continue,
            };
            let mut values = HashSet::new();
            for (i, val) in line.enumerate() {
                if val.as_slice().len() > 0 && i < units.len() {
                    values.insert(units[i]);
                }
            }
            if units.len() > 0 {
                result.hero_units.insert(typ, values);
            }
        }

        // simple terrain data
        let mut reader = csv::Reader::from_reader(terrain_type_config.as_bytes());
        for line in reader.deserialize() {
            let line: TerrainTypeConfig = line?;
            result.terrain_types.push(line.id);
            result.max_capture_resistance = result.max_capture_resistance.max(line.capture_resistance);
            result.terrain_max_anger = result.terrain_max_anger.max(line.max_anger);
            result.terrain_max_built_this_turn = result.terrain_max_built_this_turn.max(line.max_builds_per_turn);
            result.terrains.insert(line.id, line);
        }
        // attributes
        let mut reader = csv::Reader::from_reader(terrain_attribute_config.as_bytes());
        let mut attributes: Vec<TerrainAttributeKey> = Vec::new();
        for header in reader.headers()?.into_iter().skip(1) {
            attributes.push(serde_json::from_str(header)?);
        }
        for (l, line) in reader.records().enumerate() {
            let mut line = line.into_iter();
            let typ: TerrainType = match line.next() {
                Some(t) => serde_json::from_str(t.as_slice())?,
                _ => continue,
            };
            let mut values: Vec<TerrainAttributeKey> = Vec::new();
            let mut hidden: Vec<TerrainAttributeKey> = Vec::new();
            for (i, val) in line.enumerate() {
                match val.as_slice() {
                    "true" => values.push(attributes[i]),
                    "foggy" => {
                        values.push(attributes[i]);
                        hidden.push(attributes[i]);
                    }
                    "" => (),
                    e => return Err(Box::new(ConfigParseError::InvalidCellData("terrain_attribute_config", l, i, e.to_string()))),
                }
            }
            result.terrain_attributes.insert(typ, values);
            result.terrain_hidden_attributes.insert(typ, hidden);
        }
        // movement cost
        let mut reader = csv::Reader::from_reader(terrain_movement_config.as_bytes());
        // TODO: ensure uniqueness of column and row IDs
        let mut movement_types: Vec<MovementType> = Vec::new();
        for header in reader.headers()?.into_iter().skip(1) {
            movement_types.push(serde_json::from_str(header)?);
        }
        for line in reader.records() {
            let mut line = line.into_iter();
            let typ: TerrainType = match line.next() {
                Some(t) => serde_json::from_str(t.as_slice())?,
                _ => continue,
            };
            let mut values = HashMap::new();
            for (i, val) in line.enumerate() {
                if val.as_slice().len() > 0 && i < movement_types.len() {
                    values.insert(movement_types[i], val.as_slice().parse()?);
                }
            }
            if movement_types.len() > 0 {
                result.movement_cost.insert(typ, values);
            }
        }
        // attack bonus
        let mut reader = csv::Reader::from_reader(terrain_attack_config.as_bytes());
        // TODO: ensure uniqueness of column and row IDs
        let mut attackers: Vec<MovementType> = Vec::new();
        for header in reader.headers()?.into_iter().skip(1) {
            attackers.push(serde_json::from_str(header)?);
        }
        for line in reader.records() {
            let mut line = line.into_iter();
            let typ: TerrainType = match line.next() {
                Some(t) => serde_json::from_str(t.as_slice())?,
                _ => continue,
            };
            let mut values = HashMap::new();
            for (i, val) in line.enumerate() {
                if val.as_slice().len() > 0 && i < attackers.len() {
                    values.insert(attackers[i], val.as_slice().parse()?);
                }
            }
            if attackers.len() > 0 {
                result.attack_bonus.insert(typ, values);
            }
        }
        // defense bonus
        let mut reader = csv::Reader::from_reader(terrain_defense_config.as_bytes());
        // TODO: ensure uniqueness of column and row IDs
        let mut defenders: Vec<MovementType> = Vec::new();
        for header in reader.headers()?.into_iter().skip(1) {
            defenders.push(serde_json::from_str(header)?);
        }
        for line in reader.records() {
            let mut line = line.into_iter();
            let typ: TerrainType = match line.next() {
                Some(t) => serde_json::from_str(t.as_slice())?,
                _ => continue,
            };
            let mut values = HashMap::new();
            for (i, val) in line.enumerate() {
                if val.as_slice().len() > 0 && i < defenders.len() {
                    values.insert(defenders[i], val.as_slice().parse()?);
                }
            }
            if defenders.len() > 0 {
                result.defense_bonus.insert(typ, values);
            }
        }
        // terrain hides unit
        let mut reader = csv::Reader::from_reader(terrain_hiding_config.as_bytes());
        // TODO: ensure uniqueness of column and row IDs
        let mut units: Vec<UnitType> = Vec::new();
        for header in reader.headers()?.into_iter().skip(1) {
            units.push(serde_json::from_str(header)?);
        }
        for line in reader.records() {
            let mut line = line.into_iter();
            let typ: TerrainType = match line.next() {
                Some(t) => serde_json::from_str(t.as_slice())?,
                _ => continue,
            };
            let mut values = HashSet::new();
            for (i, val) in line.enumerate() {
                if val.as_slice().len() > 0 && i < units.len() {
                    values.insert(units[i]);
                }
            }
            if units.len() > 0 {
                result.hides_unit.insert(typ, values);
            }
        }
        // terrain building / repairing
        let mut reader = csv::Reader::from_reader(terrain_build_repair_config.as_bytes());
        // TODO: ensure uniqueness of column and row IDs
        let mut units: Vec<UnitType> = Vec::new();
        for header in reader.headers()?.into_iter().skip(1) {
            units.push(serde_json::from_str(header)?);
        }
        for line in reader.records() {
            let mut line = line.into_iter();
            let typ: TerrainType = match line.next() {
                Some(t) => serde_json::from_str(t.as_slice())?,
                _ => continue,
            };
            let mut values = Vec::new();
            for (i, val) in line.enumerate() {
                if val.as_slice().len() > 0 && i < units.len() {
                    values.push(units[i]);
                }
            }
            if units.len() > 0 {
                result.build_or_repair.insert(typ, values);
            }
        }

        // commanders
        let mut reader = csv::Reader::from_reader(commander_type_config.as_bytes());
        for line in reader.deserialize() {
            let line: CommanderTypeConfig = line?;
            result.commander_types.push(line.id);
            result.max_commander_charge = result.max_commander_charge.max(line.max_charge);
            result.commanders.insert(line.id, line);
        }
        if result.max_commander_charge > i32::MAX as u32 {
            // TODO: return error
        }
        Ok(result)
    }

    pub fn name(&self) -> &str {
        // TODO
        ""
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

    pub(crate) fn unit_build_overrides(&self, typ: UnitType) -> &HashSet<AttributeOverride> {
        &self.unit_config(typ).build_overrides
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

    pub fn on_start_turn(&self, typ: UnitType) -> &[UnitScript] {
        &self.unit_config(typ).on_start_turn
    }

    pub fn on_death(&self, typ: UnitType) -> &[UnitScript] {
        &self.unit_config(typ).on_death
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

    pub fn terrain_max_capture_progress(&self, typ: TerrainType) -> u8 {
        self.terrain_config(typ).max_capture_progress
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

    pub fn terrain_hides_unit(&self, typ: TerrainType, unit: UnitType) -> bool {
        self.hides_unit.get(&typ)
        .and_then(|map| Some(map.contains(&unit)))
        .unwrap_or(false)
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

    pub(super) fn commander_unit_configs<'a, D: Direction>(&'a self, commander: &'a Commander, unit: &'a Unit<D>, game: &'a Game<D>, pos: Point) -> impl Iterator<Item = &'a CommanderPowerUnitConfig> {
        if let Some(configs) = self.commander_units.get(&commander.typ()) {
            configs.as_slice()
        } else {
            &[]
        }.iter()
        .filter(move |config| {
            (config.commander_power_id.is_none() || config.commander_power_id == Some(commander.get_active_power() as u8))
            && config.affects.iter().all(|filter| filter.check(self, unit, game, pos))
        })
        .chain(
            self.default_unit_overrides.iter().filter(move |config| {
                config.affects.iter().all(|filter| filter.check(self, unit, game, pos))
            })
        )
    }

    pub fn commander_unit_death_effects<D: Direction>(&self, commander: &Commander, unit: &Unit<D>, game: &Game<D>, pos: Point) -> Vec<UnitScript> {
        let mut result = Vec::new();
        for config in self.commander_unit_configs(commander, unit, game, pos) {
            result.extend(config.on_death.iter().cloned())
        }
        result
    }

    pub fn commander_unit_attack_effects<D: Direction>(&self, commander: &Commander, unit: &Unit<D>, game: &Game<D>, pos: Point) -> Vec<AttackScript> {
        let mut result = Vec::new();
        for config in self.commander_unit_configs(commander, unit, game, pos) {
            result.extend(config.on_attack.iter().cloned())
        }
        result
    }

    pub fn commander_unit_kill_effects<D: Direction>(&self, commander: &Commander, unit: &Unit<D>, game: &Game<D>, pos: Point) -> Vec<KillScript> {
        let mut result = Vec::new();
        for config in self.commander_unit_configs(commander, unit, game, pos) {
            result.extend(config.on_kill.iter().cloned())
        }
        result
    }

    pub fn commander_movement_bonus<D: Direction>(&self, commander: &Commander, unit: &Unit<D>, game: &Game<D>, pos: Point) -> Rational32 {
        let mut result = Rational32::from_integer(0);
        for config in self.commander_unit_configs(commander, unit, game, pos) {
            result += config.bonus_movement_points;
        }
        result
    }

    pub fn commander_attack_bonus<D: Direction>(&self, commander: &Commander, unit: &Unit<D>, game: &Game<D>, pos: Point, is_counter: bool) -> Rational32 {
        let mut result = Rational32::from_integer(0);
        for config in self.commander_unit_configs(commander, unit, game, pos) {
            if is_counter {
                result += config.bonus_counter_attack;
            } else {
                result += config.bonus_attack;
            }
        }
        result
    }

    pub fn commander_defense_bonus<D: Direction>(&self, commander: &Commander, unit: &Unit<D>, game: &Game<D>, pos: Point, is_counter: bool) -> Rational32 {
        let mut result = Rational32::from_integer(0);
        for config in self.commander_unit_configs(commander, unit, game, pos) {
            if is_counter {
                result += config.bonus_counter_defense;
            } else {
                result += config.bonus_defense;
            }
        }
        result
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn export_import_map_d4() {
        // TODO
    }
}
