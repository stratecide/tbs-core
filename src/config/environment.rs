use std::fmt::Debug;
use std::sync::{Arc, Mutex};
use interfaces::ClientPerspective;
use packages::Package;
use rhai::*;
use rustc_hash::FxHashMap as HashMap;

use crate::commander::commander_type::CommanderType;
use crate::game::event_handler::EventHandler;
use crate::game::game_view::GameView;
use crate::game::settings::GameSettings;
use crate::map::direction::Direction;
use crate::map::point_map::MapSize;
use crate::script::*;
use crate::terrain::attributes::TerrainAttributeKey;
use crate::terrain::terrain::*;
use crate::terrain::TerrainType;
use crate::units::unit_types::UnitType;
use crate::units::attributes::*;
use crate::units::hero::*;

use super::config::Config;
use super::table_config::*;

#[derive(Clone)]
pub struct Environment {
    pub map_size: MapSize,
    pub config: Arc<Config>,
    pub settings: Option<Arc<GameSettings>>,
    // cache compilation
    compiled_asts: Arc<Mutex<HashMap<usize, Shared<AST>>>>,
}

impl Environment {
    pub fn new_map(config: Arc<Config>, map_size: MapSize) -> Self {
        Self {
            config,
            map_size,
            settings: None,
            compiled_asts: Arc::default(),
        }
    }

    pub fn new_game(config: Arc<Config>, map_size: MapSize, settings: GameSettings) -> Self {
        Self {
            config,
            map_size,
            settings: Some(Arc::new(settings)),
            compiled_asts: Arc::default(),
        }
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

    pub fn sludge_damage(&self) -> u16 {
        // TODO
        10
    }

    fn get_engine_base(&self, is_hex: bool) -> Engine {
        let mut engine = Engine::new_raw();
        if is_hex {
            self.config.my_package_6.register_into_engine(&mut engine);
        } else {
            self.config.my_package_4.register_into_engine(&mut engine);
        }
        engine.register_global_module(self.config.global_module.clone());
        engine
    }

    pub fn get_engine<D: Direction>(&self, game: &impl GameView<D>) -> Engine {
        let mut engine = self.get_engine_base(D::is_hex());
        let this = self.clone();
        let game = game.as_shared();
        #[allow(deprecated)]
        engine.on_var(move |name, _index, _context| {
            match name {
                CONST_NAME_CONFIG => Ok(Some(Dynamic::from(this.clone()))),
                CONST_NAME_BOARD => Ok(Some(Dynamic::from(game.clone()))),
                _ => Ok(None)
            }
        });
        engine
    }

    pub fn get_engine_handler<D: Direction>(&self, handler: &EventHandler<D>) -> Engine {
        let mut engine = self.get_engine_base(D::is_hex());
        let this = self.clone();
        let game = handler.get_game().as_shared();
        let handler = handler.clone();
        #[allow(deprecated)]
        engine.on_var(move |name, _index, _context| {
            match name {
                CONST_NAME_CONFIG => Ok(Some(Dynamic::from(this.clone()))),
                CONST_NAME_BOARD => Ok(Some(Dynamic::from(game.clone()))),
                CONST_NAME_EVENT_HANDLER => Ok(Some(Dynamic::from(handler.clone()))),
                _ => Ok(None)
            }
        });
        engine
    }

    pub fn rhai_function_name(&self, engine: &Engine, index: usize) -> (Shared<AST>, &String) {
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

    pub fn table_entry(&self, name: &str, x: TableAxisKey, y: TableAxisKey) -> Option<TableValue> {
        self.config.custom_tables.iter()
        .find(|(key, _)| key.as_str() == name)
        .map(|(_, table)| {
            table.values.get(&y)
            .and_then(|map| map.get(&x))
            .unwrap_or(&table.default_value)
            .clone()
        })
    }

    pub fn table_row(&self, name: &str, y: TableAxisKey, value: TableValue) -> Vec<TableAxisKey> {
        let mut result = Vec::new();
        if let Some((_, table)) = self.config.custom_tables.iter()
        .find(|(key, _)| key.as_str() == name) {
            let map = table.values.get(&y);
            for header in &table.column_keys {
                if value == *map.and_then(|map| map.get(header)).unwrap_or(&table.default_value) {
                    result.push(*header);
                }
            }
        }
        result
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
        CommanderType::None
    }

    pub fn unit_attributes(&self, typ: UnitType, owner: i8) -> impl Iterator<Item = &AttributeKey> {
        // order has to be preserved here
        // because this method is used for exporting, while
        // unit_specific_attributes and commander_attributes are used for importing
        self.config.unit_specific_attributes(typ).iter()
        .chain(self.config.commander_attributes(self.get_commander(owner), typ).iter())
    }

    pub fn unit_attributes_hidden_by_fog(&self, typ: UnitType, _hero: &Hero, owner: i8) -> Vec<AttributeKey> {
        self.config.unit_specific_hidden_attributes(typ).iter()
        .chain(self.config.commander_attributes_hidden_by_fog(self.get_commander(owner), typ).iter())
        .cloned()
        .collect()
    }

    pub fn unit_valid_action_status(&self, typ: UnitType, _owner: i8) -> &[ActionStatus] {
        self.config.unit_specific_statuses(typ)
    }

    pub fn unit_transport_capacity(&self, typ: UnitType, owner: i8, hero: HeroType) -> usize {
        self.config.unit_config(typ).transport_capacity
        + self.config.commander_config(self.get_commander(owner)).transport_capacity as usize
        + hero.transport_capacity(self)
    }

    // terrain

    pub fn terrain_attributes(&self, typ: TerrainType, _owner: i8) -> impl Iterator<Item = &TerrainAttributeKey> {
        // order has to be preserved here
        // because this method is used for exporting, while
        // terrain_specific_attributes and commander_attributes are used for importing
        self.config.terrain_specific_attributes(typ).iter()
    }

    pub fn default_terrain(&self) -> Terrain {
        TerrainBuilder::new(self, crate::terrain::TerrainType::Grass)
        .build()
        // TODO: when validating the config, make sure this unwrap won't panic
        .unwrap()
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
