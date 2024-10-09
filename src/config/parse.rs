use rustc_hash::{FxHashMap as HashMap, FxHashSet as HashSet};
use std::error::Error;
use std::fmt::Debug;
use std::hash::Hash;
#[cfg(not(target_family = "wasm"))]
use std::path::{Path, PathBuf};
#[cfg(not(target_family = "wasm"))]
use std::fs;

use num_rational::Rational32;
use rhai::*;

use crate::commander::commander_type::CommanderType;
use crate::script::create_base_engine;
use crate::terrain::TerrainType;
use crate::terrain::attributes::TerrainAttributeKey;
use crate::units::movement::MovementType;
use crate::units::unit_types::UnitType;
use crate::units::attributes::*;
use crate::units::hero::*;

use super::custom_action_config::*;
use super::file_loader::FileLoader;
use super::hero_power_config::*;
use super::table_config::TableConfig;
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

const RULESET_CONFIG: &'static str = "ruleset.csv";
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
const TERRAIN_BUILD: &'static str = "terrain_build.csv";
const COMMANDER_CONFIG: &'static str = "commanders.csv";
const COMMANDER_POWERS: &'static str = "commander_powers.csv";
const COMMANDER_ATTRIBUTES: &'static str = "commander_attributes.csv";
const POWERED_UNITS: &'static str = "unit_powered.csv";
const POWERED_TERRAIN: &'static str = "terrain_powered.csv";
const TABLES: &'static str = "tables.csv";
// scripts
pub(super) const GLOBAL_SCRIPT: &'static str = "global";

impl Config {
    pub fn parse(
        name: String,
        load_config: Box<dyn Fn(&str) -> Result<String, Box<dyn Error>>>,
    ) -> Result<Self, Box<dyn Error>> {
        let mut file_loader = FileLoader::new(load_config);
        let global_ast = file_loader.load_rhai_module(&GLOBAL_SCRIPT.to_string())?;
        let global_ast = Shared::into_inner(global_ast).unwrap();
        let mut global_constants = Scope::new();
        for (name, _, value) in global_ast.iter_literal_variables(true, false) {
            global_constants.push_constant(name, value);
        }
        let engine = create_base_engine();
        let global_module = engine.optimize_ast(&global_constants, global_ast.clone(), OptimizationLevel::Simple);
        let global_module = Module::eval_ast_as_new(Scope::new(), &global_module, &engine)?.into();

        let mut result = Self {
            name,
            owner_colors: Vec::new(),
            // units
            unit_types: Vec::new(),
            units: HashMap::default(),
            unit_transports: HashMap::default(),
            unit_attributes: HashMap::default(),
            unit_hidden_attributes: HashMap::default(),
            unit_status: HashMap::default(),
            attack_damage: HashMap::default(),
            custom_actions: Vec::new(),
            max_transported: 0,
            // heroes
            hero_types: Vec::new(),
            heroes: HashMap::default(),
            hero_units: HashMap::default(),
            hero_powers: HashMap::default(),
            //hero_powered_units: HashMap::default(),
            max_hero_charge: 0,
            max_aura_range: 0,
            // terrain
            terrain_types: Vec::new(),
            terrains: HashMap::default(),
            terrain_attributes: HashMap::default(),
            terrain_hidden_attributes: HashMap::default(),
            movement_cost: HashMap::default(),
            attack_bonus: HashMap::default(),
            defense_bonus: HashMap::default(),
            build: HashMap::default(),
            max_capture_resistance: 0,
            terrain_max_anger: 0,
            terrain_max_built_this_turn: 0,
            // detail
            max_sludge: 1,
            // commanders
            commander_types: Vec::new(),
            commanders: HashMap::default(),
            commander_powers: HashMap::default(),
            terrain_overrides: Vec::new(),
            unit_overrides: Vec::new(),
            commander_unit_attributes: HashMap::default(),
            max_commander_charge: 0,
            // rhai
            global_ast,
            global_module,
            global_constants,
            asts: Vec::new(),
            functions: Vec::new(),
            custom_tables: HashMap::default(),
        };

        // ruleset.csv
        let mut neutral_color = None;
        file_loader.table_key_value(RULESET_CONFIG, |key, value, file_loader| {
            match key {
                "NeutralColor" => {
                    neutral_color = Some(<[u8; 4]>::from_conf(value, file_loader)?.0);
                }
                "PlayerColors" => {
                    // owner_colors.len is checked below, so needs_content can be false here
                    result.owner_colors = parse_inner_vec(value, false, file_loader)?.0;
                }
                _ => ()
            }
            Ok(())
        })?;
        if result.owner_colors.len() < 2 {
            return Err(Box::new(ConfigParseError::NotEnoughPlayerColors));
        }
        match neutral_color {
            Some(color) => result.owner_colors.insert(0, color),
            None => return Err(Box::new(ConfigParseError::MissingNeutralColor)),
        }

        // simple unit data
        file_loader.table_with_headers(UNIT_CONFIG, |line: UnitTypeConfig| {
            if result.units.contains_key(&line.id) {
                // TODO: error
            }
            result.unit_types.push(line.id);
            result.max_transported = result.max_transported.max(line.transport_capacity);
            result.units.insert(line.id, line);
            Ok(())
        })?;

        // unit transport
        let data = file_loader.load_config(UNIT_TRANSPORT)?;
        let mut reader = csv::ReaderBuilder::new().delimiter(b';').from_reader(data.as_bytes());
        let mut transported: Vec<UnitType> = Vec::new();
        for h in reader.headers()?.into_iter().skip(1) {
            let header = UnitType::from_conf(h, &mut file_loader)?.0;
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
                Some(t) => UnitType::from_conf(t, &mut file_loader)?.0,
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
        let data = file_loader.load_config(UNIT_ATTRIBUTES)?;
        let mut reader = csv::ReaderBuilder::new().delimiter(b';').from_reader(data.as_bytes());
        let mut attributes: Vec<AttributeKey> = Vec::new();
        for h in reader.headers()?.into_iter().skip(1) {
            let header = AttributeKey::from_conf(h, &mut file_loader)?.0;
            if attributes.contains(&header) {
                return Err(Box::new(ConfigParseError::DuplicateHeader(h.to_string())))
            }
            attributes.push(header);
        }
        for (l, line) in reader.records().enumerate() {
            let line = line?;
            let mut line = line.iter();
            let typ: UnitType = match line.next() {
                Some(t) => UnitType::from_conf(t, &mut file_loader)?.0,
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
            values.sort();
            result.unit_attributes.insert(typ, values);
            result.unit_hidden_attributes.insert(typ, hidden);
        }

        // unit status
        let data = file_loader.load_config(UNIT_STATUS)?;
        let mut reader = csv::ReaderBuilder::new().delimiter(b';').from_reader(data.as_bytes());
        let mut headers: Vec<ActionStatus> = Vec::new();
        for h in reader.headers()?.into_iter().skip(1) {
            let header = ActionStatus::from_conf(h, &mut file_loader)?.0;
            if headers.contains(&header) || header == ActionStatus::Ready {
                return Err(Box::new(ConfigParseError::DuplicateHeader(h.to_string())))
            }
            headers.push(header);
        }
        for (l, line) in reader.records().enumerate() {
            let line = line?;
            let mut line = line.iter();
            let typ: UnitType = match line.next() {
                Some(t) => UnitType::from_conf(t, &mut file_loader)?.0,
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
        let data = file_loader.load_config(UNIT_DAMAGE)?;
        let mut reader = csv::ReaderBuilder::new().delimiter(b';').from_reader(data.as_bytes());
        // TODO: ensure uniqueness of column and row IDs
        let mut defenders: Vec<UnitType> = Vec::new();
        for h in reader.headers()?.into_iter().skip(1) {
            let header = UnitType::from_conf(h, &mut file_loader)?.0;
            if defenders.contains(&header) {
                return Err(Box::new(ConfigParseError::DuplicateHeader(h.to_string())))
            }
            defenders.push(header);
        }
        for line in reader.records() {
            let line = line?;
            let mut line = line.into_iter();
            let typ: UnitType = match line.next() {
                Some(t) => UnitType::from_conf(t, &mut file_loader)?.0,
                _ => continue,
            };
            let mut values = HashMap::default();
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
        file_loader.table_with_headers(CUSTOM_ACTIONS, |line: CustomActionConfig| {
            result.custom_actions.push(line);
            Ok(())
        })?;

        // simple hero data
        let mut bonus_transported = 0;
        file_loader.table_with_headers(HERO_CONFIG, |line: HeroTypeConfig| {
            if result.heroes.contains_key(&line.id) {
                // TODO: error
            }
            result.hero_types.push(line.id);
            result.hero_powers.insert(line.id, Vec::new());
            //result.hero_powered_units.insert(line.id, HashMap::default());
            if line.charge > i8::MAX as u8 {
                return Err(Box::new(ConfigParseError::HeroMaxChargeExceeded(i8::MAX as u8)));
            }
            result.max_hero_charge = result.max_hero_charge.max(line.charge);
            bonus_transported = bonus_transported.max(line.transport_capacity as usize);
            result.heroes.insert(line.id, line);
            Ok(())
        })?;
        result.max_transported += bonus_transported;

        // unit is allowed to have that hero
        let data = file_loader.load_config(UNIT_HEROES)?;
        let mut reader = csv::ReaderBuilder::new().delimiter(b';').from_reader(data.as_bytes());
        // TODO: ensure uniqueness of column and row IDs
        let mut units: Vec<UnitType> = Vec::new();
        for h in reader.headers()?.into_iter().skip(1) {
            let header = UnitType::from_conf(h, &mut file_loader)?.0;
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
                Some(t) => HeroType::from_conf(t, &mut file_loader)?.0,
                _ => continue,
            };
            let mut values = HashSet::default();
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
        file_loader.table_with_headers(HERO_POWERS, |line: HeroPowerConfig| {
            result.max_aura_range = result.max_aura_range.max(line.aura_range).max(line.aura_range_transported);
            result.hero_powers.get_mut(&line.hero)
            .ok_or(ConfigParseError::MissingHeroForPower(line.hero))?
            .push(line); // TODO: ensure that every hero has at least 1 power
            Ok(())
        })?;

        // simple terrain data
        file_loader.table_with_headers(TERRAIN_CONFIG, |line: TerrainTypeConfig| {
            if result.terrains.contains_key(&line.id) {
                // TODO: error
            }
            result.terrain_types.push(line.id);
            result.max_capture_resistance = result.max_capture_resistance.max(line.capture_resistance);
            result.terrain_max_anger = result.terrain_max_anger.max(line.max_anger);
            result.terrain_max_built_this_turn = result.terrain_max_built_this_turn.max(line.max_builds_per_turn);
            result.terrains.insert(line.id, line);
            Ok(())
        })?;

        // terrain attributes
        let data = file_loader.load_config(TERRAIN_ATTRIBUTES)?;
        let mut reader = csv::ReaderBuilder::new().delimiter(b';').from_reader(data.as_bytes());
        let mut attributes: Vec<TerrainAttributeKey> = Vec::new();
        for h in reader.headers()?.into_iter().skip(1) {
            let header = TerrainAttributeKey::from_conf(h, &mut file_loader)?.0;
            if attributes.contains(&header) {
                return Err(Box::new(ConfigParseError::DuplicateHeader(h.to_string())))
            }
            attributes.push(header);
        }
        for (l, line) in reader.records().enumerate() {
            let line = line?;
            let mut line = line.into_iter();
            let typ: TerrainType = match line.next() {
                Some(t) => TerrainType::from_conf(t, &mut file_loader)?.0,
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
        let data = file_loader.load_config(MOVEMENT_CONFIG)?;
        let mut reader = csv::ReaderBuilder::new().delimiter(b';').from_reader(data.as_bytes());
        // TODO: ensure uniqueness of column and row IDs
        let mut movement_types: Vec<MovementType> = Vec::new();
        for h in reader.headers()?.into_iter().skip(1) {
            let header = MovementType::from_conf(h, &mut file_loader)?.0;
            if movement_types.contains(&header) {
                return Err(Box::new(ConfigParseError::DuplicateHeader(h.to_string())))
            }
            movement_types.push(header);
        }
        for line in reader.records() {
            let line = line?;
            let mut line = line.into_iter();
            let typ: TerrainType = match line.next() {
                Some(t) => TerrainType::from_conf(t, &mut file_loader)?.0,
                _ => continue,
            };
            let mut values = HashMap::default();
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
        let data = file_loader.load_config(TERRAIN_ATTACK)?;
        let mut reader = csv::ReaderBuilder::new().delimiter(b';').from_reader(data.as_bytes());
        // TODO: ensure uniqueness of column and row IDs
        let mut attackers: Vec<MovementType> = Vec::new();
        for h in reader.headers()?.into_iter().skip(1) {
            let header = MovementType::from_conf(h, &mut file_loader)?.0;
            if attackers.contains(&header) {
                return Err(Box::new(ConfigParseError::DuplicateHeader(h.to_string())))
            }
            attackers.push(header);
        }
        for line in reader.records() {
            let line = line?;
            let mut line = line.into_iter();
            let typ: TerrainType = match line.next() {
                Some(t) => TerrainType::from_conf(t, &mut file_loader)?.0,
                _ => continue,
            };
            let mut values = HashMap::default();
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
        let data = file_loader.load_config(TERRAIN_DEFENSE)?;
        let mut reader = csv::ReaderBuilder::new().delimiter(b';').from_reader(data.as_bytes());
        // TODO: ensure uniqueness of column and row IDs
        let mut defenders: Vec<MovementType> = Vec::new();
        for h in reader.headers()?.into_iter().skip(1) {
            let header = MovementType::from_conf(h, &mut file_loader)?.0;
            if defenders.contains(&header) {
                return Err(Box::new(ConfigParseError::DuplicateHeader(h.to_string())))
            }
            defenders.push(header);
        }
        for line in reader.records() {
            let line = line?;
            let mut line = line.into_iter();
            let typ: TerrainType = match line.next() {
                Some(t) => TerrainType::from_conf(t, &mut file_loader)?.0,
                _ => continue,
            };
            let mut values = HashMap::default();
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
        let data = file_loader.load_config(TERRAIN_BUILD)?;
        let mut reader = csv::ReaderBuilder::new().delimiter(b';').from_reader(data.as_bytes());
        // TODO: ensure uniqueness of column and row IDs
        let mut units: Vec<UnitType> = Vec::new();
        for h in reader.headers()?.into_iter().skip(1) {
            let header = UnitType::from_conf(h, &mut file_loader)?.0;
            if units.contains(&header) {
                return Err(Box::new(ConfigParseError::DuplicateHeader(h.to_string())))
            }
            units.push(header);
        }
        for line in reader.records() {
            let line = line?;
            let mut line = line.into_iter();
            let typ: TerrainType = match line.next() {
                Some(t) => TerrainType::from_conf(t, &mut file_loader)?.0,
                _ => continue,
            };
            let mut values = Vec::new();
            for (i, val) in line.enumerate() {
                if val.len() > 0 && i < units.len() {
                    values.push(units[i]);
                }
            }
            if units.len() > 0 {
                result.build.insert(typ, values);
            }
        }

        // commanders
        let mut bonus_transported = 0;
        file_loader.table_with_headers(COMMANDER_CONFIG, |line: CommanderTypeConfig| {
            if result.commanders.contains_key(&line.id) {
                // TODO: error
            }
            result.commander_types.push(line.id);
            result.commander_powers.insert(line.id, Vec::new());
            result.commander_unit_attributes.insert(line.id, Vec::new());
            result.max_commander_charge = result.max_commander_charge.max(line.max_charge);
            bonus_transported = bonus_transported.max(line.transport_capacity as usize);
            result.commanders.insert(line.id, line);
            Ok(())
        })?;
        result.max_transported += bonus_transported;

        // commander powers
        file_loader.table_with_headers(COMMANDER_POWERS, |line: CommanderPowerConfig| {
            result.commander_powers.get_mut(&line.id)
            .ok_or(ConfigParseError::MissingCommanderForPower(line.id))?
            .push(line);
            Ok(())
        })?;

        // commanders' unit attributes
        let data = file_loader.load_config(COMMANDER_ATTRIBUTES)?;
        let mut reader = csv::ReaderBuilder::new().delimiter(b';').from_reader(data.as_bytes());
        let mut attributes: Vec<AttributeKey> = Vec::new();
        for h in reader.headers()?.into_iter().skip(2) {
            let header = AttributeKey::from_conf(h, &mut file_loader)?.0;
            if attributes.contains(&header) {
                return Err(Box::new(ConfigParseError::DuplicateHeader(h.to_string())))
            }
            attributes.push(header);
        }
        for (l, line) in reader.records().enumerate() {
            let line = line?;
            let mut line = line.into_iter();
            let typ: CommanderType = match line.next() {
                Some(t) => CommanderType::from_conf(t, &mut file_loader)?.0,
                _ => continue,
            };
            let filter: UnitTypeFilter = match line.next() {
                Some(t) => UnitTypeFilter::from_conf(t, &mut file_loader)?.0,
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
        file_loader.table_with_headers(POWERED_UNITS, |line: CommanderPowerUnitConfig| {
            result.unit_overrides.push(line);
            Ok(())
        })?;

        // terrain overrides, has to be after commander and hero parsing
        file_loader.table_with_headers(POWERED_TERRAIN, |line: TerrainPoweredConfig| {
            result.terrain_overrides.push(line);
            Ok(())
        })?;

        // tables table
        let mut table_configs = Vec::new();
        file_loader.table_with_headers(TABLES, |line: TableConfig| {
            table_configs.push(line);
            Ok(())
        })?;
        for conf in table_configs {
            let table = conf.build_table(&mut file_loader)?;
            result.custom_tables.insert(conf.id, (conf.default_value, table));
        }

        let (asts, functions) = file_loader.finish();
        result.asts = asts;
        result.functions = functions;

        Ok(result)
    }

    #[cfg(not(target_family = "wasm"))]
    pub fn parse_folder(name: impl ToString, folder: PathBuf) -> Result<Self, Box<dyn Error>> {
        if !folder.exists() || !folder.is_dir() {
            return Err(Box::new(ConfigParseError::FolderMissing(folder.to_path_buf())))
        }
        let load_config: Box<dyn Fn(&str) -> Result<String, Box<dyn Error>>> = Box::new(move |filename: &str| {
            // canonicalize and then check if still in same folder
            // to prevent path traversal attacks
            let file = folder.join(filename);
            let file = file.canonicalize()?;
            if !file.starts_with(&folder) || !file.exists() || !file.is_file() {
                return Err(Box::new(ConfigParseError::FileMissing(filename.to_string())))
            }
            Ok(fs::read_to_string(file)?)
        });
        Self::parse(name.to_string(), load_config)
    }

    #[cfg(not(target_family = "wasm"))]
    #[allow(dead_code)]
    pub (crate) fn test_config() -> Self {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("configs/default_test");
        Self::parse_folder("Test", path).expect("Failed to parse test config")
    }
}

#[cfg(not(target_family = "wasm"))]
impl Default for Config {
    fn default() -> Self {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("configs/default");
        Self::parse_folder("Default", path).expect("Failed to parse default config")
    }
}

pub trait FromConfig: Sized {
    fn from_conf<'a>(s: &'a str, loader: &mut FileLoader) -> Result<(Self, &'a str), ConfigParseError>;
}

impl FromConfig for bool {
    fn from_conf<'a>(s: &'a str, _: &mut FileLoader) -> Result<(Self, &'a str), ConfigParseError> {
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
    fn from_conf<'a>(s: &'a str, loader: &mut FileLoader) -> Result<(Self, &'a str), ConfigParseError> {
        let (num, mut s) = i32::from_conf(s, loader)?;
        let den = if s.starts_with('/') {
            let (den, r) = i32::from_conf(&s[1..], loader)?;
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

impl FromConfig for String {
    fn from_conf<'a>(s: &'a str, _: &mut FileLoader) -> Result<(Self, &'a str), ConfigParseError> {
        if let Some(pos) = s.find(&[',', ')']) {
            Ok((s[..pos].trim().to_string(), &s[pos..]))
        } else {
            Ok((s.trim().to_string(), ""))
        }
    }
}

macro_rules! uint_from_config {
    ($name: ty) => {
        impl FromConfig for $name {
            fn from_conf<'a>(s: &'a str, _: &mut FileLoader) -> Result<(Self, &'a str), ConfigParseError> {
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
            fn from_conf<'a>(mut s: &'a str, _: &mut FileLoader) -> Result<(Self, &'a str), ConfigParseError> {
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
    fn from_conf<'a>(s: &'a str, loader: &mut FileLoader) -> Result<(Self, &'a str), ConfigParseError> {
        if let Ok((a, b, s)) = parse_tuple2::<A, B>(s, loader) {
            return Ok(((a, Some(b)), s));
        }
        let (a, s) = parse_tuple1(s, loader)?;
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

pub fn parse_dyn<H: Hash + Eq + Debug, T>(data: &HashMap<H, &str>, key: H, def: Option<T>, mut from_conf: impl FnMut(&str) -> Result<(T, &str), ConfigParseError>) -> Result<T, ConfigParseError> {
    let value = match data.get(&key) {
        Some(s) => s,
        None => {
            return def.ok_or(ConfigParseError::MissingColumn(format!("{key:?}")))
        }
    };
    if value.len() == 0 && def.is_some() {
        return Ok(def.unwrap());
    }
    from_conf(value).map(|r| r.0)
}

fn _parse<H: Hash + Eq + Debug, T: FromConfig>(data: &HashMap<H, &str>, key: H, def: Option<T>, loader: &mut FileLoader) -> Result<T, ConfigParseError> {
    parse_dyn(data, key, def, |s| T::from_conf(s, loader))
}

pub fn parse<H: Hash + Eq + Debug, T: FromConfig>(data: &HashMap<H, &str>, key: H, loader: &mut FileLoader) -> Result<T, ConfigParseError> {
    _parse(data, key, None, loader)
}
pub fn parse_def<H: Hash + Eq + Debug, T: FromConfig>(data: &HashMap<H, &str>, key: H, def: T, loader: &mut FileLoader) -> Result<T, ConfigParseError> {
    _parse(data, key, Some(def), loader)
}

pub fn parse_inner_vec<'a, T: FromConfig>(s: &'a str, needs_content: bool, loader: &mut FileLoader) -> Result<(Vec<T>, &'a str), ConfigParseError> {
    parse_inner_vec_dyn(s, needs_content, |s| T::from_conf(s, loader))
}

pub fn parse_inner_vec_dyn<T>(s: &str, needs_content: bool, mut from_conf: impl FnMut(&str) -> Result<(T, &str), ConfigParseError>) -> Result<(Vec<T>, &str), ConfigParseError> {
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
        let (t, remainder) = from_conf(s)?;
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
    'a,
    A: FromConfig,
>(source: &'a str, loader: &mut FileLoader) -> Result<(A, &'a str), ConfigParseError> {
    let mut s = source.trim_start();
    let mut stop_char = None;
    if s.starts_with('(') {
        stop_char = Some(')');
        s = &s[1..];
    }
    let (a, s) = A::from_conf(s, loader)?;
    let mut s = s.trim_start();
    if stop_char.map(|c| s.starts_with(c)).unwrap_or(false) {
        s = s[1..].trim_start();
    }
    Ok((a, s))
}

pub fn parse_tuple2<
    'a,
    A: FromConfig,
    B: FromConfig,
>(source: &'a str, loader: &mut FileLoader) -> Result<(A, B, &'a str), ConfigParseError> {
    let mut s = source.trim_start();
    let mut stop_char = None;
    if s.starts_with('(') {
        stop_char = Some(')');
        s = &s[1..];
    }
    let (a, s) = A::from_conf(s, loader)?;
    let s = s.trim_start().get(1..).ok_or(ConfigParseError::NotEnoughValues(source.to_string()))?;
    let (b, s) = B::from_conf(s, loader)?;
    let mut s = s.trim_start();
    if stop_char.map(|c| s.starts_with(c)).unwrap_or(false) {
        s = s[1..].trim_start();
    }
    Ok((a, b, s))
}

pub fn parse_tuple3<
    'a,
    A: FromConfig,
    B: FromConfig,
    C: FromConfig,
>(source: &'a str, loader: &mut FileLoader) -> Result<(A, B, C, &'a str), ConfigParseError> {
    let mut s = source.trim_start();
    let mut stop_char = None;
    if s.starts_with('(') {
        stop_char = Some(')');
        s = &s[1..];
    }
    let (a, s) = A::from_conf(s, loader)?;
    let s = s.trim_start().get(1..).ok_or(ConfigParseError::NotEnoughValues(source.to_string()))?;
    let (b, s) = B::from_conf(s, loader)?;
    let s = s.trim_start().get(1..).ok_or(ConfigParseError::NotEnoughValues(source.to_string()))?;
    let (c, s) = C::from_conf(s, loader)?;
    let mut s = s.trim_start();
    if stop_char.map(|c| s.starts_with(c)).unwrap_or(false) {
        s = s[1..].trim_start();
    }
    Ok((a, b, c, s))
}

fn _parse_vec<H: Hash + Eq + Debug, T: FromConfig>(data: &HashMap<H, &str>, key: H, def: Option<Vec<T>>, loader: &mut FileLoader) -> Result<Vec<T>, ConfigParseError> {
    _parse_vec_dyn(data, key, def, |s| T::from_conf(s, loader))
}
fn _parse_vec_dyn<H: Hash + Eq + Debug, T>(data: &HashMap<H, &str>, key: H, def: Option<Vec<T>>, from_conf: impl FnMut(&str) -> Result<(T, &str), ConfigParseError>) -> Result<Vec<T>, ConfigParseError> {
    let value = match data.get(&key) {
        Some(s) => s,
        None => {
            return def.ok_or(ConfigParseError::MissingColumn(format!("{key:?}")))
        }
    };
    parse_inner_vec_dyn(value, false, from_conf).map(|(r, _)| r)
}

pub fn parse_vec<H: Hash + Eq + Debug, T: FromConfig>(data: &HashMap<H, &str>, key: H, loader: &mut FileLoader) -> Result<Vec<T>, ConfigParseError> {
    _parse_vec(data, key, None, loader)
}
pub fn parse_vec_def<H: Hash + Eq + Debug, T: FromConfig>(data: &HashMap<H, &str>, key: H, def: Vec<T>, loader: &mut FileLoader) -> Result<Vec<T>, ConfigParseError> {
    _parse_vec(data, key, Some(def), loader)
}

pub fn parse_vec_dyn_def<H: Hash + Eq + Debug, T>(data: &HashMap<H, &str>, key: H, def: Vec<T>, from_conf: impl FnMut(&str) -> Result<(T, &str), ConfigParseError>) -> Result<Vec<T>, ConfigParseError> {
    _parse_vec_dyn(data, key, Some(def), from_conf)
}

#[cfg(feature = "rendering")]
impl FromConfig for (interfaces::PreviewShape, Option<[u8; 4]>) {
    fn from_conf<'a>(s: &'a str, loader: &mut FileLoader) -> Result<(Self, &'a str), ConfigParseError> {
        let (shape, s) = string_base(s);
        let shape = match shape.parse() {
            Ok(shape) => shape,
            _ => return Err(ConfigParseError::UnknownEnumMember(shape.to_string()))
        };
        let (color, s) = parse_tuple1::<String>(s, loader)?;
        let color = if color.trim().to_lowercase().as_str() != "owner" {
            Some(<[u8; 4]>::from_conf(&color, loader)?.0)
        } else {
            None
        };
        Ok(((shape, color), s))
    }
}

impl FromConfig for [u8; 4] {
    fn from_conf<'a>(s: &'a str, _: &mut FileLoader) -> Result<(Self, &'a str), ConfigParseError> {
        let mut i = 0;
        if s.starts_with('#') {
            i += 1;
        }
        let len = s[i..].find(|c| !"0123456789abcdefABCDEF".contains(c))
            .unwrap_or(s.len() - i);
        if len == 6 || len == 8 {
            let alpha = if len == 6 {
                255
            } else {
                u8::from_str_radix(&s[i + 6 .. i + 8], 16).unwrap()
            };
            Ok(([
                u8::from_str_radix(&s[i .. i + 2], 16).unwrap(),
                u8::from_str_radix(&s[i + 2 .. i + 4], 16).unwrap(),
                u8::from_str_radix(&s[i + 4 .. i + 6], 16).unwrap(),
                alpha,
            ], &s[i + len..]))
        } else if len == 3 || len == 4 {
            let alpha = if len == 3 {
                255
            } else {
                17 * u8::from_str_radix(&s[i + 3 .. i + 4], 16).unwrap()
            };
            Ok(([
                17 * u8::from_str_radix(&s[i .. i + 1], 16).unwrap(),
                17 * u8::from_str_radix(&s[i + 1 .. i + 2], 16).unwrap(),
                17 * u8::from_str_radix(&s[i + 2 .. i + 3], 16).unwrap(),
                alpha,
            ], &s[i + len..]))
        } else {
            Err(ConfigParseError::InvalidColor(s.to_string()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn import_default_config() {
        Config::default();
    }
}
