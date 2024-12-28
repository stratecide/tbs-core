use rustc_hash::FxHashMap as HashMap;
use std::error::Error;
use std::fmt::Debug;
use std::hash::Hash;
#[cfg(not(target_family = "wasm"))]
use std::path::{Path, PathBuf};
#[cfg(not(target_family = "wasm"))]
use std::fs;
use std::usize;

use num_rational::Rational32;
use rhai::*;

use crate::game::event_fx::effect_constructor_module;
use crate::map::direction::{Direction4, Direction6};
use crate::script::{create_base_engine, MyPackage4, MyPackage6};
use crate::terrain::TerrainType;
use crate::units::movement::MovementType;
use crate::units::unit_types::UnitType;

use super::effect_config::{EffectConfig, EffectVisibility};
use super::{custom_action_config::*, editor_tag_config};
use super::file_loader::FileLoader;
use super::global_events::GlobalEventConfig;
use super::hero_power_config::*;
use super::movement_type_config::MovementTypeConfig;
use super::table_config::TableConfig;
use super::tag_config::{TagConfig, TagType};
use super::terrain_powered::*;
use super::token_typ_config::TokenTypeConfig;
use super::ConfigParseError;
use super::commander_power_config::*;
use super::commander_type_config::*;
use super::commander_unit_config::*;
use super::hero_type_config::*;
use super::terrain_type_config::*;
use super::unit_type_config::*;
use super::config::Config;

const RULESET_CONFIG: &'static str = "ruleset.csv";
const MOVEMENT_TYPE_CONFIG: &'static str = "movement_types.csv";
const SUB_MOVEMENT_TYPE_CONFIG: &'static str = "sub_mt_";
const TAG_CONFIG: &'static str = "tags.csv";
const UNIT_CONFIG: &'static str = "units.csv";
const UNIT_TAGS: &'static str = "unit_tags.csv";
const UNIT_TRANSPORT: &'static str = "unit_transport.csv";
const UNIT_DAMAGE: &'static str = "unit_damage.csv";
const CUSTOM_ACTIONS: &'static str = "custom_actions.csv";
const HERO_CONFIG: &'static str = "heroes.csv";
const HERO_POWERS: &'static str = "hero_powers.csv";
const TERRAIN_CONFIG: &'static str = "terrain.csv";
const TERRAIN_TAGS: &'static str = "terrain_tags.csv";
const TOKEN_CONFIG: &'static str = "tokens.csv";
const EFFECT_CONFIG: &'static str = "effects.csv";
const TOKEN_TAGS: &'static str = "token_tags.csv";
const MOVEMENT_CONFIG: &'static str = "movement.csv";
const COMMANDER_CONFIG: &'static str = "commanders.csv";
const COMMANDER_POWERS: &'static str = "commander_powers.csv";
const POWERED_UNITS: &'static str = "unit_powered.csv";
const POWERED_TERRAIN: &'static str = "terrain_powered.csv";
const GLOBAL_EVENTS: &'static str = "global_events.csv";
const TABLES: &'static str = "tables.csv";
// scripts
pub(super) const GLOBAL_SCRIPT: &'static str = "global";

impl Config {
    pub fn parse(
        name: String,
        load_config: Box<dyn Fn(&str) -> Result<String, Box<dyn Error>>>,
    ) -> Result<Self, Box<dyn Error>> {
        let mut file_loader = FileLoader::new(load_config);

        // tags
        let mut flags: Vec<TagConfig> = Vec::new();
        let mut tags: Vec<TagConfig> = Vec::new();
        file_loader.table_with_headers(TAG_CONFIG, |line: TagConfig| {
            if line.tag_type == TagType::Flag {
                if flags.iter().any(|conf| conf.name == line.name) {
                    // TODO: error
                }
                flags.push(line);
            } else {
                if tags.iter().any(|conf| conf.name == line.name) {
                    // TODO: error
                }
                tags.push(line);
            }
            Ok(())
        })?;
        for flag in &flags {
            file_loader.flags.push(flag.name.clone());
        }
        for tag in &tags {
            file_loader.tags.push(tag.name.clone());
        }

        let global_ast = file_loader.load_rhai_module(&GLOBAL_SCRIPT.to_string())?;
        let global_ast = Shared::into_inner(global_ast).unwrap();
        let mut global_constants = Scope::new();
        for (name, _, value) in global_ast.iter_literal_variables(true, false) {
            global_constants.push_constant(name, value);
        }
        /*for (i, conf) in flags.iter().enumerate() {
            global_constants.push_constant(conf.name.as_str(), FlagKey(i));
        }
        for (i, conf) in tags.iter().enumerate() {
            global_constants.push_constant(conf.name.as_str(), TagKey(i));
        }*/
        //println!("global constants: {global_constants:?}");
        // TODO: FileLoader also creates a base engine. no need to create two
        let engine = create_base_engine();
        let global_module = engine.optimize_ast(&global_constants, global_ast.clone(), OptimizationLevel::Simple);
        let global_module = Module::eval_ast_as_new(Scope::new(), &global_module, &engine)?.into();

        let mut result = Self {
            name,
            owner_colors: Vec::new(),
            // tags
            flags,
            tags,
            movement_types: Vec::new(),
            movement_type_transformer: HashMap::default(),
            // units
            units: Vec::new(),
            unknown_unit: UnitType(usize::MAX),
            unit_transports: HashMap::default(),
            attack_damage: HashMap::default(),
            custom_actions: Vec::new(),
            max_transported: 0,
            unit_flags: HashMap::default(),
            unit_tags: HashMap::default(),
            // heroes
            heroes: Vec::new(),
            max_hero_charge: 0,
            max_aura_range: 0,
            max_hero_transport_bonus: 0,
            // terrain
            terrains: Vec::new(),
            default_terrain: TerrainType(usize::MAX),
            movement_cost: HashMap::default(),
            terrain_flags: HashMap::default(),
            terrain_tags: HashMap::default(),
            // detail
            tokens: Vec::new(),
            token_flags: HashMap::default(),
            token_tags: HashMap::default(),
            // effects
            effect_types: Vec::new(),
            // commanders
            commanders: Vec::new(),
            terrain_overrides: Vec::new(),
            unit_overrides: Vec::new(),
            max_commander_charge: 0,
            // shared by terrain, units, commanders, ...
            global_events: Vec::new(),
            // rhai
            //global_ast,
            my_package_4: MyPackage4::new(),
            my_package_6: MyPackage6::new(),
            global_module,
            effect_modules: Vec::with_capacity(2),
            global_constants,
            asts: Vec::new(),
            functions: Vec::new(),
            is_unit_dead_rhai: usize::MAX,
            is_unit_movable_rhai: usize::MAX,
            calculate_damage_rhai: usize::MAX,
            deal_damage_rhai: usize::MAX,
            weapon_effects_rhai: None,
            custom_tables: HashMap::default(),
        };

        // ruleset.csv
        let mut neutral_color = None;
        let mut unknown_unit = String::new();
        let mut default_terrain = String::new();
        file_loader.table_key_value(RULESET_CONFIG, |key, value, file_loader| {
            match key {
                "NeutralColor" => {
                    neutral_color = Some(<[u8; 4]>::from_conf(value, file_loader)?.0);
                }
                "PlayerColors" => {
                    // owner_colors.len is checked below, so needs_content can be false here
                    result.owner_colors = parse_inner_vec(value, false, file_loader)?.0;
                }
                "UnknownUnit" => unknown_unit = value.to_string(),
                "DefaultTerrain" => default_terrain = value.to_string(),
                "UnitDeathTest" => result.is_unit_dead_rhai = file_loader.rhai_function(value, 2..=2)?.index,
                "UnitMovableTest" => result.is_unit_movable_rhai = file_loader.rhai_function(value, 0..=0)?.index,
                "CalculateAttackDamage" => result.calculate_damage_rhai = file_loader.rhai_function(value, 1..=1)?.index,
                "DealDamageToUnit" => result.deal_damage_rhai = file_loader.rhai_function(value, 2..=2)?.index,
                "WeaponEffects" => result.weapon_effects_rhai = Some(file_loader.rhai_function(value, 0..=0)?.index),
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
        if result.is_unit_dead_rhai == usize::MAX {
            return Err(format!("missing entry in {RULESET_CONFIG}: 'UnitDeathTest'").into());
        }
        if result.is_unit_movable_rhai == usize::MAX {
            return Err(format!("missing entry in {RULESET_CONFIG}: 'UnitMovableTest'").into());
        }
        if result.deal_damage_rhai == usize::MAX {
            return Err(format!("missing entry in {RULESET_CONFIG}: 'DealDamageToUnit'").into());
        }

        // movement types
        file_loader.table_with_headers(MOVEMENT_TYPE_CONFIG, |line: MovementTypeConfig| {
            if result.units.iter().any(|conf| conf.name == line.name) {
                // TODO: error
            }
            result.movement_types.push(line);
            Ok(())
        })?;

        // simple unit data
        file_loader.table_with_headers(UNIT_CONFIG, |line: UnitTypeConfig| {
            if result.units.iter().any(|conf| conf.name == line.name) {
                return Err(ConfigParseError::DuplicateEntry(format!("UnitType::{}", line.name)).into())
            }
            result.max_transported = result.max_transported.max(line.transport_capacity);
            result.units.push(line);
            Ok(())
        })?;
        for conf in &result.units {
            file_loader.unit_types.push(conf.name.clone());
        }
        match result.units.iter().position(|conf| conf.name == unknown_unit) {
            Some(i) => result.unknown_unit = UnitType(i),
            None => return Err(Box::new(ConfigParseError::MissingUnit("Unknown".to_string())))
        }

        // simple terrain data
        file_loader.table_with_headers(TERRAIN_CONFIG, |line: TerrainTypeConfig| {
            if result.terrains.iter().any(|conf| conf.name == line.name) {
                return Err(ConfigParseError::DuplicateEntry(format!("TerrainType::{}", line.name)).into())
            }
            result.terrains.push(line);
            Ok(())
        })?;
        for conf in &result.terrains {
            file_loader.terrain_types.push(conf.name.clone());
        }
        match result.terrains.iter().position(|conf| conf.name == default_terrain) {
            Some(i) => result.default_terrain = TerrainType(i),
            None => return Err(format!("missing entry in {RULESET_CONFIG}: 'DefaultTerrain'").into())
        }

        // simple token data
        file_loader.table_with_headers(TOKEN_CONFIG, |line: TokenTypeConfig| {
            if result.tokens.iter().any(|conf| conf.name == line.name) {
                return Err(ConfigParseError::DuplicateEntry(format!("TokenType::{}", line.name)).into())
            }
            result.tokens.push(line);
            Ok(())
        })?;
        for conf in &result.tokens {
            file_loader.token_types.push(conf.name.clone());
        }

        // simple effect data
        result.effect_types.push(EffectConfig {
            name: "GLITCH".to_string(),
            is_global: true,
            data_type: None,
            visibility: EffectVisibility::CurrentTeam,
        });
        result.effect_types.push(EffectConfig {
            name: "FOG_SURPRISE".to_string(),
            is_global: false,
            data_type: None,
            visibility: EffectVisibility::CurrentTeam,
        });
        result.effect_types.push(EffectConfig {
            name: "UNIT_PATH".to_string(),
            is_global: false,
            data_type: Some(super::effect_config::EffectDataType::Unit),
            visibility: EffectVisibility::Data,
        });
        file_loader.table_with_headers(EFFECT_CONFIG, |line: EffectConfig| {
            if result.effect_types.iter().any(|conf| conf.name == line.name) {
                return Err(ConfigParseError::DuplicateEntry(format!("EffectType::{}", line.name)).into())
            }
            result.effect_types.push(line);
            Ok(())
        })?;

        // commanders
        let mut bonus_transported = 0;
        file_loader.table_with_headers(COMMANDER_CONFIG, |line: CommanderTypeConfig| {
            if result.commanders.iter().any(|conf| conf.name == line.name) {
                // TODO: error
            }
            result.max_commander_charge = result.max_commander_charge.max(line.max_charge);
            bonus_transported = bonus_transported.max(line.transport_capacity as usize);
            result.commanders.push(line);
            Ok(())
        })?;
        result.max_transported += bonus_transported;
        for commander in &result.commanders {
            file_loader.commander_types.push(commander.name.clone());
        }
        if result.commanders.len() == 0 {
            // TODO: error
        }

        // simple hero data
        file_loader.table_with_headers(HERO_CONFIG, |line: HeroTypeConfig| {
            if result.heroes.iter().any(|conf| conf.name == line.name) {
                // TODO: error
            }
            if line.charge > i8::MAX as u8 {
                return Err(Box::new(ConfigParseError::HeroMaxChargeExceeded(i8::MAX as u8)));
            }
            result.max_hero_charge = result.max_hero_charge.max(line.charge);
            result.max_hero_transport_bonus = result.max_hero_transport_bonus.max(line.transport_capacity as usize);
            result.heroes.push(line);
            Ok(())
        })?;
        result.max_transported += result.max_hero_transport_bonus;
        for hero in &result.heroes {
            file_loader.hero_types.push(hero.name.clone());
        }

        // unit transport
        let data = file_loader.load_config(UNIT_TRANSPORT)?;
        let mut reader = csv::ReaderBuilder::new().delimiter(b';').from_reader(data.as_bytes());
        let mut transported: Vec<UnitType> = Vec::new();
        for h in reader.headers()?.into_iter().skip(1) {
            let header = UnitType::from_conf(h, &mut file_loader)?.0;
            if transported.contains(&header) {
                return Err(Box::new(ConfigParseError::DuplicateHeader(h.to_string())))
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

        // hero powers
        file_loader.table_with_headers(HERO_POWERS, |line: HeroPowerConfig| {
            result.heroes[line.hero.0].powers.push(line);
            Ok(())
        })?;
        for hero in &result.heroes {
            if hero.powers.len() == 0 {
                // TODO: error
            }
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

        for (i, conf) in result.movement_types.iter().enumerate() {
            if conf.sub_types.len() < 2 {
                continue;
            }
            let data = file_loader.load_config(&format!("{SUB_MOVEMENT_TYPE_CONFIG}{}.csv", conf.name))?;
            let mut reader = csv::ReaderBuilder::new().delimiter(b';').from_reader(data.as_bytes());
            // TODO: ensure uniqueness of column and row IDs
            let mut headers: Vec<MovementType> = Vec::new();
            for h in reader.headers()?.into_iter().skip(1) {
                let header = MovementType::from_conf(h, &mut file_loader)?.0;
                if headers.contains(&header) {
                    return Err(Box::new(ConfigParseError::DuplicateHeader(h.to_string())))
                }
                headers.push(header);
            }
            let mut map = HashMap::default();
            for line in reader.records() {
                let line = line?;
                let mut line = line.into_iter();
                let terrain: TerrainType = match line.next() {
                    Some(t) => TerrainType::from_conf(t, &mut file_loader)?.0,
                    _ => continue,
                };
                for (i, val) in line.enumerate() {
                    if val.len() > 0 && i < headers.len() {
                        let movement_type = MovementType::from_conf(val, &mut file_loader)?.0;
                        if movement_type != headers[i] {
                            map.insert((terrain, headers[i]), movement_type);
                        }
                    }
                }
            }
            if map.len() == 0 {
                // TODO: return error?
            }
            result.movement_type_transformer.insert(MovementType(i), map);
        }

        // commander powers
        file_loader.table_with_headers(COMMANDER_POWERS, |line: CommanderPowerConfig| {
            result.commanders[line.id.0].powers.push(line);
            Ok(())
        })?;
        for commander in &result.commanders {
            if commander.powers.len() == 0 {
                // TODO: error
            }
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

        // terrain overrides, has to be after commander and hero parsing
        file_loader.table_with_headers(GLOBAL_EVENTS, |line: GlobalEventConfig| {
            result.global_events.push(line);
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
            result.custom_tables.insert(conf.id, table);
        }

        // editor tags
        [result.terrain_flags, result.terrain_tags] = editor_tag_config::parse(TERRAIN_TAGS, &mut file_loader)?;
        [result.token_flags, result.token_tags] = editor_tag_config::parse(TOKEN_TAGS, &mut file_loader)?;
        [result.unit_flags, result.unit_tags] = editor_tag_config::parse(UNIT_TAGS, &mut file_loader)?;

        let (asts, functions) = file_loader.finish();
        result.asts = asts;
        result.functions = functions;
        result.effect_modules.push(effect_constructor_module::<Direction4>(&result.effect_types));
        result.effect_modules.push(effect_constructor_module::<Direction6>(&result.effect_types));

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
