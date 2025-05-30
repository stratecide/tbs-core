use std::fmt::Debug;
use std::sync::{Arc, Mutex};
use interfaces::ClientPerspective;
use packages::Package;
use rhai::*;
use rustc_hash::{FxHashMap as HashMap, FxHashSet};

use crate::commander::commander_type::CommanderType;
use crate::game::event_handler::EventHandler;
use crate::game::game_view::GameView;
use crate::game::rhai_board::SharedGameView;
use crate::game::settings::GameSettings;
use crate::map::direction::Direction;
use crate::map::point::Point;
use crate::map::point_map::MapSize;
use crate::script::*;
use crate::tags::*;
use crate::terrain::terrain::*;
use crate::terrain::TerrainType;
use crate::units::movement::MovementType;
use crate::units::unit::Unit;
use crate::units::unit_types::UnitType;
use crate::units::UnitVisibility;
use crate::units::hero::*;

use super::config::Config;
use super::table_config::*;
use super::tag_config::*;

#[derive(Clone)]
pub struct Environment {
    pub map_size: MapSize,
    pub config: Arc<Config>,
    pub settings: Option<Arc<GameSettings>>,
    // cache compilation
    compiled_asts: Arc<Mutex<HashMap<usize, Shared<AST>>>>,
    unique_ids: Arc<Mutex<HashMap<String, FxHashSet<usize>>>>,
}

impl Environment {
    pub fn new_map(config: Arc<Config>, map_size: MapSize) -> Self {
        Self {
            map_size,
            settings: None,
            compiled_asts: Arc::default(),
            unique_ids: Self::setup_unique_ids(&config),
            config,
        }
    }

    pub fn new_game(config: Arc<Config>, map_size: MapSize, settings: GameSettings) -> Self {
        Self {
            map_size,
            settings: Some(Arc::new(settings)),
            compiled_asts: Arc::default(),
            unique_ids: Self::setup_unique_ids(&config),
            config,
        }
    }

    fn setup_unique_ids(config: &Config) -> Arc<Mutex<HashMap<String, FxHashSet<usize>>>> {
        Arc::new(Mutex::new(config.tags.iter()
        .filter_map(|tag_config| {
            match &tag_config.tag_type {
                TagType::Unique { pool } => Some((pool.clone(), FxHashSet::default())),
                _ => None
            }
        }).collect()))
    }

    pub fn start_game(&mut self, settings: &Arc<GameSettings>) {
        if self.settings.is_some() {
            panic!("Attempted to start an already started game!")
        }
        self.settings = Some(settings.clone());
    }

    pub fn built_this_turn_cost_factor(&self) -> i32 {
        // TODO
        200
    }

    pub(crate) fn add_unique_id(&self, tag_key: usize, id: usize) {
        if let TagType::Unique { pool } = self.config.tag_type(tag_key) {
            self.unique_ids.lock().unwrap().get_mut(pool).unwrap().insert(id);
        }
    }
    pub(crate) fn remove_unique_id(&self, tag_key: usize, id: usize) {
        if let TagType::Unique { pool } = self.config.tag_type(tag_key) {
            self.unique_ids.lock().unwrap().get_mut(pool).unwrap().remove(&id);
        }
    }
    pub(crate) fn generate_unique_id(&self, tag_key: usize, random: f32) -> Option<usize> {
        let TagType::Unique { pool } = self.config.tag_type(tag_key) else {
            return None;
        };
        let mut unique_ids = self.unique_ids.lock().unwrap();
        let pool = unique_ids.get_mut(pool).unwrap();
        if pool.len() > UniqueId::MAX_VALUE {
            return None;
        }
        let count = UniqueId::MAX_VALUE + 1;
        let mut id = (count as f64 * random as f64) as usize;
        while pool.contains(&id) {
            id = (id + 1) % count;
        }
        pool.insert(id);
        Some(id)
    }

    pub(crate) fn log_rhai_error(&self, location: &str, function_name: impl AsRef<str>, error: &EvalAltResult) {
        let config_name = &self.config.name;
        let function_name = function_name.as_ref();
        tracing::warn!("RHAI error in {location}, config '{config_name}', function '{function_name}':\n{error:?}");
    }

    fn get_engine_base(&self, is_hex: bool) -> Engine {
        let mut engine = Engine::new_raw();
        if is_hex {
            self.config.my_package_6.register_into_engine(&mut engine);
            engine.register_global_module(self.config.effect_modules[1].clone());
        } else {
            self.config.my_package_4.register_into_engine(&mut engine);
            engine.register_global_module(self.config.effect_modules[0].clone());
        }
        engine.register_global_module(self.config.global_module.clone());
        engine
    }

    pub fn get_engine<D: Direction>(&self) -> Engine {
        self._get_engine::<D>(None, None)
    }

    pub fn get_engine_board<D: Direction>(&self, game: &impl GameView<D>) -> Engine {
        let game = game.as_shared();
        self._get_engine(Some(game), None)
    }

    pub fn get_engine_handler<D: Direction>(&self, handler: &EventHandler<D>) -> Engine {
        let game = handler.get_game().as_shared();
        let handler = handler.clone();
        self._get_engine(Some(game), Some(handler))
    }

    pub fn _get_engine<D: Direction>(&self, game: Option<SharedGameView<D>>, handler: Option<EventHandler<D>>) -> Engine {
        let mut engine = self.get_engine_base(D::is_hex());
        let this = self.clone();
        #[allow(deprecated)]
        engine.on_var(move |name, _index, _context| {
            match name.split_once("_") {
                Some(("TAG", name)) => return Ok(this.config.tag_by_name(name).map(|key| Dynamic::from(TagKey(key)))),
                Some(("FLAG", name)) => return Ok(this.config.flag_by_name(name).map(|key| Dynamic::from(FlagKey(key)))),
                Some(("MOVEMENT", name)) => return Ok(this.config.find_movement_by_name(name).map(Dynamic::from)),
                Some(("TERRAIN", name)) => return Ok(this.config.find_terrain_by_name(name).map(Dynamic::from)),
                Some(("TOKEN", name)) => return Ok(this.config.find_token_by_name(name).map(Dynamic::from)),
                Some(("UNIT", name)) => return Ok(this.config.find_unit_by_name(name).map(Dynamic::from)),
                _ => (),
            }
            match name {
                CONST_NAME_CONFIG => Ok(Some(Dynamic::from(this.clone()))),
                CONST_NAME_BOARD => Ok(game.clone().map(Dynamic::from)),
                CONST_NAME_EVENT_HANDLER => Ok(handler.clone().map(Dynamic::from)),
                _ => Ok(None)
            }
        });
        engine
    }

    pub fn get_rhai_function_name(&self, index: usize) -> &String {
        &self.config.functions[index].1
    }

    pub fn get_rhai_function(&self, engine: &Engine, index: usize) -> (Shared<AST>, &String) {
        let (ast_index, name) = &self.config.functions[index];
        let mut asts = self.compiled_asts.lock().unwrap();
        let ast = if let Some(ast) = asts.get(ast_index) {
            ast.clone()
        } else {
            let ast = self.config.asts[*ast_index].clone();
            let ast = Shared::new(engine.optimize_ast(&self.config.global_constants, ast, OptimizationLevel::Simple));
            asts.insert(*ast_index, ast.clone());
            ast
        };
        (ast, name)
    }

    pub fn is_unit_dead_rhai(&self) -> usize {
        self.config.is_unit_dead_rhai
    }

    pub fn is_unit_movable_rhai(&self) -> usize {
        self.config.is_unit_movable_rhai
    }

    pub fn deal_damage_rhai(&self) -> usize {
        self.config.deal_damage_rhai
    }

    pub fn calculate_attack_damage_rhai(&self) -> usize {
        self.config.calculate_damage_rhai
    }

    pub fn weapon_effects_rhai(&self) -> Option<usize> {
        self.config.weapon_effects_rhai
    }

    pub fn table_entry(&self, name: &str, x: TableAxisKey, y: TableAxisKey) -> Option<TableValue> {
        //tracing::debug!("table_entry at {x:?}, {y:?}");
        self.config.custom_tables.iter()
        .find(|(key, _)| key.as_str() == name)
        .map(|(_, table)| {
            let value = table.values.get(&(x, y))
            .unwrap_or(&table.default_value)
            .clone();
            //tracing::debug!("value = {value:?}");
            value
        })
    }

    pub fn table_row(&self, name: &str, y: TableAxisKey, value: TableValue) -> Vec<TableAxisKey> {
        let mut result = Vec::new();
        if let Some((_, table)) = self.config.custom_tables.iter()
        .find(|(key, _)| key.as_str() == name) {
            for header in &table.column_keys {
                if value == *table.values.get(&(*header, y)).unwrap_or(&table.default_value) {
                    result.push(*header);
                }
            }
        }
        result
    }
    pub fn table_column(&self, name: &str, x: TableAxisKey, value: TableValue) -> Vec<TableAxisKey> {
        let mut result = Vec::new();
        if let Some((_, table)) = self.config.custom_tables.iter()
        .find(|(key, _)| key.as_str() == name) {
            for row_key in &table.row_keys {
                if value == *table.values.get(&(x, *row_key)).unwrap_or(&table.default_value) {
                    result.push(*row_key);
                }
            }
        }
        result
    }

    pub fn transform_sub_movement_type(&self, base: MovementType, sub: MovementType, terrain: TerrainType) -> MovementType {
        let Some(transformer) = self.config.movement_type_transformer.get(&base) else {
            return sub;
        };
        transformer.get(&(terrain, sub)).cloned()
        .unwrap_or(sub)
    }

    pub fn get_team(&self, owner_id: i8) -> ClientPerspective {
        if let Some(settings) = &self.settings {
            for player in &settings.players {
                if player.get_owner_id() == owner_id {
                    return ClientPerspective::Team(player.get_team())
                }
            }
        }
        ClientPerspective::Neutral
    }

    pub fn get_income(&self, owner_id: i8) -> i32 {
        if let Some(settings) = &self.settings {
            for player in &settings.players {
                if player.get_owner_id() == owner_id {
                    return player.get_income()
                }
            }
        }
        0
    }

    pub fn get_commander(&self, owner_id: i8) -> CommanderType {
        if let Some(settings) = &self.settings {
            for player in &settings.players {
                if player.get_owner_id() == owner_id {
                    return player.get_commander();
                }
            }
        }
        CommanderType(0)
    }

    pub fn unit_custom_attribute(&self, typ: UnitType, column_name: ImmutableString) -> Option<ImmutableString> {
        self.config.unit_config(typ).custom_columns
            .get(&column_name)
            .cloned()
    }

    pub fn unit_transport_capacity(&self, typ: UnitType, owner: i8, hero: Option<HeroType>) -> usize {
        self.config.unit_config(typ).transport_capacity
        + self.config.commander_config(self.get_commander(owner)).transport_capacity as usize
        + hero.map(|hero| hero.transport_capacity(self)).unwrap_or(0)
    }

    pub fn unit_transport_visibility<D: Direction>(&self, _game: &impl GameView<D>, _unit: &Unit<D>, _p: Point, _heroes: &[HeroInfluence<D>]) -> UnitVisibility {
        // TODO
        UnitVisibility::Normal
    }

    pub fn hero_visibility<D: Direction>(&self, _game: &impl GameView<D>, _unit: &Unit<D>, _p: Point, _hero: HeroType) -> UnitVisibility {
        // TODO
        UnitVisibility::Normal
    }

    // terrain

    pub fn default_terrain<D: Direction>(&self) -> Terrain<D> {
        TerrainBuilder::new(self, self.config.default_terrain)
        .build()
    }

}

impl PartialEq for Environment {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.config, &other.config)
        && match (&self.settings, &other.settings) {
            (Some(a), Some(b)) => **a == **b,
            (None, None) => true,
            _ => false
        }
    }
}
impl Eq for Environment {}
impl Debug for Environment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Environment")
        .field("ruleset", &self.config.name)
        .field("map size", &self.map_size)
        .field("settings", &self.settings)
        .finish()
    }
}
