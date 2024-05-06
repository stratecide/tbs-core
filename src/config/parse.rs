use std::collections::HashMap;
use std::collections::HashSet;
use std::error::Error;
use std::fmt::Debug;
use std::fs;
use std::hash::Hash;
use std::path::Path;
use std::path::PathBuf;

use num_rational::Rational32;

use crate::commander::commander_type::CommanderType;
use crate::terrain::TerrainType;
use crate::terrain::attributes::TerrainAttributeKey;
use crate::units::movement::MovementType;
use crate::units::unit_types::UnitType;
use crate::units::attributes::*;
use crate::units::hero::*;

use super::custom_action_config::*;
use super::hero_power_config::*;
use super::terrain_powered::*;
use super::ConfigParseError;
use super::commander_power_config::*;
use super::commander_type_config::*;
use super::commander_unit_config::*;
use super::hero_type_config::*;
use super::terrain_type_config::*;
use super::unit_filter::*;
use super::unit_type_config::*;
use super::config::Config;

const UNIT_CONFIG: &'static str = "units.csv";
const UNIT_ATTRIBUTES: &'static str = "unit_attributes.csv";
const UNIT_TRANSPORT: &'static str = "unit_transport.csv";
const UNIT_DAMAGE: &'static str = "unit_damage.csv";
const UNIT_STATUS: &'static str = "unit_status.csv";
const CUSTOM_ACTIONS: &'static str = "custom_actions.csv";
const HERO_CONFIG: &'static str = "heroes.csv";
const HERO_POWERS: &'static str = "hero_powers.csv";
const UNIT_HEROES: &'static str = "unit_heroes.csv";
const TERRAIN_CONFIG: &'static str = "terrain.csv";
const TERRAIN_ATTRIBUTES: &'static str = "terrain_attributes.csv";
const MOVEMENT_CONFIG: &'static str = "movement.csv";
const TERRAIN_ATTACK: &'static str = "terrain_attack.csv";
const TERRAIN_DEFENSE: &'static str = "terrain_defense.csv";
const TERRAIN_BUILD_REPAIR: &'static str = "terrain_build_repair.csv";
const COMMANDER_CONFIG: &'static str = "commanders.csv";
const COMMANDER_POWERS: &'static str = "commander_powers.csv";
const COMMANDER_ATTRIBUTES: &'static str = "commander_attributes.csv";
const POWERED_UNITS: &'static str = "unit_powered.csv";
const POWERED_TERRAIN: &'static str = "terrain_powered.csv";

impl Config {
    pub fn parse(
        name: String,
        load_config: Box<dyn Fn(&str) -> Result<String, Box<dyn Error>>>,
    ) -> Result<Self, Box<dyn Error>> {
        let mut result = Self {
            name,
            // units
            unit_types: Vec::new(),
            units: HashMap::new(),
            unit_transports: HashMap::new(),
            unit_attributes: HashMap::new(),
            unit_hidden_attributes: HashMap::new(),
            unit_status: HashMap::new(),
            attack_damage: HashMap::new(),
            custom_actions: Vec::new(),
            max_transported: 0,
            // heroes
            hero_types: Vec::new(),
            heroes: HashMap::new(),
            hero_units: HashMap::new(),
            hero_powers: HashMap::new(),
            //hero_powered_units: HashMap::new(),
            max_hero_charge: 0,
            // terrain
            terrain_types: Vec::new(),
            terrains: HashMap::new(),
            terrain_attributes: HashMap::new(),
            terrain_hidden_attributes: HashMap::new(),
            movement_cost: HashMap::new(),
            attack_bonus: HashMap::new(),
            defense_bonus: HashMap::new(),
            build_or_repair: HashMap::new(),
            max_capture_resistance: 0,
            terrain_max_anger: 0,
            terrain_max_built_this_turn: 0,
            // detail
            max_sludge: 1,
            // commanders
            commander_types: Vec::new(),
            commanders: HashMap::new(),
            commander_powers: HashMap::new(),
            default_terrain_overrides: Vec::new(),
            commander_terrain: HashMap::new(),
            default_unit_overrides: Vec::new(),
            commander_units: HashMap::new(),
            commander_unit_attributes: HashMap::new(),
            max_commander_charge: 0,
        };

        // simple unit data
        let data = load_config(UNIT_CONFIG)?;
        let mut reader = csv::ReaderBuilder::new().delimiter(b';').from_reader(data.as_bytes());
        let mut headers: Vec<UnitTypeConfigHeader> = Vec::new();
        for h in reader.headers()? {
            let header = UnitTypeConfigHeader::from_conf(h)?.0;
            if headers.contains(&header) {
                return Err(Box::new(ConfigParseError::DuplicateHeader(h.to_string())))
            }
            headers.push(header);
        }
        for line in reader.records() {
            let mut map = HashMap::new();
            let line = line?;
            for (i, s) in line.iter().enumerate().take(headers.len()) {
                map.insert(headers[i], s);
            }
            let conf = UnitTypeConfig::parse(&map)?;
            if result.units.contains_key(&conf.id) {
                // TODO: error
            }
            result.unit_types.push(conf.id);
            result.max_transported = result.max_transported.max(conf.transport_capacity);
            result.units.insert(conf.id, conf);
        }

        // unit transport
        let data = load_config(UNIT_TRANSPORT)?;
        let mut reader = csv::ReaderBuilder::new().delimiter(b';').from_reader(data.as_bytes());
        let mut transported: Vec<UnitType> = Vec::new();
        for h in reader.headers()?.into_iter().skip(1) {
            let header = UnitType::from_conf(h)?.0;
            if transported.contains(&header) {
                return Err(Box::new(ConfigParseError::DuplicateHeader(h.to_string())))
            }
            if !result.units.contains_key(&header) {
                return Err(Box::new(ConfigParseError::MissingUnit(h.to_string())))
            }
            transported.push(header);
        }
        for line in reader.records() {
            let line = line?;
            let mut line = line.iter();
            let typ: UnitType = match line.next() {
                Some(t) => UnitType::from_conf(t)?.0,
                _ => continue,
            };
            let mut values: Vec<UnitType> = Vec::new();
            for (i, val) in line.enumerate() {
                if val.len() > 0 && i < transported.len() {
                    values.push(transported[i]);
                }
            }
            if transported.len() > 0 {
                result.unit_transports.insert(typ, values);
            }
        }

        // unit attributes
        let data = load_config(UNIT_ATTRIBUTES)?;
        let mut reader = csv::ReaderBuilder::new().delimiter(b';').from_reader(data.as_bytes());
        let mut attributes: Vec<AttributeKey> = Vec::new();
        for h in reader.headers()?.into_iter().skip(1) {
            let header = AttributeKey::from_conf(h)?.0;
            if attributes.contains(&header) {
                return Err(Box::new(ConfigParseError::DuplicateHeader(h.to_string())))
            }
            attributes.push(header);
        }
        for (l, line) in reader.records().enumerate() {
            let line = line?;
            let mut line = line.iter();
            let typ: UnitType = match line.next() {
                Some(t) => UnitType::from_conf(t)?.0,
                _ => continue,
            };
            let mut values: Vec<AttributeKey> = Vec::new();
            let mut hidden: Vec<AttributeKey> = Vec::new();
            for (i, val) in line.enumerate() {
                match val {
                    "true" => values.push(attributes[i]),
                    "hidden" => {
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

        // unit status
        let data = load_config(UNIT_STATUS)?;
        let mut reader = csv::ReaderBuilder::new().delimiter(b';').from_reader(data.as_bytes());
        let mut headers: Vec<ActionStatus> = Vec::new();
        for h in reader.headers()?.into_iter().skip(1) {
            let header = ActionStatus::from_conf(h)?.0;
            if headers.contains(&header) || header == ActionStatus::Ready {
                return Err(Box::new(ConfigParseError::DuplicateHeader(h.to_string())))
            }
            headers.push(header);
        }
        for (l, line) in reader.records().enumerate() {
            let line = line?;
            let mut line = line.iter();
            let typ: UnitType = match line.next() {
                Some(t) => UnitType::from_conf(t)?.0,
                _ => continue,
            };
            let mut values: Vec<ActionStatus> = vec![ActionStatus::Ready];
            for (i, val) in line.enumerate() {
                match val {
                    "true" => values.push(headers[i]),
                    "" => (),
                    e => return Err(Box::new(ConfigParseError::InvalidCellData("unit_attribute_config", l, i, e.to_string()))),
                }
            }
            if values.len() > 1 {
                result.unit_status.insert(typ, values);
            }
        }

        // attack damage
        let data = load_config(UNIT_DAMAGE)?;
        let mut reader = csv::ReaderBuilder::new().delimiter(b';').from_reader(data.as_bytes());
        // TODO: ensure uniqueness of column and row IDs
        let mut defenders: Vec<UnitType> = Vec::new();
        for h in reader.headers()?.into_iter().skip(1) {
            let header = UnitType::from_conf(h)?.0;
            if defenders.contains(&header) {
                return Err(Box::new(ConfigParseError::DuplicateHeader(h.to_string())))
            }
            defenders.push(header);
        }
        for line in reader.records() {
            let line = line?;
            let mut line = line.into_iter();
            let typ: UnitType = match line.next() {
                Some(t) => UnitType::from_conf(t)?.0,
                _ => continue,
            };
            let mut values = HashMap::new();
            for (i, val) in line.enumerate() {
                if val.len() > 0 && i < defenders.len() {
                    values.insert(defenders[i], val.parse()?);
                }
            }
            if defenders.len() > 0 {
                result.attack_damage.insert(typ, values);
            }
        }

        // custom actions
        let data: String = load_config(CUSTOM_ACTIONS)?;
        let mut reader = csv::ReaderBuilder::new().delimiter(b';').from_reader(data.as_bytes());
        // TODO: ensure uniqueness of column and row IDs
        let mut headers: Vec<CustomActionConfigHeader> = Vec::new();
        for h in reader.headers()?.into_iter() {
            let header = CustomActionConfigHeader::from_conf(h)?.0;
            if headers.contains(&header) {
                return Err(Box::new(ConfigParseError::DuplicateHeader(h.to_string())))
            }
            headers.push(header);
        }
        for line in reader.records() {
            let mut map = HashMap::new();
            let line = line?;
            for (i, s) in line.iter().enumerate().take(headers.len()) {
                map.insert(headers[i], s);
            }
            let conf = CustomActionConfig::parse(&map)?;
            result.custom_actions.push(conf);
        }

        // simple hero data
        let data = load_config(HERO_CONFIG)?;
        let mut reader = csv::ReaderBuilder::new().delimiter(b';').from_reader(data.as_bytes());
        let mut headers: Vec<HeroTypeConfigHeader> = Vec::new();
        for h in reader.headers()? {
            let header = HeroTypeConfigHeader::from_conf(h)?.0;
            if headers.contains(&header) {
                return Err(Box::new(ConfigParseError::DuplicateHeader(h.to_string())))
            }
            headers.push(header);
        }
        let mut bonus_transported = 0;
        for line in reader.records() {
            let mut map = HashMap::new();
            let line = line?;
            for (i, s) in line.iter().enumerate().take(headers.len()) {
                map.insert(headers[i], s);
            }
            let conf = HeroTypeConfig::parse(&map)?;
            if result.heroes.contains_key(&conf.id) {
                // TODO: error
            }
            result.hero_types.push(conf.id);
            result.hero_powers.insert(conf.id, Vec::new());
            //result.hero_powered_units.insert(conf.id, HashMap::new());
            result.max_hero_charge = result.max_hero_charge.max(conf.charge);
            bonus_transported = bonus_transported.max(conf.transport_capacity as usize);
            result.heroes.insert(conf.id, conf);
        }
        result.max_transported += bonus_transported;
        if result.max_hero_charge > i8::MAX as u8 {
            return Err(Box::new(ConfigParseError::HeroMaxChargeExceeded(i8::MAX as u8)));
        }

        // unit is allowed to have that hero
        let data = load_config(UNIT_HEROES)?;
        let mut reader = csv::ReaderBuilder::new().delimiter(b';').from_reader(data.as_bytes());
        // TODO: ensure uniqueness of column and row IDs
        let mut units: Vec<UnitType> = Vec::new();
        for h in reader.headers()?.into_iter().skip(1) {
            let header = UnitType::from_conf(h)?.0;
            if units.contains(&header) {
                return Err(Box::new(ConfigParseError::DuplicateHeader(h.to_string())))
            }
            if let Some(attributes) = result.unit_attributes.get(&header) {
                if !attributes.contains(&AttributeKey::Hero) {
                    return Err(Box::new(ConfigParseError::MissingUnitAttribute(header, AttributeKey::Hero)));
                }
            } else {
                return Err(Box::new(ConfigParseError::MissingUnitAttribute(header, AttributeKey::Hero)));
            }
            units.push(header);
        }
        result.hero_units.insert(HeroType::None, result.unit_types.iter()
        .filter(|u| result.unit_attributes.get(u).unwrap().contains(&AttributeKey::Hero))
        .cloned().collect());
        for line in reader.records() {
            let line = line?;
            let mut line = line.into_iter();
            let typ: HeroType = match line.next() {
                Some(t) => HeroType::from_conf(t)?.0,
                _ => continue,
            };
            let mut values = HashSet::new();
            for (i, val) in line.enumerate() {
                if val.len() > 0 && i < units.len() {
                    values.insert(units[i]);
                }
            }
            if units.len() > 0 {
                result.hero_units.insert(typ, values);
            }
        }

        // hero powers
        let data = load_config(HERO_POWERS)?;
        let mut reader = csv::ReaderBuilder::new().delimiter(b';').from_reader(data.as_bytes());
        let mut headers: Vec<HeroPowerConfigHeader> = Vec::new();
        for h in reader.headers()? {
            let header = HeroPowerConfigHeader::from_conf(h)?.0;
            if headers.contains(&header) {
                return Err(Box::new(ConfigParseError::DuplicateHeader(h.to_string())))
            }
            headers.push(header);
        }
        for line in reader.records() {
            let mut map = HashMap::new();
            let line = line?;
            for (i, s) in line.iter().enumerate().take(headers.len()) {
                map.insert(headers[i], s);
            }
            /*if map.get(&HeroPowerConfigHeader::Name).unwrap_or(&"").len() == 0 {
                continue;
            }*/
            let conf = HeroPowerConfig::parse(&map)?;
            result.hero_powers.get_mut(&conf.hero)
            .ok_or(ConfigParseError::MissingHeroForPower(conf.hero))?
            .push(conf); // TODO: ensure that every hero has at least 1 power
        }

        // simple terrain data
        let data = load_config(TERRAIN_CONFIG)?;
        let mut reader = csv::ReaderBuilder::new().delimiter(b';').from_reader(data.as_bytes());
        let mut headers: Vec<TerrainTypeConfigHeader> = Vec::new();
        for h in reader.headers()? {
            let header = TerrainTypeConfigHeader::from_conf(h)?.0;
            if headers.contains(&header) {
                return Err(Box::new(ConfigParseError::DuplicateHeader(h.to_string())))
            }
            headers.push(header);
        }
        for line in reader.records() {
            let mut map = HashMap::new();
            let line = line?;
            for (i, s) in line.iter().enumerate().take(headers.len()) {
                map.insert(headers[i], s);
            }
            let conf = TerrainTypeConfig::parse(&map)?;
            if result.terrains.contains_key(&conf.id) {
                // TODO: error
            }
            result.terrain_types.push(conf.id);
            result.max_capture_resistance = result.max_capture_resistance.max(conf.capture_resistance);
            result.terrain_max_anger = result.terrain_max_anger.max(conf.max_anger);
            result.terrain_max_built_this_turn = result.terrain_max_built_this_turn.max(conf.max_builds_per_turn);
            result.terrains.insert(conf.id, conf);
        }

        // terrain attributes
        let data = load_config(TERRAIN_ATTRIBUTES)?;
        let mut reader = csv::ReaderBuilder::new().delimiter(b';').from_reader(data.as_bytes());
        let mut attributes: Vec<TerrainAttributeKey> = Vec::new();
        for h in reader.headers()?.into_iter().skip(1) {
            let header = TerrainAttributeKey::from_conf(h)?.0;
            if attributes.contains(&header) {
                return Err(Box::new(ConfigParseError::DuplicateHeader(h.to_string())))
            }
            attributes.push(header);
        }
        for (l, line) in reader.records().enumerate() {
            let line = line?;
            let mut line = line.into_iter();
            let typ: TerrainType = match line.next() {
                Some(t) => TerrainType::from_conf(t)?.0,
                _ => continue,
            };
            let mut values: Vec<TerrainAttributeKey> = Vec::new();
            let mut hidden: Vec<TerrainAttributeKey> = Vec::new();
            for (i, val) in line.enumerate() {
                match val {
                    "true" => values.push(attributes[i]),
                    "hidden" => {
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
        let data = load_config(MOVEMENT_CONFIG)?;
        let mut reader = csv::ReaderBuilder::new().delimiter(b';').from_reader(data.as_bytes());
        // TODO: ensure uniqueness of column and row IDs
        let mut movement_types: Vec<MovementType> = Vec::new();
        for h in reader.headers()?.into_iter().skip(1) {
            let header = MovementType::from_conf(h)?.0;
            if movement_types.contains(&header) {
                return Err(Box::new(ConfigParseError::DuplicateHeader(h.to_string())))
            }
            movement_types.push(header);
        }
        for line in reader.records() {
            let line = line?;
            let mut line = line.into_iter();
            let typ: TerrainType = match line.next() {
                Some(t) => TerrainType::from_conf(t)?.0,
                _ => continue,
            };
            let mut values = HashMap::new();
            for (i, val) in line.enumerate() {
                if val.len() > 0 && i < movement_types.len() {
                    values.insert(movement_types[i], val.parse()?);
                }
            }
            if movement_types.len() > 0 {
                result.movement_cost.insert(typ, values);
            }
        }

        // attack bonus
        let data = load_config(TERRAIN_ATTACK)?;
        let mut reader = csv::ReaderBuilder::new().delimiter(b';').from_reader(data.as_bytes());
        // TODO: ensure uniqueness of column and row IDs
        let mut attackers: Vec<MovementType> = Vec::new();
        for h in reader.headers()?.into_iter().skip(1) {
            let header = MovementType::from_conf(h)?.0;
            if attackers.contains(&header) {
                return Err(Box::new(ConfigParseError::DuplicateHeader(h.to_string())))
            }
            attackers.push(header);
        }
        for line in reader.records() {
            let line = line?;
            let mut line = line.into_iter();
            let typ: TerrainType = match line.next() {
                Some(t) => TerrainType::from_conf(t)?.0,
                _ => continue,
            };
            let mut values = HashMap::new();
            for (i, val) in line.enumerate() {
                if val.len() > 0 && i < attackers.len() {
                    values.insert(attackers[i], val.parse()?);
                }
            }
            if attackers.len() > 0 {
                result.attack_bonus.insert(typ, values);
            }
        }

        // defense bonus
        let data = load_config(TERRAIN_DEFENSE)?;
        let mut reader = csv::ReaderBuilder::new().delimiter(b';').from_reader(data.as_bytes());
        // TODO: ensure uniqueness of column and row IDs
        let mut defenders: Vec<MovementType> = Vec::new();
        for h in reader.headers()?.into_iter().skip(1) {
            let header = MovementType::from_conf(h)?.0;
            if defenders.contains(&header) {
                return Err(Box::new(ConfigParseError::DuplicateHeader(h.to_string())))
            }
            defenders.push(header);
        }
        for line in reader.records() {
            let line = line?;
            let mut line = line.into_iter();
            let typ: TerrainType = match line.next() {
                Some(t) => TerrainType::from_conf(t)?.0,
                _ => continue,
            };
            let mut values = HashMap::new();
            for (i, val) in line.enumerate() {
                if val.len() > 0 && i < defenders.len() {
                    values.insert(defenders[i], val.parse()?);
                }
            }
            if defenders.len() > 0 {
                result.defense_bonus.insert(typ, values);
            }
        }

        // terrain building / repairing
        let data = load_config(TERRAIN_BUILD_REPAIR)?;
        let mut reader = csv::ReaderBuilder::new().delimiter(b';').from_reader(data.as_bytes());
        // TODO: ensure uniqueness of column and row IDs
        let mut units: Vec<UnitType> = Vec::new();
        for h in reader.headers()?.into_iter().skip(1) {
            let header = UnitType::from_conf(h)?.0;
            if units.contains(&header) {
                return Err(Box::new(ConfigParseError::DuplicateHeader(h.to_string())))
            }
            units.push(header);
        }
        for line in reader.records() {
            let line = line?;
            let mut line = line.into_iter();
            let typ: TerrainType = match line.next() {
                Some(t) => TerrainType::from_conf(t)?.0,
                _ => continue,
            };
            let mut values = Vec::new();
            for (i, val) in line.enumerate() {
                if val.len() > 0 && i < units.len() {
                    values.push(units[i]);
                }
            }
            if units.len() > 0 {
                result.build_or_repair.insert(typ, values);
            }
        }

        // commanders
        let data = load_config(COMMANDER_CONFIG)?;
        let mut reader = csv::ReaderBuilder::new().delimiter(b';').from_reader(data.as_bytes());
        let mut headers: Vec<CommanderTypeConfigHeader> = Vec::new();
        for h in reader.headers()? {
            let header = CommanderTypeConfigHeader::from_conf(h)?.0;
            if headers.contains(&header) {
                return Err(Box::new(ConfigParseError::DuplicateHeader(h.to_string())))
            }
            headers.push(header);
        }
        let mut bonus_transported = 0;
        for line in reader.records() {
            let mut map = HashMap::new();
            let line = line?;
            for (i, s) in line.iter().enumerate().take(headers.len()) {
                map.insert(headers[i], s);
            }
            let conf = CommanderTypeConfig::parse(&map)?;
            if result.commanders.contains_key(&conf.id) {
                // TODO: error
            }
            result.commander_types.push(conf.id);
            result.commander_powers.insert(conf.id, Vec::new());
            result.commander_units.insert(conf.id, HashMap::new());
            result.commander_terrain.insert(conf.id, HashMap::new());
            result.commander_unit_attributes.insert(conf.id, Vec::new());
            result.max_commander_charge = result.max_commander_charge.max(conf.max_charge);
            bonus_transported = bonus_transported.max(conf.transport_capacity as usize);
            result.commanders.insert(conf.id, conf);
        }
        result.max_transported += bonus_transported;

        // commander powers
        let data = load_config(COMMANDER_POWERS)?;
        let mut reader = csv::ReaderBuilder::new().delimiter(b';').from_reader(data.as_bytes());
        let mut headers: Vec<CommanderPowerConfigHeader> = Vec::new();
        for h in reader.headers()? {
            let header = CommanderPowerConfigHeader::from_conf(h)?.0;
            if headers.contains(&header) {
                return Err(Box::new(ConfigParseError::DuplicateHeader(h.to_string())))
            }
            headers.push(header);
        }
        for line in reader.records() {
            let mut map = HashMap::new();
            let line = line?;
            for (i, s) in line.iter().enumerate().take(headers.len()) {
                map.insert(headers[i], s);
            }
            if map.get(&CommanderPowerConfigHeader::Id).unwrap_or(&"").len() == 0 {
                continue;
            }
            let conf = CommanderPowerConfig::parse(&map)?;
            result.commander_powers.get_mut(&conf.id)
            .ok_or(ConfigParseError::MissingCommanderForPower(conf.id))?
            .push(conf);
        }

        // commanders' unit attributes
        let data = load_config(COMMANDER_ATTRIBUTES)?;
        let mut reader = csv::ReaderBuilder::new().delimiter(b';').from_reader(data.as_bytes());
        let mut attributes: Vec<AttributeKey> = Vec::new();
        for h in reader.headers()?.into_iter().skip(2) {
            let header = AttributeKey::from_conf(h)?.0;
            if attributes.contains(&header) {
                return Err(Box::new(ConfigParseError::DuplicateHeader(h.to_string())))
            }
            attributes.push(header);
        }
        for (l, line) in reader.records().enumerate() {
            let line = line?;
            let mut line = line.into_iter();
            let typ: CommanderType = match line.next() {
                Some(t) => CommanderType::from_conf(t)?.0,
                _ => continue,
            };
            let filter: UnitTypeFilter = match line.next() {
                Some(t) => UnitTypeFilter::from_conf(t)?.0,
                _ => continue,
            };
            let mut values: Vec<AttributeKey> = Vec::new();
            let mut hidden: Vec<AttributeKey> = Vec::new();
            for (i, val) in line.enumerate() {
                match val {
                    "true" => values.push(attributes[i]),
                    "false" => {
                        values.push(attributes[i]);
                        hidden.push(attributes[i]);
                    }
                    "" => (),
                    e => return Err(Box::new(ConfigParseError::InvalidCellData("unit_attribute_config", l, i, e.to_string()))),
                }
            }
            result.commander_unit_attributes.get_mut(&typ).ok_or(ConfigParseError::MissingCommanderForAttributes(typ))?
            .push((filter, values, hidden));
        }

        // unit overrides, has to be after commander and hero parsing
        for (key, map) in result.commander_units.iter_mut() {
            map.insert(None, Vec::new());
            let power_count = result.commander_powers.get(key).unwrap().len();
            if power_count > u8::MAX as usize {
                return Err(Box::new(ConfigParseError::TooManyPowers(*key, power_count)));
            }
            for i in 0..power_count {
                map.insert(Some(i as u8), Vec::new());
            }
        }
        /*for (_, map) in result.hero_powered_units.iter_mut() {
            map.insert(None, Vec::new());
            map.insert(Some(true), Vec::new());
            map.insert(Some(false), Vec::new());
        }*/
        let data = load_config(POWERED_UNITS)?;
        let mut reader = csv::ReaderBuilder::new().delimiter(b';').from_reader(data.as_bytes());
        let mut headers: Vec<CommanderPowerUnitConfigHeader> = Vec::new();
        for h in reader.headers()? {
            let header = CommanderPowerUnitConfigHeader::from_conf(h)?.0;
            if headers.contains(&header) {
                return Err(Box::new(ConfigParseError::DuplicateHeader(h.to_string())))
            }
            headers.push(header);
        }
        for line in reader.records() {
            let mut map = HashMap::new();
            let line = line?;
            for (i, s) in line.iter().enumerate().take(headers.len()) {
                map.insert(headers[i], s);
            }
            let conf = CommanderPowerUnitConfig::parse(&map)?;
            match &conf.power {
                PowerRestriction::None => result.default_unit_overrides.push(conf),
                PowerRestriction::Commander(commander_type, power) => {
                    if let Some(list) = result.commander_units.get_mut(commander_type)
                    .ok_or(ConfigParseError::MissingCommanderForPower(*commander_type))?
                    .get_mut(power){
                        list.push(conf);
                    }
                }
                /*PowerRestriction::Hero(hero_type, power) => {
                    result.hero_powered_units.get_mut(hero_type)
                    .ok_or(ConfigParseError::MissingHeroForPower(*hero_type))?
                    .get_mut(power).unwrap().push(conf);
                }*/
            }
        }

        // terrain overrides, has to be after commander and hero parsing
        for (key, map) in result.commander_terrain.iter_mut() {
            map.insert(None, Vec::new());
            let power_count = result.commander_powers.get(key).unwrap().len();
            if power_count > u8::MAX as usize {
                return Err(Box::new(ConfigParseError::TooManyPowers(*key, power_count)));
            }
            for i in 0..power_count {
                map.insert(Some(i as u8), Vec::new());
            }
        }
        let data = load_config(POWERED_TERRAIN)?;
        let mut reader = csv::ReaderBuilder::new().delimiter(b';').from_reader(data.as_bytes());
        let mut headers: Vec<TerrainPoweredConfigHeader> = Vec::new();
        for h in reader.headers()? {
            let header = TerrainPoweredConfigHeader::from_conf(h)?.0;
            if headers.contains(&header) {
                return Err(Box::new(ConfigParseError::DuplicateHeader(h.to_string())))
            }
            headers.push(header);
        }
        for line in reader.records() {
            let mut map = HashMap::new();
            let line = line?;
            for (i, s) in line.iter().enumerate().take(headers.len()) {
                map.insert(headers[i], s);
            }
            let conf = TerrainPoweredConfig::parse(&map)?;
            match &conf.power {
                PowerRestriction::None => result.default_terrain_overrides.push(conf),
                PowerRestriction::Commander(commander_type, power) => {
                    if let Some(list) = result.commander_terrain.get_mut(commander_type)
                    .ok_or(ConfigParseError::MissingCommanderForPower(*commander_type))?
                    .get_mut(power){
                        list.push(conf);
                    }
                }
            }
        }
        Ok(result)
    }

    pub fn parse_folder(folder: PathBuf) -> Result<Self, Box<dyn Error>> {
        if !folder.exists() || !folder.is_dir() {
            return Err(Box::new(ConfigParseError::FolderMissing(folder.to_path_buf())))
        }
        let name = match folder.file_name().and_then(|s| s.to_str()) {
            Some(s) => s.to_string(),
            None => return Err(Box::new(ConfigParseError::FolderMissing(folder.to_path_buf()))),
        };
        let load_config: Box<dyn Fn(&str) -> Result<String, Box<dyn Error>>> = Box::new(move |filename: &str| {
            let file = folder.join(filename);
            if !file.exists() || !file.is_file() {
                return Err(Box::new(ConfigParseError::FileMissing(filename.to_string())))
            }
            println!("{filename}");
            Ok(fs::read_to_string(file)?)
        });
        Self::parse(name, load_config)
    }

    #[allow(dead_code)]
    pub (crate) fn test_config() -> Self {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("configs/default_test");
        Self::parse_folder(path).expect("Failed to parse test config")
    }
}

impl Default for Config {
    fn default() -> Self {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("configs/default");
        Self::parse_folder(path).expect("Failed to parse default config")
    }
}

pub trait FromConfig: Sized {
    fn from_conf(s: &str) -> Result<(Self, &str), ConfigParseError>;
}

impl FromConfig for bool {
    fn from_conf(s: &str) -> Result<(Self, &str), ConfigParseError> {
        let s = s.trim_start();
        for t in ["true", "t"] {
            if s.starts_with(t) {
                return Ok((true, &s[t.len()..]));
            }
        }
        for f in ["false", "f"] {
            if s.starts_with(f) {
                return Ok((false, &s[f.len()..]));
            }
        }
        Err(ConfigParseError::InvalidBool(s.to_string()))
    }
}

impl FromConfig for Rational32 {
    fn from_conf(s: &str) -> Result<(Self, &str), ConfigParseError> {
        let (num, mut s) = i32::from_conf(s)?;
        let den = if s.starts_with('/') {
            let (den, r) = i32::from_conf(&s[1..])?;
            s = r;
            den
        } else {
            1
        };
        if den == 0 {
            return Err(ConfigParseError::DivisionByZero(num));
        }
        Ok((Rational32::new(num, den), s))
    }
}

macro_rules! uint_from_config {
    ($name: ty) => {
        impl FromConfig for $name {
            fn from_conf(s: &str) -> Result<(Self, &str), ConfigParseError> {
                let s = s.trim_start();
                let index = s.find(|c| !char::is_numeric(c)).unwrap_or(s.len());
                let (number, remainder) = s.split_at(index);
                if let Ok(result) = number.parse() {
                    return Ok((result, remainder.trim_start()));
                }
                Err(ConfigParseError::InvalidInteger(s.to_string()))
            }
        }
    };
}

macro_rules! sint_from_config {
    ($name: ty) => {
        impl FromConfig for $name {
            fn from_conf(mut s: &str) -> Result<(Self, &str), ConfigParseError> {
                s = s.trim_start();
                let sign = if s.starts_with('-') {
                    s = &s[1..];
                    -1
                } else {
                    1
                };
                let index = s.find(|c| !char::is_numeric(c)).unwrap_or(s.len());
                let (number, remainder) = s.split_at(index);
                if let Ok(result) = number.parse::<$name>() {
                    return Ok((result * sign, remainder.trim_start()));
                }
                Err(ConfigParseError::InvalidInteger(s.to_string()))
            }
        }
    };
}

uint_from_config!(u8);
uint_from_config!(u16);
uint_from_config!(u32);
sint_from_config!(i8);
sint_from_config!(i16);
sint_from_config!(i32);

impl<A: FromConfig, B: FromConfig> FromConfig for (A, Option<B>) {
    fn from_conf(s: &str) -> Result<(Self, &str), ConfigParseError> {
        if let Ok((a, b, s)) = parse_tuple2::<A, B>(s) {
            return Ok(((a, Some(b)), s));
        }
        let (a, s) = parse_tuple1(s)?;
        Ok(((a, None), s))
    }
}

pub fn string_base(s: &str) -> (&str, &str) {
    let s = s.trim_start();
    if let Some(split_index) = s.find(|c| !char::is_alphanumeric(c)) {
        (s[..split_index].trim_end(), s[split_index..].trim_start())
    } else {
        (s, "")
    }
}

fn _parse<H: Hash + Eq + Debug, T: FromConfig>(data: &HashMap<H, &str>, key: H, def: Option<T>) -> Result<T, ConfigParseError> {
    let value = match data.get(&key) {
        Some(s) => s,
        None => {
            return def.ok_or(ConfigParseError::MissingColumn(format!("{key:?}")))
        }
    };
    if value.len() == 0 && def.is_some() {
        return Ok(def.unwrap());
    }
    T::from_conf(value).map(|r| r.0)
}

pub fn parse<H: Hash + Eq + Debug, T: FromConfig>(data: &HashMap<H, &str>, key: H) -> Result<T, ConfigParseError> {
    _parse(data, key, None)
}
pub fn parse_def<H: Hash + Eq + Debug, T: FromConfig>(data: &HashMap<H, &str>, key: H, def: T) -> Result<T, ConfigParseError> {
    _parse(data, key, Some(def))
}

pub fn parse_inner_vec<T: FromConfig>(s: &str, needs_content: bool) -> Result<(Vec<T>, &str), ConfigParseError> {
    let mut result: Vec<T> = Vec::new();
    let mut s = s.trim_start();
    let mut stop_char = None;
    if s.starts_with('(') {
        stop_char = Some(')');
        s = &s[1..];
    }
    loop {
        if s.len() == 0 {
            break;
        }
        if stop_char.map(|c| s.starts_with(c)).unwrap_or(false) {
            s = s[1..].trim_start();
            break;
        }
        let (t, remainder) = T::from_conf(s)?;
        result.push(t);
        s = remainder.trim_start();
        if s.starts_with(',') {
            s = s[1..].trim_start();
        }
    }
    if needs_content && result.len() == 0 {
        return Err(ConfigParseError::EmptyList);
    }
    Ok((result, s))
}

pub fn parse_tuple1<
    A: FromConfig,
>(source: &str) -> Result<(A, &str), ConfigParseError> {
    let mut s = source.trim_start();
    let mut stop_char = None;
    if s.starts_with('(') {
        stop_char = Some(')');
        s = &s[1..];
    }
    let (a, s) = A::from_conf(s)?;
    let mut s = s.trim_start();
    if stop_char.map(|c| s.starts_with(c)).unwrap_or(false) {
        s = s[1..].trim_start();
    }
    Ok((a, s))
}

pub fn parse_tuple2<
    A: FromConfig,
    B: FromConfig,
>(source: &str) -> Result<(A, B, &str), ConfigParseError> {
    let mut s = source.trim_start();
    let mut stop_char = None;
    if s.starts_with('(') {
        stop_char = Some(')');
        s = &s[1..];
    }
    let (a, s) = A::from_conf(s)?;
    let s = s.trim_start().get(1..).ok_or(ConfigParseError::NotEnoughValues(source.to_string()))?;
    let (b, s) = B::from_conf(s)?;
    let mut s = s.trim_start();
    if stop_char.map(|c| s.starts_with(c)).unwrap_or(false) {
        s = s[1..].trim_start();
    }
    Ok((a, b, s))
}

pub fn parse_tuple3<
    A: FromConfig,
    B: FromConfig,
    C: FromConfig,
>(source: &str) -> Result<(A, B, C, &str), ConfigParseError> {
    let mut s = source.trim_start();
    let mut stop_char = None;
    if s.starts_with('(') {
        stop_char = Some(')');
        s = &s[1..];
    }
    let (a, s) = A::from_conf(s)?;
    let s = s.trim_start().get(1..).ok_or(ConfigParseError::NotEnoughValues(source.to_string()))?;
    let (b, s) = B::from_conf(s)?;
    let s = s.trim_start().get(1..).ok_or(ConfigParseError::NotEnoughValues(source.to_string()))?;
    let (c, s) = C::from_conf(s)?;
    let mut s = s.trim_start();
    if stop_char.map(|c| s.starts_with(c)).unwrap_or(false) {
        s = s[1..].trim_start();
    }
    Ok((a, b, c, s))
}

fn _parse_vec<H: Hash + Eq + Debug, T: FromConfig>(data: &HashMap<H, &str>, key: H, def: Option<Vec<T>>) -> Result<Vec<T>, ConfigParseError> {
    let value = match data.get(&key) {
        Some(s) => s,
        None => {
            return def.ok_or(ConfigParseError::MissingColumn(format!("{key:?}")))
        }
    };
    parse_inner_vec(value, false).map(|(r, _)| r)
}

pub fn parse_vec<H: Hash + Eq + Debug, T: FromConfig>(data: &HashMap<H, &str>, key: H) -> Result<Vec<T>, ConfigParseError> {
    _parse_vec(data, key, None)
}
pub fn parse_vec_def<H: Hash + Eq + Debug, T: FromConfig>(data: &HashMap<H, &str>, key: H, def: Vec<T>) -> Result<Vec<T>, ConfigParseError> {
    _parse_vec(data, key, Some(def))
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn import_default_config() {
        Config::default();
    }
}
