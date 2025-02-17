use rustc_hash::FxHashMap as HashMap;
use std::error::Error;
use std::sync::Arc;

use interfaces::*;
use num_rational::Rational32;
use rhai::*;
use semver::Version;

use crate::combat::*;
use crate::game::event_fx::EffectType;
use crate::tokens::token_types::TokenType;
use crate::game::fog::VisionMode;
use crate::commander::commander_type::CommanderType;
use crate::game::game::Game;
use crate::game::game_view::GameView;
use crate::game::modified_view::UnitMovementView;
use crate::game::{import_client, import_server};
use crate::game::settings::GameConfig;
use crate::game::GameType;
use crate::handle::Handle;
use crate::map::direction::Direction;
use crate::map::map::import_map;
use crate::map::map::MapType;
use crate::map::point::Point;
use crate::script::executor::Executor;
use crate::script::*;
use crate::terrain::terrain::Terrain;
use crate::terrain::*;
use crate::units::UnitVisibility;
use crate::units::movement::*;
use crate::units::unit::Unit;
use crate::units::unit_types::UnitType;
use crate::units::hero::*;
use crate::VERSION;

use super::attack_config::{AttackConfig, AttackSplashConfig};
use super::attack_powered::AttackPoweredConfig;
use super::custom_action_config::CustomActionConfig;
use super::editor_tag_config::TagEditorVisibility;
use super::effect_config::{EffectConfig, EffectDataType, EffectVisibility};
use super::global_events::GlobalEventConfig;
use super::hero_power_config::HeroPowerConfig;
use super::hero_type_config::HeroTypeConfig;
use super::commander_power_config::CommanderPowerConfig;
use super::commander_type_config::CommanderTypeConfig;
use super::commander_unit_config::CommanderPowerUnitConfig;
use super::movement_type_config::{MovementPattern, MovementTypeConfig};
use super::number_modification::NumberMod;
use super::table_config::CustomTable;
use super::tag_config::{TagConfig, TagType};
use super::terrain_powered::TerrainPoweredConfig;
use super::terrain_type_config::TerrainTypeConfig;
use super::token_typ_config::TokenTypeConfig;
use super::unit_type_config::UnitTypeConfig;
use super::OwnershipPredicate;

pub struct Config {
    pub(super) name: String,
    pub(super) owner_colors: Vec<[u8; 4]>,
    // tags
    pub(super) flags: Vec<TagConfig>,
    pub(super) tags: Vec<TagConfig>,
    pub(super) movement_types: Vec<MovementTypeConfig>,
    pub(super) movement_type_transformer: HashMap<MovementType, HashMap<(TerrainType, MovementType), MovementType>>,
    // units
    pub(super) units: Vec<UnitTypeConfig>,
    pub(super) unknown_unit: UnitType,
    pub(super) unit_transports: HashMap<UnitType, Vec<UnitType>>,
    //pub(super) attack_damage: HashMap<UnitType, HashMap<UnitType, u16>>,
    pub(super) custom_actions: Vec<CustomActionConfig>,
    pub(super) max_transported: usize,
    pub(super) unit_flags: HashMap<(usize, UnitType), TagEditorVisibility>,
    pub(super) unit_tags: HashMap<(usize, UnitType), TagEditorVisibility>,
    pub(super) attack_types: Vec<(String, Vec<AttackConfig>)>,
    pub(super) splash_types: Vec<(String, Vec<AttackSplashConfig>)>,
    pub(super) attack_overrides: Vec<AttackPoweredConfig>,
    // heroes
    pub(super) heroes: Vec<HeroTypeConfig>,
    pub(super) max_hero_charge: u8,
    pub(super) max_aura_range: i8,
    pub(super) max_hero_transport_bonus: usize,
    // terrain
    pub(super) terrains: Vec<TerrainTypeConfig>,
    pub(super) default_terrain: TerrainType,
    pub(super) movement_cost: HashMap<TerrainType, HashMap<MovementType, Rational32>>,
    pub(super) terrain_flags: HashMap<(usize, TerrainType), TagEditorVisibility>,
    pub(super) terrain_tags: HashMap<(usize, TerrainType), TagEditorVisibility>,
    // detail
    pub(super) tokens: Vec<TokenTypeConfig>,
    pub(super) token_flags: HashMap<(usize, TokenType), TagEditorVisibility>,
    pub(super) token_tags: HashMap<(usize, TokenType), TagEditorVisibility>,
    // effects
    pub(super) effect_types: Vec<EffectConfig>,
    // commanders
    pub(super) commanders: Vec<CommanderTypeConfig>,
    pub(super) terrain_overrides: Vec<TerrainPoweredConfig>,
    pub(super) unit_overrides: Vec<CommanderPowerUnitConfig>,
    pub(super) max_commander_charge: u32,
    // global events, shared by terrain, units, commanders, ...
    pub(crate) global_events: Vec<GlobalEventConfig>,
    // rhai
    //pub(super) global_ast: AST,
    pub(super) my_package_4: MyPackage4,
    pub(super) my_package_6: MyPackage6,
    pub(super) global_module: Shared<Module>,
    pub(super) effect_modules: Vec<Shared<Module>>,
    pub(super) global_constants: Scope<'static>,
    pub(super) asts: Vec<AST>,
    pub(super) functions: Vec<(usize, String)>,
    pub(super) is_unit_dead_rhai: usize,
    pub(super) is_unit_movable_rhai: usize,
    pub(super) calculate_damage_rhai: usize,
    pub(super) deal_damage_rhai: usize,
    pub(super) weapon_effects_rhai: Option<usize>,
    pub(super) custom_tables: HashMap<String, CustomTable>,
}

impl ConfigInterface for Config {
    fn get_name(&self) -> &str {
        &self.name
    }

    fn parse_map(self: Arc<Self>, bytes: Vec<u8>) -> Result<Box<dyn MapInterface>, Box<dyn Error>> {
        match import_map(&self, bytes, Version::parse(VERSION)?)? {
            MapType::Hex(map) => Ok(Box::new(Handle::new(map))),
            MapType::Square(map) => Ok(Box::new(Handle::new(map))),
        }
    }

    fn parse_game_settings(self: Arc<Self>, bytes: Vec<u8>) -> Result<Box<dyn GameSettingsInterface>, Box<dyn Error>> {
        Ok(Box::new(GameConfig::import(self, bytes)?))
    }

    fn parse_server(self: Arc<Self>, data: ExportedGame) -> Result<Box<dyn GameInterface>, Box<dyn Error>> {
        match import_server(&self, data, Version::parse(VERSION)?)? {
            GameType::Hex(game) => Ok(Box::new(Handle::new(game))),
            GameType::Square(game) => Ok(Box::new(Handle::new(game))),
        }
    }

    fn parse_client(self: Arc<Self>, public: Vec<u8>, secret: Option<(Team, Vec<u8>)>) -> Result<Box<dyn GameInterface>, Box<dyn Error>> {
        match import_client(&self, public, secret, Version::parse(VERSION)?)? {
            GameType::Hex(game) => Ok(Box::new(Handle::new(game))),
            GameType::Square(game) => Ok(Box::new(Handle::new(game))),
        }
    }
}

impl Config {
    pub fn max_player_count(&self) -> i8 {
        16
    }

    pub fn max_aura_range(&self) -> i8 {
        self.max_aura_range
    }

    // flags / tags

    pub fn flag_count(&self) -> usize {
        self.flags.len()
    }
    pub fn flag_visibility(&self, index: usize) -> UnitVisibility {
        self.flags[index].visibility
    }
    pub fn flag_name(&self, index: usize) -> &str {
        &self.flags[index].name
    }
    pub fn flag_by_name(&self, name: &str) -> Option<usize> {
        self.flags.iter().position(|flag| flag.name.as_str() == name)
    }

    pub fn tag_count(&self) -> usize {
        self.tags.len()
    }
    pub fn tag_type(&self, index: usize) -> &TagType {
        &self.tags[index].tag_type
    }
    pub fn tag_visibility(&self, index: usize) -> UnitVisibility {
        self.tags[index].visibility
    }
    pub fn tag_name(&self, index: usize) -> &str {
        &self.tags[index].name
    }
    pub fn tag_by_name(&self, name: &str) -> Option<usize> {
        self.tags.iter().position(|tag| tag.name.as_str() == name)
    }

    pub fn is_terrain_flag_normal(&self, typ: TerrainType, flag: usize) -> TagEditorVisibility {
        self.terrain_flags.get(&(flag, typ)).cloned().unwrap_or(TagEditorVisibility::Hidden)
    }
    pub fn is_terrain_tag_normal(&self, typ: TerrainType, tag: usize) -> TagEditorVisibility {
        self.terrain_tags.get(&(tag, typ)).cloned().unwrap_or(TagEditorVisibility::Hidden)
    }
    pub fn is_token_flag_normal(&self, typ: TokenType, flag: usize) -> TagEditorVisibility {
        self.token_flags.get(&(flag, typ)).cloned().unwrap_or(TagEditorVisibility::Hidden)
    }
    pub fn is_token_tag_normal(&self, typ: TokenType, tag: usize) -> TagEditorVisibility {
        self.token_tags.get(&(tag, typ)).cloned().unwrap_or(TagEditorVisibility::Hidden)
    }
    pub fn is_unit_flag_normal(&self, typ: UnitType, flag: usize) -> TagEditorVisibility {
        self.unit_flags.get(&(flag, typ)).cloned().unwrap_or(TagEditorVisibility::Hidden)
    }
    pub fn is_unit_tag_normal(&self, typ: UnitType, tag: usize) -> TagEditorVisibility {
        self.unit_tags.get(&(tag, typ)).cloned().unwrap_or(TagEditorVisibility::Hidden)
    }

    // units

    pub fn unit_count(&self) -> usize {
        self.units.len()
    }

    pub fn unit_types(&self) -> Vec<UnitType> {
        (0..self.unit_count()).map(|i| UnitType(i)).collect()
    }

    pub fn unknown_unit(&self) -> UnitType {
        self.unknown_unit
    }

    pub fn max_transported(&self) -> usize {
        self.max_transported
    }

    pub fn unit_max_transport_capacity(&self, typ: UnitType) -> usize {
        self.unit_config(typ).transport_capacity
        + self.commanders.iter().map(|c| c.transport_capacity as usize).max().unwrap_or(0)
        + self.max_hero_transport_bonus
    }

    pub(super) fn unit_config(&self, typ: UnitType) -> &UnitTypeConfig {
        &self.units[typ.0]
    }

    pub fn unit_name(&self, typ: UnitType) -> &str {
        &self.unit_config(typ).name
    }

    pub fn find_unit_by_name(&self, name: &str) -> Option<UnitType> {
        for (unit_type, conf) in self.units.iter().enumerate() {
            if conf.name.as_str() == name {
                return Some(UnitType(unit_type))
            }
        }
        None
    }

    pub fn unit_ownership(&self, typ: UnitType) -> OwnershipPredicate {
        self.unit_config(typ).owned
    }

    pub fn movement_pattern(&self, typ: UnitType) -> MovementPattern {
        self.unit_config(typ).movement_pattern
    }

    pub fn base_movement_type(&self, typ: UnitType) -> MovementType {
        self.unit_config(typ).movement_type
    }
    pub fn sub_movement_types(&self, typ: MovementType) -> &[MovementType] {
        &self.movement_types[typ.0].sub_types
    }

    pub fn base_movement_points(&self, typ: UnitType) -> Rational32 {
        self.unit_config(typ).movement_points
    }

    pub fn can_be_moved_through(&self, typ: UnitType) -> bool {
        self.unit_config(typ).can_be_moved_through
    }

    pub fn can_be_taken(&self, typ: UnitType) -> bool {
        self.unit_config(typ).can_be_taken
    }

    pub fn can_attack_after_moving(&self, typ: UnitType) -> bool {
        self.unit_config(typ).can_attack_after_moving
    }

    pub fn default_attack_pattern(&self, typ: UnitType) -> AttackPattern {
        self.unit_config(typ).attack_pattern.clone()
    }

    pub fn default_attack_direction(&self, typ: UnitType) -> AllowedAttackInputDirectionSource {
        self.unit_config(typ).attack_direction
    }

    pub fn valid_attack_targets(&self, typ: UnitType) -> ValidAttackTargets {
        self.unit_config(typ).attack_targets
    }

    pub fn base_value(&self, typ: UnitType) -> i32 {
        self.unit_config(typ).value as i32
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

    pub fn hero_types(&self) -> Vec<HeroType> {
        (0..self.hero_count()).map(|i| HeroType(i)).collect()
    }

    pub(super) fn hero_config(&self, typ: HeroType) -> &HeroTypeConfig {
        &self.heroes[typ.0]
    }

    pub fn hero_name(&self, typ: HeroType) -> &str {
        &self.hero_config(typ).name
    }

    pub fn find_hero_by_name(&self, name: &str) -> Option<HeroType> {
        for (hero_type, conf) in self.heroes.iter().enumerate() {
            if conf.name.as_str() == name {
                return Some(HeroType(hero_type))
            }
        }
        None
    }

    pub fn hero_price<D: Direction>(
        &self,
        game: &Handle<Game<D>>,
        hero: HeroType,
        path: &Path<D>,
        // when moving out of a transporter
        transport_index: Option<usize>,
    ) -> Option<i32> {
        let mut game = UnitMovementView::new(game);
        let (unit_pos, unit) = game.unit_path_without_placing(transport_index, &path)?;
        game.put_unit(unit_pos, unit.clone());
        self.hero_price_after_moving(&game, hero, path, unit_pos, unit, transport_index)
    }
    pub fn hero_price_after_moving<D: Direction>(
        &self,
        game: &impl GameView<D>,
        hero: HeroType,
        path: &Path<D>,
        unit_pos: Point,
        unit: Unit<D>,
        // when moving out of a transporter
        transport_index: Option<usize>,
    ) -> Option<i32> {
        let unit_type = unit.typ();
        let mut scope = Scope::new();
        scope.push_constant(CONST_NAME_TRANSPORTER, game.get_unit(path.start).map(|u| Dynamic::from(u)).unwrap_or(().into()));
        scope.push_constant(CONST_NAME_TRANSPORTER_POSITION, path.start);
        scope.push_constant(CONST_NAME_TRANSPORT_INDEX, transport_index.map(|i| Dynamic::from(i as i32)).unwrap_or(().into()));
        scope.push_constant(CONST_NAME_PATH, path.clone());
        scope.push_constant(CONST_NAME_UNIT, unit);
        scope.push_constant(CONST_NAME_POSITION, unit_pos);
        let engine = game.environment().get_engine_board(game);
        let executor = Executor::new(engine, scope, game.environment());
        let cost = self.hero_config(hero).price.update_value(self.base_value(unit_type), &executor);
        if cost < 0 {
            None
        } else {
            Some(cost)
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
        &self.heroes[typ.0].powers
    }

    pub fn hero_can_gain_charge(&self, typ: HeroType, power: usize) -> bool {
        !self.hero_powers(typ)[power].prevents_charging
    }

    pub fn hero_aura_range<D: Direction>(
        &self,
        map: &impl GameView<D>,
        unit: &Unit<D>,
        unit_pos: Point,
        transporter: Option<(&Unit<D>, usize)>,
    ) -> Option<usize> {
        let Some(hero) = unit.get_hero() else {
            return None
        };
        let result = self.unit_power_configs(
            map,
            unit,
            (unit_pos, transporter.map(|u| u.1)),
            transporter.map(|u| (u.0, unit_pos)),
            None,
            &[],
            &[],
            false,
            |iter, executor| -> Option<i8> {
                Some(if transporter.is_none() {
                    let aura_range = self.hero_powers(hero.typ())[hero.get_active_power()].aura_range;
                    NumberMod::update_value_repeatedly(
                        aura_range,
                        iter.map(|c| c.aura_range),
                        executor,
                    )
                } else {
                    let aura_range = self.hero_powers(hero.typ())[hero.get_active_power()].aura_range_transported;
                    NumberMod::update_value_repeatedly(
                        aura_range,
                        iter.map(|c| c.aura_range_transported),
                        executor,
                    )
                })
            }
        )?;
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

    pub fn terrain_types(&self) -> Vec<TerrainType> {
        (0..self.terrain_count()).map(|i| TerrainType(i)).collect()
    }

    pub(super) fn terrain_config(&self, typ: TerrainType) -> &TerrainTypeConfig {
        &self.terrains[typ.0]
    }

    pub fn terrain_name(&self, typ: TerrainType) -> &str {
        &self.terrain_config(typ).name
    }

    pub fn find_terrain_by_name(&self, name: &str) -> Option<TerrainType> {
        for (terrain_type, conf) in self.terrains.iter().enumerate() {
            if conf.name.as_str() == name {
                return Some(TerrainType(terrain_type))
            }
        }
        None
    }

    pub fn terrain_ownership(&self, typ: TerrainType) -> OwnershipPredicate {
        self.terrain_config(typ).owned
    }

    pub fn terrain_owner_is_playable(&self, typ: TerrainType) -> bool {
        self.terrain_config(typ).owner_is_playable
    }

    pub fn terrain_chess(&self, typ: TerrainType) -> bool {
        self.terrain_config(typ).chess
    }

    /**
     * this function could indirectly call itself!
     * avoids infinite recursion using "terrain_config_limit"
     */
    pub(super) fn terrain_power_configs<'a, D: Direction, R>(
        &'a self,
        game: &'a impl GameView<D>,
        pos: Point,
        terrain: &'a Terrain<D>,
        // the heroes affecting this terrain. shouldn't be taken from game since they could have died before this function is called
        heroes: &'a [HeroInfluence<D>],
        f: impl FnOnce(Box<dyn DoubleEndedIterator<Item = &'a TerrainPoweredConfig> + 'a>, &Executor) -> R,
    ) -> R {
        let engine = game.environment().get_engine_board(game);
        let mut scope = Scope::new();
        // build scope
        scope.push_constant(CONST_NAME_POSITION, pos);
        scope.push_constant(CONST_NAME_TERRAIN, terrain.clone());
        // TODO: heroes (put them into Arc<Vec<>> instead of &[])
        let executor = Arc::new(Executor::new(engine, scope, game.environment()));
        let executor_ = executor.clone();
        let max_len = self.terrain_overrides.len();
        let limit = game.get_terrain_config_limit();
        let it = self.terrain_overrides.iter()
        .take(limit.unwrap_or(max_len))
        .enumerate()
        .filter(move |(i, config)| {
            game.set_terrain_config_limit(Some(*i));
            config.affects.iter().all(|filter| filter.check(game, pos, terrain, heroes, &executor))
        })
        .map(|(_, config)| config);
        let r = f(Box::new(it), &executor_);
        game.set_terrain_config_limit(limit);
        r
    }

    pub fn terrain_path_extra(&self, typ: TerrainType) -> ExtraMovementOptions {
        self.terrain_config(typ).extra_movement_options
    }

    pub fn terrain_movement_cost(&self, typ: TerrainType, movement_type: MovementType) -> Option<Rational32> {
        self.movement_cost.get(&typ)
        .and_then(|map| map.get(&movement_type))
        .cloned()
    }

    pub fn terrain_owner_visibility(&self, _typ: TerrainType) -> UnitVisibility {
        // TODO
        UnitVisibility::Normal
    }

    pub fn terrain_vision_range_base(&self, typ: TerrainType) -> Option<usize> {
        let range = self.terrain_config(typ).vision_range;
        if range < 0 {
            None
        } else {
            Some(range as usize)
        }
    }

    pub fn terrain_vision_range<D: Direction>(
        &self,
        map: &impl GameView<D>,
        pos: Point,
        terrain: &Terrain<D>,
        // the heroes affecting this terrain. shouldn't be taken from game since they could have died before this function is called
        heroes: &[HeroInfluence<D>],
    ) -> Option<usize> {
        let result = self.terrain_power_configs(
            map,
            pos,
            terrain,
            heroes,
            |iter, executor| {
                NumberMod::update_value_repeatedly(
                    self.terrain_config(terrain.typ()).vision_range,
                    iter.map(|c| c.vision),
                    executor,
                ) as i8
            }
        );
        if result < 0 {
            None
        } else {
            Some(result as usize)
        }
    }

    pub fn terrain_base_income_factor(&self, typ: TerrainType) -> Rational32 {
        self.terrain_config(typ).income_factor
    }

    pub fn terrain_income_factor<D: Direction>(
        &self,
        map: &impl GameView<D>,
        pos: Point,
        terrain: &Terrain<D>,
        heroes: &[HeroInfluence<D>],
    ) -> Rational32 {
        self.terrain_power_configs(
            map,
            pos,
            terrain,
            heroes,
            |iter, executor| {
                NumberMod::update_value_repeatedly(
                    self.terrain_config(terrain.typ()).income_factor,
                    iter.map(|c| c.income_factor),
                    executor,
                )
            }
        )
    }

    pub fn terrain_action_script<D: Direction>(
        &self,
        map: &impl GameView<D>,
        pos: Point,
        terrain: &Terrain<D>,
        heroes: &[HeroInfluence<D>],
    ) -> Option<(usize, usize)> {
        let mut result = None;
        self.terrain_power_configs(
            map,
            pos,
            terrain,
            heroes,
            |iter, _executor| {
                for config in iter {
                    if let Some(script) = config.action_script {
                        result = Some(script);
                    }
                }
            }
        );
        result
    }

    #[cfg(feature = "rendering")]
    pub fn terrain_preview(&self, typ: TerrainType, owner: i8) -> Vec<(interfaces::PreviewShape, [u8; 4])> {
        self.terrain_config(typ).preview.iter()
        .map(|(shape, color)| {
            let color = color.clone()
                .unwrap_or(self.owner_colors[(owner + 1) as usize]);
            (*shape, color)
        }).collect()
    }

    // tokens

    pub fn token_count(&self) -> usize {
        self.tokens.len()
    }

    pub fn token_types(&self) -> Vec<TokenType> {
        (0..self.token_count()).map(|i| TokenType(i)).collect()
    }

    pub(super) fn token_config(&self, typ: TokenType) -> &TokenTypeConfig {
        &self.tokens[typ.0]
    }

    pub fn token_name(&self, typ: TokenType) -> &str {
        &self.token_config(typ).name
    }

    pub fn find_token_by_name(&self, name: &str) -> Option<TokenType> {
        for (token_type, conf) in self.tokens.iter().enumerate() {
            if conf.name.as_str() == name {
                return Some(TokenType(token_type))
            }
        }
        None
    }

    pub fn token_ownership(&self, typ: TokenType) -> OwnershipPredicate {
        self.token_config(typ).owned
    }

    pub fn token_owner_is_playable(&self, typ: TokenType) -> bool {
        self.token_config(typ).owner_is_playable
    }

    pub fn token_visibility(&self, typ: TokenType) -> UnitVisibility {
        self.token_config(typ).visibility
    }

    pub fn token_owner_visibility(&self, _typ: TokenType) -> UnitVisibility {
        // TODO
        UnitVisibility::Normal
    }

    pub fn token_vision_range(&self, typ: TokenType) -> Option<usize> {
        let range = self.token_config(typ).vision_range;
        if range < 0 {
            None
        } else {
            Some(range as usize)
        }
    }

    pub fn token_action_script(&self, typ: TokenType) -> Option<(usize, usize)> {
        self.token_config(typ).action_script
    }

    pub fn token_on_unit_path(&self, typ: TokenType) -> Option<usize> {
        self.token_config(typ).on_unit_path
    }

    // movement type

    pub fn movement_type_count(&self) -> usize {
        self.movement_types.len()
    }

    pub fn movement_types(&self) -> Vec<MovementType> {
        (0..self.movement_type_count()).map(|i| MovementType(i)).collect()
    }

    pub fn movement_type_name(&self, typ: MovementType) -> &str {
        &self.movement_types[typ.0].name
    }
    
    pub fn find_movement_by_name(&self, name: &str) -> Option<MovementType> {
        for (movement_type, conf) in self.movement_types.iter().enumerate() {
            if conf.name.as_str() == name {
                return Some(MovementType(movement_type))
            }
        }
        None
    }

    // effect

    pub fn effect_count(&self) -> usize {
        self.effect_types.len()
    }

    pub fn effect_types(&self) -> Vec<EffectType> {
        (0..self.effect_count()).map(|i| EffectType(i)).collect()
    }

    pub fn effect_name(&self, effect: EffectType) -> &String {
        &self.effect_types[effect.0].name
    }

    pub fn find_effect_by_name(&self, name: &str) -> Option<EffectType> {
        for (effect_type, conf) in self.effect_types.iter().enumerate() {
            if conf.name.as_str() == name {
                return Some(EffectType(effect_type))
            }
        }
        None
    }

    pub fn effect_is_global(&self, effect: EffectType) -> bool {
        self.effect_types[effect.0].is_global
    }

    pub fn effect_data(&self, effect: EffectType) -> Option<EffectDataType> {
        self.effect_types[effect.0].data_type
    }

    pub fn effect_visibility(&self, effect: EffectType) -> EffectVisibility {
        self.effect_types[effect.0].visibility
    }

    // commanders

    pub fn commander_count(&self) -> usize {
        self.commanders.len()
    }

    pub fn commander_types(&self) -> Vec<CommanderType> {
        (0..self.commander_count()).map(|i| CommanderType(i)).collect()
    }

    pub(super) fn commander_config(&self, typ: CommanderType) -> &CommanderTypeConfig {
        &self.commanders[typ.0]
    }

    pub fn commander_name(&self, typ: CommanderType) -> &str {
        &self.commander_config(typ).name
    }

    pub fn max_commander_charge(&self) -> u32 {
        self.max_commander_charge
    }

    pub fn commander_max_charge(&self, typ: CommanderType) -> u32 {
        self.commander_config(typ).max_charge
    }

    pub fn commander_powers(&self, typ: CommanderType) -> &[CommanderPowerConfig] {
        &self.commander_config(typ).powers
    }

    pub fn commander_can_gain_charge(&self, typ: CommanderType, power: usize) -> bool {
        self.commander_powers(typ).get(power)
        .map(|power| !power.prevents_charging)
        .unwrap_or(false)
    }

    // commander unit

    /**
     * this function can indirectly call itself, if
     *      - some config of other_unit, transporter or a hero is filtered for
     *      - the filter takes a unit from game and wants to check one of its configs
     * avoids infinite recursion using "unit_config_limit"
     */
    fn unit_power_configs<'a, D: Direction, R>(
        &'a self,
        game: &'a impl GameView<D>,
        unit: &'a Unit<D>,
        unit_pos: (Point, Option<usize>),
        // when moving out of a transporter, or start_turn for transported units
        transporter: Option<(&'a Unit<D>, Point)>,
        // the attacked unit, the unit this one was destroyed by, ...
        other_unit: Option<(&'a Unit<D>, Point, Option<usize>, &'a [HeroInfluence<D>])>,
        // the heroes affecting this unit. shouldn't be taken from game since they could have died before this function is called
        heroes: &'a [HeroInfluence<D>],
        // empty if the unit hasn't moved
        temporary_ballast: &'a [TBallast<D>],
        is_counter: bool,
        f: impl FnOnce(Box<dyn DoubleEndedIterator<Item = &'a CommanderPowerUnitConfig> + 'a>, &Executor) -> R,
    ) -> R {
        // get engine...
        let engine = game.environment().get_engine_board(game);
        let mut scope = Scope::new();
        // build scope
        scope.push_constant(CONST_NAME_UNIT, unit.clone());
        scope.push_constant(CONST_NAME_POSITION, unit_pos.0);
        scope.push_constant(CONST_NAME_TRANSPORT_INDEX, unit_pos.1.map(|i| i as i32));
        scope.push_constant(CONST_NAME_TRANSPORTER, transporter.map(|(t, _)| t.clone()));
        scope.push_constant(CONST_NAME_TRANSPORTER_POSITION, transporter.map(|(_, p)| p));
        scope.push_constant(CONST_NAME_OTHER_UNIT, other_unit.map(|(t, _, _, _)| t.clone()));
        scope.push_constant(CONST_NAME_OTHER_POSITION, other_unit.map(|(_, p, _, _)| p));
        // TODO: heroes and ballast (put them into Arc<>s)
        scope.push_constant(CONST_NAME_IS_COUNTER,is_counter);
        let executor = Arc::new(Executor::new(engine, scope, game.environment()));
        let executor_ = executor.clone();
        let max_len = self.unit_overrides.len();
        let limit = game.get_unit_config_limit();
        let it = self.unit_overrides.iter()
        .take(limit.unwrap_or(max_len))
        .enumerate()
        .filter(move |(i, config)| {
            game.set_unit_config_limit(Some(*i));
            config.affects.iter().all(|filter| filter.check(game, unit, unit_pos, transporter, other_unit, heroes, temporary_ballast, is_counter, &executor))
        })
        .map(|(_, config)| config);
        let r = f(Box::new(it), &executor_);
        game.set_unit_config_limit(limit);
        r
    }

    pub fn unit_value<D: Direction>(
        &self,
        game: &impl GameView<D>,
        unit: &Unit<D>,
        unit_pos: Point,
        factory_unit: Option<&Unit<D>>, // if built by a unit
        heroes: &[HeroInfluence<D>],
    ) -> i32 {
        self.unit_power_configs(
            game,
            unit,
            (unit_pos, None),
            factory_unit.map(|u| (u, unit_pos)),
            None,
            heroes,
            &[],
            false,
            |iter, executor| {
                NumberMod::update_value_repeatedly(
                    self.base_value(unit.typ()),
                    iter.map(|c| c.value),
                    executor,
                )
            }
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
        self.unit_power_configs(
            game,
            unit,
            (unit_pos, None),
            None,
            None,
            heroes,
            &[],
            false,
            |iter, _executor| {
                for config in iter.rev() {
                    if let Some(visibility) = config.visibility {
                        result = visibility;
                        break;
                    }
                }
            }
        );
        result
    }

    pub fn unit_vision<D: Direction>(
        &self,
        game: &impl GameView<D>,
        unit: &Unit<D>,
        unit_pos: Point,
        heroes: &[HeroInfluence<D>],
    ) -> usize {
        self.unit_power_configs(
            game,
            unit,
            (unit_pos, None),
            None,
            None,
            heroes,
            &[],
            false,
            |iter, executor| {
                NumberMod::update_value_repeatedly(
                    self.base_vision_range(unit.typ()) as u8,
                    iter.map(|c| c.vision),
                    executor,
                )
            }
        ) as usize
    }

    pub fn unit_true_vision<D: Direction>(
        &self,
        game: &impl GameView<D>,
        unit: &Unit<D>,
        unit_pos: Point,
        heroes: &[HeroInfluence<D>],
    ) -> usize {
        self.unit_power_configs(
            game,
            unit,
            (unit_pos, None),
            None,
            None,
            heroes,
            &[],
            false,
            |iter, executor| {
                NumberMod::update_value_repeatedly(
                    self.base_true_vision_range(unit.typ()) as u8,
                    iter.map(|c| c.true_vision),
                    executor,
                )
            }
        ) as usize
    }

    /*pub fn unit_attack_effects<D: Direction>(
        &self,
        game: &impl GameView<D>,
        unit: &Unit<D>,
        unit_pos: Point,
        defender: &Unit<D>,
        defender_pos: Point,
        transporter: Option<(&Unit<D>, Point)>, // if the attacker moved out of a transporter to attack
        heroes: &HeroMap<D>,
        temporary_ballast: &[TBallast<D>],
        is_counter: bool,
    ) -> Vec<usize> {
        let mut result = Vec::new();
        self.unit_power_configs(
            game,
            unit,
            (unit_pos, None),
            transporter,
            Some((defender, defender_pos, None, heroes.get(defender_pos, defender.get_owner_id()))),
            heroes.get(unit_pos, unit.get_owner_id()),
            temporary_ballast,
            is_counter,
            |iter, _executor| {
                for config in iter {
                    result.extend(config.on_attack.iter().cloned())
                }
            }
        );
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
        heroes: &HeroMap<D>,
        temporary_ballast: &[TBallast<D>],
        is_counter: bool
    ) -> Vec<usize> {
        let mut result = Vec::new();
        self.unit_power_configs(
            game,
            unit,
            (unit_pos, None),
            transporter,
            Some((attacker, attacker_pos, None, heroes.get(attacker_pos, attacker.get_owner_id()))),
            heroes.get(unit_pos, unit.get_owner_id()),
            temporary_ballast,
            is_counter,
            |iter, _executor| {
                for config in iter {
                    result.extend(config.on_defend.iter().cloned())
                }
            }
        );
        result
    }*/

    pub fn unit_kill_effects<D: Direction>(
        &self,
        game: &impl GameView<D>,
        unit: &Unit<D>,
        unit_pos: Point,
        defender: &Unit<D>,
        defender_pos: Point,
        transporter: Option<(&Unit<D>, Point)>, // if the attacker moved out of a transporter to attack
        heroes: &HeroMap<D>,
        temporary_ballast: &[TBallast<D>],
        is_counter: bool,
    ) -> Vec<usize> {
        let mut result = Vec::new();
        self.unit_power_configs(
            game,
            unit,
            (unit_pos, None),
            transporter,
            Some((defender, defender_pos, None, heroes.get(defender_pos, defender.get_owner_id()))),
            heroes.get(unit_pos, unit.get_owner_id()),
            temporary_ballast,
            is_counter,
            |iter, _executor| {
                for config in iter {
                    result.extend(config.on_kill.iter().cloned())
                }
            }
        );
        result
    }

    pub fn unit_death_effects<D: Direction>(
        &self,
        game: &impl GameView<D>,
        unit: &Unit<D>,
        unit_pos: (Point, Option<usize>),
        transporter: Option<(&Unit<D>, Point)>,
        attacker: Option<(&Unit<D>, Point)>,
        heroes: &HeroMap<D>,
        temporary_ballast: &[TBallast<D>],
    ) -> Vec<usize> {
        let mut result = Vec::new();
        self.unit_power_configs(
            game,
            unit,
            unit_pos,
            transporter,
            attacker.map(|(u, p)| (u, p, None, heroes.get(p, u.get_owner_id()))),
            heroes.get(unit_pos.0, unit.get_owner_id()),
            temporary_ballast,
            false,
            |iter, _executor| {
                for config in iter {
                    result.extend(config.on_death.iter().cloned())
                }
            }
        );
        result
    }

    pub fn unit_normal_action_effects<D: Direction>(
        &self,
        game: &impl GameView<D>,
        unit: &Unit<D>,
        unit_pos: (Point, Option<usize>),
        // the transporter the unit moved out of
        transporter: Option<(&Unit<D>, Point)>,
        // the unit this unit moved on top of
        other_unit: Option<(&Unit<D>, Point)>,
        heroes: &HeroMap<D>,
        temporary_ballast: &[TBallast<D>],
    ) -> Vec<usize> {
        let mut result = Vec::new();
        self.unit_power_configs(
            game,
            unit,
            unit_pos,
            transporter,
            other_unit.map(|(u, p)| (u, p, None, heroes.get(p, u.get_owner_id()))),
            heroes.get(unit_pos.0, unit.get_owner_id()),
            temporary_ballast,
            false,
            |iter, _executor| {
                for config in iter {
                    result.extend(config.on_normal_action.iter().cloned())
                }
            }
        );
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
        self.unit_power_configs(
            game,
            unit,
            unit_pos,
            transporter,
            None,
            heroes,
            &[],
            false,
            |iter, executor| {
                NumberMod::update_value_repeatedly(
                    self.base_movement_points(unit.typ()),
                    iter.map(|c| c.movement_points),
                    executor,
                )
            }
        )
    }

    pub fn unit_can_pass_enemy_units<D: Direction>(
        &self,
        game: &impl GameView<D>,
        unit: &Unit<D>,
        unit_pos: (Point, Option<usize>),
        transporter: Option<(&Unit<D>, Point)>,
        heroes: &[HeroInfluence<D>],
    ) -> bool {
        self.unit_power_configs(
            game,
            unit,
            unit_pos,
            transporter,
            None,
            heroes,
            &[],
            false,
            |iter, _| {
                for conf in iter.rev() {
                    if let Some(pass_enemy_units) = conf.pass_enemy_units {
                        return pass_enemy_units;
                    }
                }
                self.unit_config(unit.typ()).pass_enemy_units
            }
        )
    }

    fn attack_power_configs<'a, D: Direction, R>(
        &'a self,
        game: &'a impl GameView<D>,
        outer_scope: &'a Scope,
        attack: ConfiguredAttack,
        splash: Option<AttackInstance>,
        unit: &'a Unit<D>,
        unit_pos: (Point, Option<usize>),
        // when moving out of a transporter, or start_turn for transported units
        transporter: Option<(&'a Unit<D>, Point)>,
        // the heroes affecting this unit. shouldn't be taken from game since they could have died before this function is called
        heroes: &'a HeroMap<D>,
        // empty if the unit hasn't moved
        temporary_ballast: &'a [TBallast<D>],
        counter: &'a AttackCounterState<D>,
        f: impl FnOnce(Box<dyn DoubleEndedIterator<Item = &'a AttackPoweredConfig> + 'a>, &Executor) -> R,
    ) -> R {
        // get engine...
        let engine = game.environment().get_engine_board(game);
        let mut scope = Scope::new();
        for (name, _, value) in outer_scope.iter() {
            scope.push_constant_dynamic(name, value);
        }
        let executor = Arc::new(Executor::new(engine, scope, game.environment()));
        let executor_ = executor.clone();
        let it = self.attack_overrides.iter()
        .filter(move |config| {
            config.affects.iter().all(|filter| filter.check(game, &attack, splash.as_ref(), unit, unit_pos, transporter, heroes, temporary_ballast, counter, &executor))
        });
        let r = f(Box::new(it), &executor_);
        r
    }

    pub fn attack_bonus<D: Direction>(
        &self,
        column_name: &String,
        base_value: Rational32,
        game: &impl GameView<D>,
        attack: &ConfiguredAttack,
        splash: &AttackInstance,
        attacker: &Unit<D>,
        attacker_pos: Point,
        defender: Option<(&Unit<D>, Point, Option<usize>)>,
        heroes: &HeroMap<D>,
        temporary_ballast: &[TBallast<D>],
        counter: &AttackCounterState<D>,
    ) -> Rational32 {
        let scope = attack_power_scope(
            attack,
            Some(splash),
            attacker,
            attacker_pos,
            defender,
            None,
            heroes,
            temporary_ballast,
            counter.is_counter(),
        );
        self.attack_power_configs(
            game,
            &scope,
            attack.clone(),
            Some(splash.clone()),
            attacker,
            (attacker_pos, None),
            None,
            heroes,
            temporary_ballast,
            counter,
            |iter, executor| {
                NumberMod::update_value_repeatedly(
                    base_value,
                    iter.map(|c| c.get_fraction(column_name)),
                    executor,
                )
            }
        )
    }

    pub fn unit_configured_attacks<D: Direction>(
        &self,
        game: &impl GameView<D>,
        unit: &Unit<D>,
        unit_pos: Point,
        transporter: Option<(&Unit<D>, Point)>,
        counter: AttackCounterState<D>,
        heroes: &HeroMap<D>,
        temporary_ballast: &[TBallast<D>],
    ) -> Vec<ConfiguredAttack> {
        let is_counter = counter.is_counter();
        let other_unit = counter.attacker()
            .map(|(u, p)| (u, p, None, heroes.get(p, u.get_owner_id())));
        let AttackType(Some(attack_type)) = self.unit_power_configs(
            game,
            unit,
            (unit_pos, None),
            None,
            other_unit,
            heroes.get(unit_pos, unit.get_owner_id()),
            temporary_ballast,
            is_counter,
            |iter, _| {
                for conf in iter.rev() {
                    if let Some(attack_type) = conf.attack_type {
                        return attack_type;
                    }
                }
                self.unit_config(unit.typ()).attack_type
            }
        ) else {
            return Vec::new();
        };
        let mut result = Vec::new();
        for attack in &self.attack_types[attack_type].1 {
            let mut atk = ConfiguredAttack {
                typ: AttackType(Some(attack_type)),
                splash_pattern: attack.splash_pattern,
                splash_range: attack.splash_range,
                priority: attack.priority,
                splash: Vec::new(),
                focus: attack.focus,
            };
            let scope = attack_power_scope(
                &atk,
                None,
                unit,
                unit_pos,
                None,
                transporter,
                heroes,
                temporary_ballast,
                is_counter,
            );
            let engine = game.environment().get_engine_board(game);
            let executor = Executor::new(engine, scope.clone(), game.environment());
            if attack.condition.iter().all(|cond| cond.check(
                game,
                &atk,
                None,
                unit,
                (unit_pos, None),
                transporter,
                heroes,
                temporary_ballast,
                &counter,
                &executor,
            )) {
                self.attack_power_configs(
                    game,
                    &scope,
                    atk.clone(),
                    None,
                    unit,
                    (unit_pos, None),
                    None,
                    heroes,
                    temporary_ballast,
                    &counter,
                    |iter, executor| {
                        for conf in iter {
                            atk.priority = conf.attack_priority.update_value(atk.priority, executor);
                            atk.splash_range = conf.splash_range.update_value(atk.splash_range, executor);
                            if let Some(focus) = conf.focus {
                                atk.focus = focus;
                            }
                        }
                    }
                );
                for splash_distance in 0..=atk.splash_range {
                    for splash in &self.splash_types[attack.splash_type.0].1 {
                        let mut spl = AttackInstance {
                            allows_counter_attack: splash.allows_counter_attack,
                            splash_distance: splash_distance as usize,
                            priority: splash.priority,
                            direction_modifier: splash.direction_modifier,
                            script: splash.script,
                        };
                        // TODO: add spl to scope and executor
                        if splash.condition.iter().all(|cond| cond.check(
                            game,
                            &atk,
                            Some(&spl),
                            unit,
                            (unit_pos, None),
                            transporter,
                            heroes,
                            temporary_ballast,
                            &counter,
                            &executor,
                        )) {
                            self.attack_power_configs(
                                game,
                                &scope,
                                atk.clone(),
                                Some(spl.clone()),
                                unit,
                                (unit_pos, None),
                                None,
                                heroes,
                                temporary_ballast,
                                &counter,
                                |iter, executor| {
                                    for conf in iter {
                                        spl.priority = conf.splash_priority.update_value(spl.priority, executor);
                                        if let Some(allows_counter_attack) = conf.allows_counter_attack {
                                            spl.allows_counter_attack = allows_counter_attack;
                                        }
                                        if let Some(direction_modifier) = conf.direction_modifier {
                                            spl.direction_modifier = direction_modifier;
                                        }
                                    }
                                }
                            );
                            atk.splash.push(spl);
                        }
                    }
                }
                if atk.splash.len() > 0 {
                    result.push(atk);
                }
            }
        }
        result.sort_by_key(|attack| attack.priority);
        result
    }

    pub fn unit_attack_pattern<D: Direction>(
        &self,
        game: &impl GameView<D>,
        unit: &Unit<D>,
        unit_pos: Point,
        counter: AttackCounterState<D>,
        heroes: &HeroMap<D>,
        temporary_ballast: &[TBallast<D>],
    ) -> AttackPattern {
        let is_counter = counter.is_counter();
        let other_unit = counter.attacker()
            .map(|(u, p)| (u, p, None, heroes.get(p, u.get_owner_id())));
        let mut result = self.default_attack_pattern(unit.typ());
        self.unit_power_configs(
            game,
            unit,
            (unit_pos, None),
            None,
            other_unit,
            heroes.get(unit_pos, unit.get_owner_id()),
            temporary_ballast,
            is_counter,
            |iter, _executor| {
                for conf in iter {
                    if let Some(pattern) = &conf.attack_pattern {
                        result = pattern.clone();
                    }
                }
            }
        );
        for (name, value) in result.parameters() {
            *value = self.unit_power_configs(
                game,
                unit,
                (unit_pos, None),
                None,
                other_unit,
                heroes.get(unit_pos, unit.get_owner_id()),
                temporary_ballast,
                is_counter,
                |iter, executor| {
                    NumberMod::update_value_repeatedly(
                        *value,
                        iter.map(|c| c.get_fraction(&name)),
                        executor,
                    )
                }
            )
        }
        result
    }

    pub fn unit_attack_directions<D: Direction>(
        &self,
        game: &impl GameView<D>,
        unit: &Unit<D>,
        unit_pos: Point,
        counter: AttackCounterState<D>,
        heroes: &HeroMap<D>,
        temporary_ballast: &[TBallast<D>],
    ) -> AllowedAttackInputDirectionSource {
        let is_counter = counter.is_counter();
        let other_unit = counter.attacker()
            .map(|(u, p)| (u, p, None, heroes.get(p, u.get_owner_id())));
        self.unit_power_configs(
            game,
            unit,
            (unit_pos, None),
            None,
            other_unit,
            heroes.get(unit_pos, unit.get_owner_id()),
            temporary_ballast,
            is_counter,
            |iter, _executor| {
                for conf in iter {
                    if let Some(attack_direction) = conf.attack_direction {
                        return attack_direction;
                    }
                }
                self.default_attack_direction(unit.typ())
            }
        )
    }

    pub fn unit_attack_bonus<D: Direction>(
        &self,
        column_name: &String,
        base_value: Rational32,
        game: &impl GameView<D>,
        unit: &Unit<D>,
        unit_pos: Point,
        defender: &Unit<D>,
        (defender_pos, defender_unload_index): (Point, Option<usize>),
        heroes: &HeroMap<D>,
        temporary_ballast: &[TBallast<D>],
        is_counter: bool,
    ) -> Rational32 {
        self.unit_power_configs(
            game,
            unit,
            (unit_pos, None),
            None,
            Some((defender, defender_pos, defender_unload_index, heroes.get(defender_pos, defender.get_owner_id()))),
            heroes.get(unit_pos, unit.get_owner_id()),
            temporary_ballast,
            is_counter,
            |iter, executor| {
                NumberMod::update_value_repeatedly(
                    base_value,
                    iter.map(|c| c.get_fraction(column_name)),
                    executor,
                )
            }
        )
    }

    pub fn unit_defense_bonus<D: Direction>(
        &self,
        column_name: &String,
        base_value: Rational32,
        game: &impl GameView<D>,
        unit: &Unit<D>,
        unit_pos: (Point, Option<usize>),
        attacker: &Unit<D>,
        attacker_pos: Point,
        heroes: &HeroMap<D>,
        is_counter: bool,
    ) -> Rational32 {
        self.unit_power_configs(
            game,
            unit,
            unit_pos,
            None,
            Some((attacker, attacker_pos, None, heroes.get(attacker_pos, attacker.get_owner_id()))),
            heroes.get(unit_pos.0, unit.get_owner_id()),
            &[],
            is_counter,
            |iter, executor| {
                NumberMod::update_value_repeatedly(
                    base_value,
                    iter.map(|c| c.get_fraction(column_name)),
                    executor,
                )
            }
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
        self.unit_power_configs(
            game,
            unit,
            (unit_pos, None),
            transporter,
            None,
            heroes,
            temporary_ballast,
            is_counter,
            |iter, executor| {
                if min_range {
                    NumberMod::update_value_repeatedly(
                        base_range,
                        iter.map(|c| c.min_range),
                        executor,
                    )
                } else {
                    NumberMod::update_value_repeatedly(
                        base_range,
                        iter.map(|c| c.max_range),
                        executor,
                    )
                }
            }
        )
    }

    pub fn unit_can_be_displaced<D: Direction>(
        &self,
        game: &impl GameView<D>,
        unit: &Unit<D>,
        unit_pos: Point,
        attacker: &Unit<D>,
        attacker_pos: Point,
        heroes: &HeroMap<D>,
        is_counter: bool,
    ) -> bool {
        self.unit_power_configs(
            game,
            unit,
            (unit_pos, None),
            None,
            Some((attacker, attacker_pos, None, heroes.get(attacker_pos, attacker.get_owner_id()))),
            heroes.get(unit_pos, unit.get_owner_id()),
            &[],
            is_counter,
            |iter, _executor| {
                for conf in iter.rev() {
                    if let Some(can_be_displaced) = conf.can_be_displaced {
                        return can_be_displaced;
                    }
                }
                self.unit_config(unit.typ()).can_be_displaced
            }
        )
    }

}

fn attack_power_scope<D: Direction>(
    attack: &ConfiguredAttack,
    splash: Option<&AttackInstance>,
    attacker: &Unit<D>,
    attacker_pos: Point,
    defender: Option<(&Unit<D>, Point, Option<usize>)>,
    // when moving out of a transporter, or start_turn for transported units
    transporter: Option<(&Unit<D>, Point)>,
    // the heroes affecting this unit. shouldn't be taken from game since they could have died before this function is called
    heroes: &HeroMap<D>,
    // empty if the unit hasn't moved
    temporary_ballast: &[TBallast<D>],
    is_counter: bool,
) -> Scope<'static> {
    let mut scope = Scope::new();
    scope.push_constant(CONST_NAME_SPLASH_DISTANCE, splash.map(|s| Dynamic::from(s.splash_distance as i32)).unwrap_or(().into()));
    scope.push_constant(CONST_NAME_TRANSPORTER, transporter.map(|(u, _)| Dynamic::from(u.clone())).unwrap_or(().into()));
    scope.push_constant(CONST_NAME_TRANSPORTER_POSITION, transporter.map(|(_, p)| Dynamic::from(p)).unwrap_or(().into()));
    scope.push_constant(CONST_NAME_UNIT, attacker.clone());
    scope.push_constant(CONST_NAME_POSITION, attacker_pos);
    scope.push_constant(CONST_NAME_IS_COUNTER, is_counter);
    scope
}
