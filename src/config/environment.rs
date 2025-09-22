use std::fmt::Debug;
use std::rc::Rc;

use interfaces::ClientPerspective;
use rhai::*;
use rustc_hash::{FxHashMap as HashMap, FxHashSet};
use uniform_smart_pointer::*;

use crate::commander::commander_type::CommanderType;
use crate::game::settings::GameSettings;
use crate::game::settings::PlayerSettings;
use crate::map::board::Board;
use crate::map::direction::Direction;
use crate::map::point::Point;
use crate::map::point_map::MapSize;
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
    pub config: Urc<Config>,
    pub settings: Option<Urc<GameSettings>>,
    unique_ids: Urc<Umutex<HashMap<String, FxHashSet<usize>>>>,
}

impl Environment {
    pub fn new_map(config: Urc<Config>, map_size: MapSize) -> Self {
        Self {
            map_size,
            settings: None,
            unique_ids: Self::setup_unique_ids(&config),
            config,
        }
    }

    pub fn new_game(config: Urc<Config>, map_size: MapSize, settings: GameSettings) -> Self {
        Self {
            map_size,
            settings: Some(Urc::new(settings)),
            unique_ids: Self::setup_unique_ids(&config),
            config,
        }
    }

    fn setup_unique_ids(config: &Config) -> Urc<Umutex<HashMap<String, FxHashSet<usize>>>> {
        Urc::new(Umutex::new(config.tags.iter()
        .filter_map(|tag_config| {
            match &tag_config.tag_type {
                TagType::Unique { pool } => Some((pool.clone(), FxHashSet::default())),
                _ => None
            }
        }).collect()))
    }

    pub(crate) fn start_game(&mut self, settings: &Urc<GameSettings>) {
        if self.settings.is_some() {
            panic!("Attempted to start an already started game!")
        }
        self.settings = Some(settings.clone());
    }

    pub(crate) fn add_unique_id(&self, tag_key: usize, id: usize) {
        if let TagType::Unique { pool } = self.config.tag_type(tag_key) {
            self.unique_ids.lock().get_mut(pool).unwrap().insert(id);
        }
    }
    pub(crate) fn remove_unique_id(&self, tag_key: usize, id: usize) {
        if let TagType::Unique { pool } = self.config.tag_type(tag_key) {
            self.unique_ids.lock().get_mut(pool).unwrap().remove(&id);
        }
    }
    pub(crate) fn generate_unique_id(&self, tag_key: usize, random: f32) -> Option<usize> {
        let TagType::Unique { pool } = self.config.tag_type(tag_key) else {
            return None;
        };
        let mut unique_ids = self.unique_ids.lock();
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
        crate::warn!("RHAI error in {location}, config '{config_name}', function '{function_name}':\n{error:?}");
    }

    pub fn get_rhai_function_name(&self, index: usize) -> &String {
        &self.config.functions[index].1
    }

    pub fn get_rhai_function(&self, index: usize) -> (&Rc<AST>, &String) {
        let (ast_index, name) = &self.config.functions[index];
        (&self.config.asts[*ast_index], name)
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

    fn get_player_setting(&self, owner_id: i8) -> Option<&PlayerSettings> {
        if let Some(settings) = &self.settings {
            for player in &settings.players {
                if player.get_owner_id() == owner_id {
                    return Some(player);
                }
            }
        }
        None
    }

    pub fn get_team(&self, owner_id: i8) -> ClientPerspective {
        self.get_player_setting(owner_id)
            .map(|player| ClientPerspective::Team(player.get_team()))
            .unwrap_or(ClientPerspective::Neutral)
    }

    pub fn get_commander(&self, owner_id: i8) -> CommanderType {
        self.get_player_setting(owner_id)
            .map(|player| player.get_commander())
            .unwrap_or(CommanderType(0))
    }

    pub fn get_hero(&self, owner_id: i8) -> Option<HeroType> {
        self.get_player_setting(owner_id)
            .map(|player| player.get_hero())
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

    pub fn unit_transport_visibility<D: Direction>(&self, _game: &Board<D>, _unit: &Unit<D>, _p: Point, _heroes: &[HeroInfluence<D>]) -> UnitVisibility {
        // TODO
        UnitVisibility::Normal
    }

    pub fn hero_visibility<D: Direction>(&self, _game: &Board<D>, _unit: &Unit<D>, _p: Point, _hero: HeroType) -> UnitVisibility {
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
        Urc::ptr_eq(&self.config, &other.config)
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
