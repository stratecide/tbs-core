use std::collections::HashMap;
use std::collections::HashSet;
use std::error::Error;
use std::sync::Arc;

use interfaces::*;
use num_rational::Rational32;
use semver::Version;

use crate::game::fog::VisionMode;
use crate::commander::commander_type::CommanderType;
use crate::game::game_view::GameView;
use crate::game::import_client;
use crate::game::import_server;
use crate::game::settings::GameConfig;
use crate::game::GameType;
use crate::map::direction::Direction;
use crate::map::map::import_map;
use crate::map::map::MapType;
use crate::map::point::Point;
use crate::script::attack::AttackScript;
use crate::script::death::DeathScript;
use crate::script::defend::DefendScript;
use crate::script::kill::KillScript;
use crate::script::terrain::TerrainScript;
use crate::script::unit::UnitScript;
use crate::terrain::terrain::Terrain;
use crate::terrain::AmphibiousTyping;
use crate::terrain::ExtraMovementOptions;
use crate::terrain::TerrainType;
use crate::terrain::attributes::TerrainAttributeKey;
use crate::units::combat::*;
use crate::units::movement::MovementType;
use crate::units::movement::TBallast;
use crate::units::unit::Unit;
use crate::units::unit_types::UnitType;
use crate::units::attributes::*;
use crate::units::hero::*;
use crate::VERSION;

use super::custom_action_config::CustomActionConfig;
use super::hero_power_config::HeroPowerConfig;
use super::hero_type_config::HeroTypeConfig;
use super::commander_power_config::CommanderPowerConfig;
use super::commander_type_config::CommanderTypeConfig;
use super::commander_unit_config::CommanderPowerUnitConfig;
use super::movement_type_config::MovementPattern;
use super::number_modification::NumberMod;
use super::terrain_powered::TerrainPoweredConfig;
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
    pub(super) unit_status: HashMap<UnitType, Vec<ActionStatus>>,
    pub(super) attack_damage: HashMap<UnitType, HashMap<UnitType, u16>>,
    pub(super) custom_actions: Vec<CustomActionConfig>,
    pub(super) max_transported: usize,
    // heroes
    pub(super) hero_types: Vec<HeroType>,
    pub(super) heroes: HashMap<HeroType, HeroTypeConfig>,
    pub(super) hero_units: HashMap<HeroType, HashSet<UnitType>>,
    pub(super) hero_powers: HashMap<HeroType, Vec<HeroPowerConfig>>,
    //pub(super) hero_powered_units: HashMap<HeroType, HashMap<Option<bool>, Vec<CommanderPowerUnitConfig>>>,
    pub(super) max_hero_charge: u8,
    pub(super) max_aura_range: i8,
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
    // detail
    pub(super) max_sludge: u8,
    // commanders
    pub(super) commander_types: Vec<CommanderType>,
    pub(super) commanders: HashMap<CommanderType, CommanderTypeConfig>,
    pub(super) commander_powers: HashMap<CommanderType, Vec<CommanderPowerConfig>>,
    pub(super) default_terrain_overrides: Vec<TerrainPoweredConfig>,
    pub(super) commander_terrain: HashMap<CommanderType, HashMap<Option<u8>, Vec<TerrainPoweredConfig>>>,
    pub(super) default_unit_overrides: Vec<CommanderPowerUnitConfig>,
    pub(super) commander_units: HashMap<CommanderType, HashMap<Option<u8>, Vec<CommanderPowerUnitConfig>>>,
    pub(super) commander_unit_attributes: HashMap<CommanderType, Vec<(UnitTypeFilter, Vec<AttributeKey>, Vec<AttributeKey>)>>,
    pub(super) max_commander_charge: u32,
}

impl ConfigInterface for Config {
    fn get_name(&self) -> &str {
        &self.name
    }

    fn parse_map(self: Arc<Self>, bytes: Vec<u8>) -> Result<Box<dyn MapInterface>, Box<dyn Error>> {
        match import_map(&self, bytes, Version::parse(VERSION)?)? {
            MapType::Hex(map) => Ok(Box::new(map)),
            MapType::Square(map) => Ok(Box::new(map)),
        }
    }

    fn parse_game_settings(self: Arc<Self>, bytes: Vec<u8>) -> Result<Box<dyn GameSettingsInterface>, Box<dyn Error>> {
        Ok(Box::new(GameConfig::import(self, bytes)?))
    }

    fn parse_server(self: Arc<Self>, data: ExportedGame) -> Result<Box<dyn GameInterface>, Box<dyn Error>> {
        match import_server(&self, data, Version::parse(VERSION)?)? {
            GameType::Hex(game) => Ok(Box::new(game)),
            GameType::Square(game) => Ok(Box::new(game)),
        }
    }

    fn parse_client(self: Arc<Self>, public: Vec<u8>, secret: Option<(Team, Vec<u8>)>) -> Result<Box<dyn GameInterface>, Box<dyn Error>> {
        match import_client(&self, public, secret, Version::parse(VERSION)?)? {
            GameType::Hex(game) => Ok(Box::new(game)),
            GameType::Square(game) => Ok(Box::new(game)),
        }
    }
}

impl Config {
    pub fn max_player_count(&self) -> i8 {
        16
    }

    pub fn max_sludge(&self) -> u8 {
        // TODO: parse from config. currently is just set to a fixed value
        self.max_sludge
    }

    pub fn max_unit_level(&self) -> u8 {
        3
    }

    pub fn max_aura_range(&self) -> i8 {
        self.max_aura_range
    }

    // units

    pub fn unit_count(&self) -> usize {
        self.units.len()
    }

    pub fn unit_types(&self) -> &[UnitType] {
        &self.unit_types
    }

    pub fn max_transported(&self) -> usize {
        self.max_transported
    }

    pub fn unit_max_transport_capacity(&self, typ: UnitType) -> usize {
        self.unit_config(typ).transport_capacity
        + self.commanders.values().map(|c| c.transport_capacity as usize).max().unwrap_or(0)
        + self.heroes.iter()
        .filter(|(hero, _)| self.hero_units.get(*hero).unwrap().contains(&typ))
        .map(|(_, c)| c.transport_capacity as usize).max().unwrap_or(0)
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

    pub fn base_movement_points(&self, typ: UnitType) -> Rational32 {
        self.unit_config(typ).movement_points
    }

    pub fn has_stealth(&self, typ: UnitType) -> bool {
        self.unit_config(typ).stealthy
    }

    pub fn can_be_moved_through(&self, typ: UnitType) -> bool {
        self.unit_config(typ).can_be_moved_through
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

    pub fn default_attack_pattern(&self, typ: UnitType) -> AttackType {
        self.unit_config(typ).attack_pattern
    }

    pub fn attack_targeting(&self, typ: UnitType) -> AttackTargeting {
        self.unit_config(typ).attack_targets
    }

    pub fn base_damage(&self, attacker: UnitType, defender: UnitType) -> Option<u16> {
        self.attack_damage.get(&attacker)?.get(&defender).cloned()
    }

    pub fn base_cost(&self, typ: UnitType) -> i32 {
        self.unit_config(typ).cost as i32
    }

    pub fn displacement(&self, typ: UnitType) -> Displacement {
        self.unit_config(typ).displacement
    }

    pub fn base_displacement_distance(&self, typ: UnitType) -> i8 {
        self.unit_config(typ).displacement_distance
    }

    pub fn can_be_displaced(&self, typ: UnitType) -> bool {
        self.unit_config(typ).can_be_displaced
    }

    pub fn vision_mode(&self, typ: UnitType) -> VisionMode {
        self.unit_config(typ).vision_mode
    }

    pub fn base_vision_range(&self, typ: UnitType) -> usize {
        self.unit_config(typ).vision
    }

    pub fn base_true_vision_range(&self, typ: UnitType) -> usize {
        self.unit_config(typ).true_vision
    }

    pub fn unit_specific_attributes(&self, typ: UnitType) -> &[AttributeKey] {
        self.unit_attributes.get(&typ).expect(&format!("Environment doesn't contain unit type {typ:?}"))
    }

    pub(crate) fn unit_specific_hidden_attributes(&self, typ: UnitType) -> &[AttributeKey] {
        self.unit_hidden_attributes.get(&typ).expect(&format!("Environment doesn't contain unit type {typ:?}"))
    }

    pub fn unit_specific_statuses(&self, typ: UnitType) -> &[ActionStatus] {
        self.unit_status.get(&typ).map(|v| v.as_slice()).unwrap_or(&[ActionStatus::Ready])
    }

    pub fn unit_transportable(&self, typ: UnitType) -> &[UnitType] {
        if let Some(transportable) = self.unit_transports.get(&typ) {
            transportable
        } else {
            &[]
        }
    }

    pub fn custom_actions(&self) -> &[CustomActionConfig] {
        &self.custom_actions
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

    pub fn hero_name(&self, typ: HeroType) -> &str {
        &self.hero_config(typ).name
    }

    pub fn hero_price(&self, typ: HeroType, unit: UnitType) -> Option<i32> {
        if self.hero_units.get(&typ)?.contains(&unit) {
            Some(self.hero_config(typ).price.update_value(self.base_cost(unit)))
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

    pub fn hero_transport_capacity(&self, typ: HeroType) -> u8 {
        self.hero_config(typ).transport_capacity
    }

    pub fn hero_powers(&self, typ: HeroType) -> &[HeroPowerConfig] {
        if let Some(powers) = self.hero_powers.get(&typ) {
            powers
        } else {
            &[]
        }
    }

    pub fn hero_can_gain_charge(&self, typ: HeroType, power: usize) -> bool {
        self.hero_powers.get(&typ)
        .and_then(|powers| powers.get(power))
        .map(|power| !power.prevents_charging)
        .unwrap_or(false)
    }

    pub fn hero_aura_range<D: Direction>(
        &self,
        map: &impl GameView<D>,
        unit: &Unit<D>,
        unit_pos: Point,
        transporter: Option<(&Unit<D>, usize)>,
    ) -> Option<usize> {
        let hero = unit.get_hero();
        let iter = self.unit_power_configs(
            map,
            unit,
            (unit_pos, transporter.map(|u| u.1)),
            transporter.map(|u| (u.0, unit_pos)),
            None,
            &[],
            &[],
            false,
        );
        let result = if transporter.is_none() {
            let aura_range = self.hero_powers.get(&hero.typ())?.get(hero.get_active_power())?.aura_range;
            NumberMod::update_value_repeatedly(
                aura_range,
                iter.map(|c| &c.aura_range)
            )
        } else {
            let aura_range = self.hero_powers.get(&hero.typ())?.get(hero.get_active_power())?.aura_range_transported;
            NumberMod::update_value_repeatedly(
                aura_range,
                iter.map(|c| &c.aura_range_transported)
            )
        };
        if result < 0 {
            None
        } else {
            Some(result as usize)
        }
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

    /**
     * this function could indirectly call itself!
     * checking another terrain's config from game may cause infinite recursion!
     * -> get_terrain has to replace the returned terrain with a "dummy" terrain that doesn't have access to any configs
     */
    pub(super) fn terrain_power_configs<'a, D: Direction>(
        &'a self,
        map: &'a impl GameView<D>,
        pos: Point,
        terrain: &'a Terrain,
        is_bubble: bool,
        // the heroes affecting this terrain. shouldn't be taken from game since they could have died before this function is called
        heroes: &'a [HeroInfluence<D>],
    ) -> impl DoubleEndedIterator<Item = &'a TerrainPoweredConfig> {
        let commander = terrain.get_commander(map);
        let mut slices = vec![&self.default_terrain_overrides];
        // should always be true
        if let Some(configs) = self.commander_terrain.get(&commander.typ()) {
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
            config.affects.iter().all(|filter| filter.check(map, pos, terrain, is_bubble, heroes))
        })
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

    pub fn terrain_vision_range<D: Direction>(
        &self,
        map: &impl GameView<D>,
        pos: Point,
        terrain: &Terrain,
        // the heroes affecting this terrain. shouldn't be taken from game since they could have died before this function is called
        heroes: &[HeroInfluence<D>],
    ) -> Option<usize> {
        let iter = self.terrain_power_configs(map, pos, terrain, false, heroes)
        .map(|c| &c.vision);
        let mut result = NumberMod::update_value_repeatedly(
            self.terrain_config(terrain.typ()).vision_range,
            iter,
        ) as i8;
        if result < 0 && self.terrain_can_build(map, pos, terrain, heroes) {
            result = 0;
        }
        if result < 0 {
            None
        } else {
            Some(result as usize)
        }
    }

    pub fn terrain_income_factor(&self, typ: TerrainType) -> i16 {
        self.terrain_config(typ).income_factor
    }

    pub fn terrain_can_build_base(&self, typ: TerrainType) -> bool {
        self.terrain_config(typ).can_build
    }

    pub fn terrain_can_build<D: Direction>(
        &self,
        map: &impl GameView<D>,
        pos: Point,
        terrain: &Terrain,
        heroes: &[HeroInfluence<D>],
    ) -> bool {
        let mut result = self.terrain_can_build_base(terrain.typ());
        for config in self.terrain_power_configs(map, pos, terrain, false, heroes) {
            if let Some(can_build) = config.build {
                result = can_build;
            }
        }
        result
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

    pub fn terrain_specific_attributes(&self, typ: TerrainType) -> &[TerrainAttributeKey] {
        self.terrain_attributes.get(&typ).expect(&format!("Environment doesn't contain terrain type {typ:?}"))
    }

    pub(crate) fn terrain_specific_hidden_attributes(&self, typ: TerrainType) -> &[TerrainAttributeKey] {
        self.terrain_hidden_attributes.get(&typ).expect(&format!("Environment doesn't contain terrain type {typ:?}"))
    }

    pub fn terrain_unit_attribute_overrides<D: Direction>(
        &self,
        _game: &impl GameView<D>,
        terrain: &Terrain,
        _pos: Point,
        _heroes: &[HeroInfluence<D>],
    ) -> HashMap<AttributeKey, AttributeOverride> {
        let mut result = HashMap::new();
        for ov in &self.terrain_config(terrain.typ()).build_overrides {
            result.insert(ov.key(), ov.clone());
        }
        result
    }

    pub fn terrain_on_build<D: Direction>(
        &self,
        map: &impl GameView<D>,
        pos: Point,
        terrain: &Terrain,
        is_bubble: bool,
        heroes: &[HeroInfluence<D>],
    ) -> Vec<TerrainScript> {
        let mut result = Vec::new();
        for config in self.terrain_power_configs(map, pos, terrain, is_bubble, heroes) {
            result.extend(config.on_build.iter().cloned())
        }
        result
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

    pub fn commander_attributes(&self, typ: CommanderType, unit: UnitType) -> &[AttributeKey] {
        if let Some(attributes) = self.commander_unit_attributes.get(&typ) {
            for (filter, attributes, _) in attributes {
                if filter.check(self, unit) {
                    return &attributes;
                }
            }
        }
        &[]
    }

    pub fn commander_attributes_hidden_by_fog(&self, typ: CommanderType, unit: UnitType) -> &[AttributeKey] {
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

    pub fn commander_can_gain_charge(&self, typ: CommanderType, power: usize) -> bool {
        self.commander_powers.get(&typ)
        .and_then(|powers| powers.get(power))
        .map(|power| !power.prevents_charging)
        .unwrap_or(false)
    }

    // commander unit

    /**
     * this function can indirectly call itself, if
     *      - some config of other_unit, transporter or a hero is filtered for
     *      - the filter takes a unit from game and wants to check one of its configs
     * checking a unit config from game may cause infinite recursion!
     * -> get_unit has to replace the returned unit with a "dummy" unit that doesn't have access to any configs (not through its hero either)
     */
    pub(super) fn unit_power_configs<'a, D: Direction>(
        &'a self,
        map: &'a impl GameView<D>,
        unit: &'a Unit<D>,
        unit_pos: (Point, Option<usize>),
        // when moving out of a transporter, or start_turn for transported units
        transporter: Option<(&'a Unit<D>, Point)>,
        // the attacked unit, the unit this one was destroyed by, ...
        other_unit: Option<(&'a Unit<D>, Point)>,
        // the heroes affecting this unit. shouldn't be taken from game since they could have died before this function is called
        heroes: &'a [HeroInfluence<D>],
        // empty if the unit hasn't moved
        temporary_ballast: &'a [TBallast<D>],
        is_counter: bool,
    ) -> impl DoubleEndedIterator<Item = &'a CommanderPowerUnitConfig> {
        let commander = unit.get_commander(map);
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
            config.affects.iter().all(|filter| filter.check(map, unit, unit_pos, transporter, other_unit, heroes, temporary_ballast, is_counter))
        })
    }

    pub fn unit_cost<D: Direction>(
        &self,
        game: &impl GameView<D>,
        unit: &Unit<D>,
        unit_pos: Point,
        factory_unit: Option<&Unit<D>>, // if built by a unit
        heroes: &[HeroInfluence<D>],
    ) -> i32 {
        let iter = self.unit_power_configs(game, unit, (unit_pos, None), factory_unit.map(|u| (u, unit_pos)), None, heroes, &[], false);
        NumberMod::update_value_repeatedly(
            self.base_cost(unit.typ()),
            iter.map(|c| &c.cost)
        )
    }

    pub fn unit_visibility<D: Direction>(
        &self,
        game: &impl GameView<D>,
        unit: &Unit<D>,
        unit_pos: Point,
        //transporter: Option<(&Unit<D>, Point)>,
        heroes: &[HeroInfluence<D>],
    ) -> UnitVisibility {
        let mut result = self.unit_config(unit.typ()).visibility;
        for config in self.unit_power_configs(game, unit, (unit_pos, None), None, None, heroes, &[], false) {
            if let Some(visibility) = config.visibility {
                result = visibility;
            }
        }
        result
    }

    pub fn unit_vision<D: Direction>(
        &self,
        game: &impl GameView<D>,
        unit: &Unit<D>,
        unit_pos: Point,
        heroes: &[HeroInfluence<D>],
    ) -> usize {
        let iter = self.unit_power_configs(game, unit, (unit_pos, None), None, None, heroes, &[], false)
        .map(|c| &c.vision);
        NumberMod::update_value_repeatedly(
            self.base_vision_range(unit.typ()) as u8,
            iter,
        ) as usize
    }

    pub fn unit_true_vision<D: Direction>(
        &self,
        game: &impl GameView<D>,
        unit: &Unit<D>,
        unit_pos: Point,
        heroes: &[HeroInfluence<D>],
    ) -> usize {
        let iter = self.unit_power_configs(game, unit, (unit_pos, None), None, None, heroes, &[], false)
        .map(|c| &c.true_vision);
        NumberMod::update_value_repeatedly(
            self.base_true_vision_range(unit.typ()) as u8,
            iter,
        ) as usize
    }

    pub fn unit_attribute_overrides<D: Direction>(
        &self,
        game: &impl GameView<D>,
        unit: &Unit<D>,
        unit_pos: Point,
        transporter: Option<(&Unit<D>, Point)>, // move out of this transporter and then build something
        heroes: &[HeroInfluence<D>],
        temporary_ballast: &[TBallast<D>],
    ) -> HashMap<AttributeKey, AttributeOverride> {
        let mut result = HashMap::new();
        for config in self.unit_power_configs(game, unit, (unit_pos, None), transporter, None, heroes, temporary_ballast, false) {
            for ov in &config.build_overrides {
                result.insert(ov.key(), ov.clone());
            }
        }
        result
    }

    pub fn unit_start_turn_effects<D: Direction>(
        &self,
        game: &impl GameView<D>,
        unit: &Unit<D>,
        unit_pos: (Point, Option<usize>),
        transporter: Option<(&Unit<D>, Point)>,
        heroes: &[HeroInfluence<D>],
    ) -> Vec<UnitScript> {
        let mut result = Vec::new();
        for config in self.unit_power_configs(game, unit, unit_pos, transporter, None, heroes, &[], false) {
            result.extend(config.on_start_turn.iter().cloned())
        }
        result
    }

    pub fn unit_end_turn_effects<D: Direction>(
        &self,
        game: &impl GameView<D>,
        unit: &Unit<D>,
        unit_pos: (Point, Option<usize>),
        transporter: Option<(&Unit<D>, Point)>,
        heroes: &[HeroInfluence<D>],
    ) -> Vec<UnitScript> {
        let mut result = Vec::new();
        for config in self.unit_power_configs(game, unit, unit_pos, transporter, None, heroes, &[], false) {
            result.extend(config.on_end_turn.iter().cloned())
        }
        result
    }

    pub fn unit_attack_effects<D: Direction>(
        &self,
        game: &impl GameView<D>,
        unit: &Unit<D>,
        unit_pos: Point,
        defender: &Unit<D>,
        defender_pos: Point,
        transporter: Option<(&Unit<D>, Point)>, // if the attacker moved out of a transporter to attack
        heroes: &[HeroInfluence<D>],
        temporary_ballast: &[TBallast<D>],
        is_counter: bool,
    ) -> Vec<AttackScript> {
        let mut result = Vec::new();
        for config in self.unit_power_configs(game, unit, (unit_pos, None), transporter, Some((defender, defender_pos)), heroes, temporary_ballast, is_counter) {
            result.extend(config.on_attack.iter().cloned())
        }
        result
    }

    pub fn unit_defend_effects<D: Direction>(
        &self,
        game: &impl GameView<D>,
        unit: &Unit<D>,
        unit_pos: Point,
        attacker: &Unit<D>,
        attacker_pos: Point,
        transporter: Option<(&Unit<D>, Point)>, // if the defender moved out of a transporter to attack + defend
        heroes: &[HeroInfluence<D>],
        temporary_ballast: &[TBallast<D>],
        is_counter: bool
    ) -> Vec<DefendScript> {
        let is_counter = temporary_ballast.len() > 0;
        let mut result = Vec::new();
        for config in self.unit_power_configs(game, unit, (unit_pos, None), transporter, Some((attacker, attacker_pos)), heroes, temporary_ballast, is_counter) {
            result.extend(config.on_defend.iter().cloned())
        }
        result
    }

    pub fn unit_kill_effects<D: Direction>(
        &self,
        game: &impl GameView<D>,
        unit: &Unit<D>,
        unit_pos: Point,
        defender: &Unit<D>,
        defender_pos: Point,
        transporter: Option<(&Unit<D>, Point)>, // if the attacker moved out of a transporter to attack
        heroes: &[HeroInfluence<D>],
        temporary_ballast: &[TBallast<D>],
        is_counter: bool,
    ) -> Vec<KillScript> {
        let mut result = Vec::new();
        for config in self.unit_power_configs(game, unit, (unit_pos, None), transporter, Some((defender, defender_pos)), heroes, temporary_ballast, is_counter) {
            result.extend(config.on_kill.iter().cloned())
        }
        result
    }

    pub fn unit_death_effects<D: Direction>(
        &self,
        game: &impl GameView<D>,
        unit: &Unit<D>,
        unit_pos: (Point, Option<usize>),
        transporter: Option<(&Unit<D>, Point)>,
        attacker: Option<(&Unit<D>, Point)>,
        heroes: &[HeroInfluence<D>],
        temporary_ballast: &[TBallast<D>],
    ) -> Vec<DeathScript> {
        let mut result = Vec::new();
        for config in self.unit_power_configs(game, unit, unit_pos, transporter, attacker, heroes, temporary_ballast, false) {
            result.extend(config.on_death.iter().cloned())
        }
        result
    }

    pub fn unit_movement_points<D: Direction>(
        &self,
        game: &impl GameView<D>,
        unit: &Unit<D>,
        unit_pos: (Point, Option<usize>),
        transporter: Option<(&Unit<D>, Point)>,
        heroes: &[HeroInfluence<D>],
    ) -> Rational32 {
        let iter = self.unit_power_configs(game, unit, unit_pos, transporter, None, heroes, &[], false)
        .map(|c| &c.movement_points);
        NumberMod::update_value_repeatedly(
            self.base_movement_points(unit.typ()),
            iter,
        )
    }

    pub fn unit_attack_pattern<D: Direction>(
        &self,
        game: &impl GameView<D>,
        unit: &Unit<D>,
        unit_pos: Point,
        counter: Counter<D>,
        heroes: &[HeroInfluence<D>],
        temporary_ballast: &[TBallast<D>],
    ) -> AttackType {
        let mut result = self.default_attack_pattern(unit.typ());
        for conf in self.unit_power_configs(
            game,
            unit,
            (unit_pos, None),
            None,
            counter.attacker(),
            heroes,
            temporary_ballast,
            counter.is_counter(),
        ) {
            if let Some(pattern) = conf.attack_pattern {
                result = pattern;
            }
        }
        result
    }

    pub fn unit_attack<D: Direction>(
        &self,
        game: &impl GameView<D>,
        unit: &Unit<D>,
        unit_pos: Point,
        defender: &Unit<D>,
        defender_pos: Point,
        heroes: &[HeroInfluence<D>],
        temporary_ballast: &[TBallast<D>],
        is_counter: bool,
    ) -> Rational32 {
        let iter = || self.unit_power_configs(
            game,
            unit,
            (unit_pos, None),
            None,
            Some((defender, defender_pos)),
            heroes,
            temporary_ballast,
            is_counter,
        );
        let factor = NumberMod::update_value_repeatedly(
            Rational32::from_integer(1),
            iter().map(|c| &c.attack)
        );
        // attack is reduced by the damage the attacker has already taken
        let damage_factor = NumberMod::update_value_repeatedly(
            Rational32::from_integer(1),
            iter().map(|c| &c.attack_reduced_by_damage)
        );
        let damage = Rational32::from_integer(100 - unit.get_hp() as i32);
        let hp_factor = (Rational32::from_integer(100) - damage * damage_factor) / 100;
        hp_factor * factor
    }

    pub fn unit_defense<D: Direction>(
        &self,
        game: &impl GameView<D>,
        unit: &Unit<D>,
        unit_pos: Point,
        attacker: &Unit<D>,
        attacker_pos: Point,
        heroes: &[HeroInfluence<D>],
        is_counter: bool,
    ) -> Rational32 {
        let iter = self.unit_power_configs(
            game,
            unit,
            (unit_pos, None),
            None,
            Some((attacker, attacker_pos)),
            heroes,
            &[],
            is_counter,
        );
        NumberMod::update_value_repeatedly(
            Rational32::from_integer(1),
            iter.map(|c| &c.defense)
        )
    }

    pub fn unit_range<D: Direction>(
        &self,
        game: &impl GameView<D>,
        unit: &Unit<D>,
        unit_pos: Point,
        transporter: Option<(&Unit<D>, Point)>,
        heroes: &[HeroInfluence<D>],
        temporary_ballast: &[TBallast<D>],
        min_range: bool,
        base_range: u8,
        is_counter: bool,
    ) -> u8 {
        let iter = self.unit_power_configs(
            game,
            unit,
            (unit_pos, None),
            transporter,
            None,
            heroes,
            temporary_ballast,
            is_counter,
        );
        if min_range {
            NumberMod::update_value_repeatedly(
                base_range,
                iter.map(|c| &c.min_range)
            )
        } else {
            NumberMod::update_value_repeatedly(
                base_range,
                iter.map(|c| &c.max_range)
            )
        }
    }

    pub fn unit_displacement_distance<D: Direction>(
        &self,
        game: &impl GameView<D>,
        unit: &Unit<D>,
        unit_pos: Point,
        transporter: Option<(&Unit<D>, Point)>,
        heroes: &[HeroInfluence<D>],
        temporary_ballast: &[TBallast<D>],
        is_counter: bool,
    ) -> i8 {
        let iter = self.unit_power_configs(
            game,
            unit,
            (unit_pos, None),
            transporter,
            None,
            heroes,
            temporary_ballast,
            is_counter,
        );
        let base_displacement = self.base_displacement_distance(unit.typ());
        // manipulating the absolute value is more intuitive
        // but that means the sign has to be multiplied with at the end
        let sign = if base_displacement < 0 {
            -1
        } else {
            1
        };
        NumberMod::update_value_repeatedly(
            base_displacement.abs(),
            iter.map(|c| &c.displacement_distance)
        ) * sign
    }

    pub fn unit_splash_damage<D: Direction>(
        &self,
        game: &impl GameView<D>,
        unit: &Unit<D>,
        unit_pos: Point,
        heroes: &[HeroInfluence<D>],
        temporary_ballast: &[TBallast<D>],
        is_counter: bool,
    ) -> Vec<Rational32> {
        let mut result: &[Rational32] = &[];
        for config in self.unit_power_configs(
            game,
            unit,
            (unit_pos, None),
            None,
            None,
            heroes,
            temporary_ballast,
            is_counter,
        ) {
            if config.splash_damage.len() > 0 {
                result = config.splash_damage.as_slice();
            }
        }
        if result.len() == 0 {
            result = &self.unit_config(unit.typ()).splash_damage;
        }
        if result.len() == 0 {
            result = &DEFAULT_SPLASH
        }
        result.to_vec()
    }

}
