use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::Arc;

use interfaces::game_interface::ClientPerspective;
use num_rational::Rational32;
use rustc_hash::FxHashMap;
use zipper::*;

use crate::config::environment::Environment;
use crate::game::fog::{FogIntensity, FogSetting};
use crate::game::game::Game;
use crate::game::settings::GameSettings;
use crate::map::direction::Direction;
use crate::map::map_view::MapView;
use crate::map::point::Point;
use crate::player::Owner;
use crate::units::attributes::ActionStatus;
use crate::units::hero::Hero;
use crate::units::movement::MovementType;
use crate::units::unit::{Unit, UnitBuilder};
use crate::units::unit_types::UnitType;

use super::{TerrainType, AmphibiousTyping, ExtraMovementOptions};
use super::attributes::*;

#[derive(Clone, PartialEq, Eq)]
pub struct Terrain {
    environment: Environment,
    typ: TerrainType,
    attributes: FxHashMap<TerrainAttributeKey, TerrainAttribute>,
}

impl Debug for Terrain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}(", self.name())?;
        let mut keys: Vec<_> = self.attributes.keys().collect();
        keys.sort();
        for key in keys {
            write!(f, "{:?}", self.attributes.get(key).unwrap())?;
        }
        write!(f, ")")
    }
}

impl Terrain {
    pub(super) fn new(environment: Environment, typ: TerrainType) -> Self {
        Self {
            environment,
            typ,
            attributes: FxHashMap::default(),
        }
    }

    pub(crate) fn start_game(&mut self, settings: &Arc<GameSettings>) {
        self.environment.start_game(settings);
    }

    // getters that aren't influenced by attributes

    pub fn typ(&self) -> TerrainType {
        self.typ
    }

    pub fn name(&self) -> &str {
        self.environment.config.terrain_name(self.typ)
    }

    pub fn get_capture_resistance(&self) -> u8 {
        self.environment.config.terrain_capture_resistance(self.typ)
    }

    pub fn get_amphibious(&self) -> Option<AmphibiousTyping> {
        self.environment.config.terrain_amphibious(self.typ)
    }

    pub fn is_chess(&self) -> bool {
        self.environment.config.terrain_chess(self.typ)
    }

    pub fn attack_bonus<D: Direction>(&self, unit: &Unit<D>) -> Rational32 {
        let bonus = self.environment.config.terrain_attack_bonus(self.typ, unit.default_movement_type());
        bonus
    }
    
    pub fn defense_bonus<D: Direction>(&self, unit: &Unit<D>) -> Rational32 {
        let bonus = self.environment.config.terrain_defense_bonus(self.typ, unit.default_movement_type());
        bonus
    }

    pub fn income_factor(&self) -> i32 {
        self.environment.config.terrain_income_factor(self.typ) as i32
    }

    pub fn vision_range<D: Direction>(&self, game: &Game<D>) -> Option<usize> {
        let mut range = self.environment.config.terrain_vision_range(self.typ)? as usize;
        match game.get_fog_setting() {
            FogSetting::None => (),
            FogSetting::Light(bonus) |
            FogSetting::Sharp(bonus) |
            FogSetting::Fade1(bonus) |
            FogSetting::Fade2(bonus) |
            FogSetting::ExtraDark(bonus) => range += bonus as usize,
        }
        Some(range)
    }

    pub fn can_build(&self) -> bool {
        self.environment.config.terrain_can_build(self.typ)
    }

    pub fn buildable_units(&self) -> &[UnitType] {
        if self.can_build() {
            self.environment.config.terrain_build_or_repair(self.typ)
        } else {
            &[]
        }
    }

    pub fn can_repair(&self) -> bool {
        self.environment.config.terrain_can_repair(self.typ)
    }

    pub fn can_repair_unit(&self, unit: UnitType) -> bool {
        self.environment.config.terrain_can_repair(self.typ)
        && self.environment.config.terrain_build_or_repair(self.typ).contains(&unit)
    }

    pub fn could_sell_hero(&self) -> bool {
        self.environment.config.terrain_sells_hero(self.typ)
    }

    pub fn extra_step_options(&self) -> ExtraMovementOptions {
        self.environment.config.terrain_path_extra(self.typ)
    }

    pub fn movement_cost(&self, movement_type: MovementType) -> Option<Rational32> {
        self.environment.config.terrain_movement_cost(self.typ, movement_type)
    }

    // getters + setters that relate to attributes

    pub fn has_attribute(&self, key: TerrainAttributeKey) -> bool {
        // TODO: consider all attributes, not just terrain-specific ones
        self.environment.config.terrain_specific_attributes(self.typ).contains(&key)
    }

    fn get<T: TrAttribute>(&self) -> T {
        if let Some(a) = self.attributes.get(&T::key()) {
            T::try_from(a.clone()).expect("Impossible! attribute of wrong type")
        } else {
            //println!("Terrain of type {:?} doesn't have {} attribute, but it was requested anyways", self.typ, T::key());
            T::try_from(T::key().default()).expect("Impossible! attribute defaults to wrong type")
        }
    }

    fn set<T: TrAttribute>(&mut self, value: T) -> bool {
        if self.has_attribute(T::key()) {
            self.attributes.insert(T::key(), value.into());
            true
        } else {
            false
        }
    }

    pub fn get_owner_id(&self) -> i8 {
        self.get::<Owner>().0
    }
    pub fn set_owner_id(&mut self, id: i8) {
        if id >= 0 || !self.environment.config.terrain_needs_owner(self.typ) {
            let owner_before = self.get_owner_id();
            self.set(Owner(id.max(-1).min(self.environment.config.max_player_count() - 1)));
            /*let co_before = self.environment.config.commander_attributes(self.typ, owner_before);
            let co_after = self.environment.config.commander_attributes(self.typ, self.get_owner_id());
            for key in co_before.iter().filter(|k| !co_after.contains(k)) {
                self.attributes.remove(key);
            }
            for key in co_after.iter().filter(|k| !co_before.contains(k)) {
                self.attributes.insert(*key, key.default(self.typ, &self.environment));
            }*/
        }
    }

    pub fn get_team(&self) -> ClientPerspective {
        self.environment.get_team(self.get_owner_id())
    }

    pub fn get_capture_progress(&self) -> CaptureProgress {
        self.get()
    }
    pub fn set_capture_progress(&mut self, progress: CaptureProgress) {
        self.set(progress);
    }

    pub fn get_anger(&self) -> u8 {
        self.get::<Anger>().0
    }
    pub fn set_anger(&mut self, anger: u8) {
        self.set(Anger(anger));
    }

    pub fn get_built_this_turn(&self) -> u8 {
        self.get::<BuiltThisTurn>().0
    }
    pub fn set_built_this_turn(&mut self, built_this_turn: u8) {
        self.set(BuiltThisTurn(built_this_turn));
    }

    // methods that go beyond getter / setter functionality

    pub fn get_vision<D: Direction>(&self, game: &Game<D>, pos: Point, team: ClientPerspective) -> HashMap<Point, FogIntensity> {
        if self.get_team() != team {
            return HashMap::new();
        }
        let vision_range = if let Some(v) = self.vision_range(game) {
            v
        } else {
            return HashMap::new();
        };
        let mut result = HashMap::new();
        result.insert(pos, FogIntensity::TrueSight);
        let normal_range = match game.get_fog_setting() {
            FogSetting::ExtraDark(_) => 0,
            FogSetting::Fade1(_) => 1.max(vision_range) - 1,
            FogSetting::Fade2(_) => 2.max(vision_range) - 2,
            _ => vision_range
        };
        let layers = game.get_map().range_in_layers(pos, vision_range);
        for (i, layer) in layers.into_iter().enumerate() {
            for p in layer {
                let vision = if i < normal_range {
                    FogIntensity::NormalVision
                } else {
                    FogIntensity::Light
                };
                result.insert(p, vision.min(result.get(&p).cloned().unwrap_or(FogIntensity::Dark)));
            }
        }
        result
    }

    pub fn fog_replacement(&self, intensity: FogIntensity) -> Self {
        if intensity != FogIntensity::Dark {
            return self.clone();
        }
        let hidden_attributes = self.environment.config.terrain_specific_hidden_attributes(self.typ);
        let attributes = self.attributes.iter()
        .filter(|(key, _)| !hidden_attributes.contains(key))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
        Self {
            typ: self.typ,
            environment: self.environment.clone(),
            attributes,
        }
    }

    pub fn can_sell_hero<D: Direction>(&self, map: &impl MapView<D>, pos: Point, owner_id: i8) -> bool {
        if !self.could_sell_hero() {
            return false;
        }
        if self.has_attribute(TerrainAttributeKey::Owner) && self.get_owner_id() != owner_id {
            return false;
        }
        for p in map.all_points() {
            if let Some(unit) = map.get_unit(p) {
                if unit.get_owner_id() == owner_id {
                    // check if unit is mercenary or transports a mercenary
                    if unit.get_hero().get_origin() == Some(pos) {
                        return false;
                    }
                    for unit in unit.get_transported() {
                        if unit.get_hero().get_origin() == Some(pos) {
                            return false;
                        }
                    }
                }
            }
        }
        true
    }

    pub(crate) fn unit_shop_option<D: Direction>(&self, game: &Game<D>, pos: Point, unit_type: UnitType, heroes: &[(Unit<D>, Hero, Point, Option<usize>)]) -> (Unit<D>, i32) {
        let builder: UnitBuilder<D> = unit_type.instance(&self.environment)
        .set_status(ActionStatus::Exhausted);
        // TODO: terrain build-overrides for commanders, heroes
        /*for attr in terrain.build_overrides(&heroes) {
            builder = builder.set_attribute(&attr.into());
        }*/
        let unit = builder
        .set_owner_id(self.get_owner_id())
        .build_with_defaults();
        let cost = unit.full_price(game, pos, None, &heroes)
        + self.get_built_this_turn() as i32 * self.environment.built_this_turn_cost_factor();
        (unit, cost)
    }

    pub fn unit_shop<D: Direction>(&self, game: &Game<D>, pos: Point) -> Vec<(Unit<D>, i32)> {
        if !self.can_build() {
            return Vec::new();
        }
        let heroes = Hero::hero_influence_at(game, pos, self.get_owner_id());
        self.buildable_units().iter().map(|unit_type| {
            self.unit_shop_option(game, pos, *unit_type, &heroes)
        }).collect()
    }
}

impl SupportedZippable<&Environment> for Terrain {
    fn export(&self, zipper: &mut Zipper, support: &Environment) {
        self.typ.export(zipper, support);
        for key in support.config.terrain_specific_attributes(self.typ) {
            let value = key.default();
            let value = self.attributes.get(key).unwrap_or(&value);
            value.export(zipper, support, self.typ);
        }
    }
    fn import(unzipper: &mut Unzipper, support: &Environment) -> Result<Self, ZipperError> {
        let typ = TerrainType::import(unzipper, support)?;
        let mut attributes = FxHashMap::default();
        for key in support.config.terrain_specific_attributes(typ) {
            let attr = TerrainAttribute::import(unzipper, support, *key, typ)?;
            attributes.insert(*key, attr);
        }
        Ok(Self {
            environment: support.clone(),
            typ,
            attributes,
        })
    }
}

#[derive(Clone)]
pub struct TerrainBuilder {
    terrain: Terrain,
}

impl TerrainBuilder {
    pub fn new(environment: &Environment, typ: TerrainType) -> Self {
        let mut terrain = Terrain::new(environment.clone(), typ);
        terrain.set_owner_id(-1);
        Self {
            terrain,
        }
    }

    pub fn copy_from(mut self, other: &Terrain) -> Self {
        if self.terrain.environment != other.environment {
            panic!("Can't copy from terrain from different environment");
        }
        for (key, value) in &other.attributes {
            if self.terrain.has_attribute(*key) {
                self.terrain.attributes.insert(*key, value.clone());
            }
        }
        self
    }

    pub fn set_attribute(mut self, attribute: &TerrainAttribute) -> Self {
        let key = attribute.key();
        if self.terrain.has_attribute(key) {
            self.terrain.attributes.insert(key, attribute.clone());
        }
        self
    }

    pub fn set_owner_id(mut self, id: i8) -> Self {
        self.terrain.set_owner_id(id);
        self
    }

    pub fn set_capture_progress(mut self, progress: CaptureProgress) -> Self {
        self.terrain.set_capture_progress(progress);
        self
    }

    pub fn build(&self) -> Option<Terrain> {
        for key in self.terrain.environment.config.terrain_specific_attributes(self.terrain.typ()) {
            if !self.terrain.attributes.contains_key(key) {
                return None;
            }
        }
        Some(self.terrain.clone())
    }

    /**
     * Take Care! The following attributes don't have reasonable defaults:
     *  - owner_id
     */
    pub fn build_with_defaults(&self) -> Terrain {
        let mut terrain = self.terrain.clone();
        for key in self.terrain.environment.config.terrain_specific_attributes(self.terrain.typ()) {
            if !terrain.attributes.contains_key(key) {
                /*if *key == AttributeKey::DroneId || *key == AttributeKey::DroneStationId || *key == AttributeKey::Owner {
                    println!("WARNING: building terrain with missing Attribute {key}");
                    //return Err(AttributeError { requested: *key, received: None });
                }*/
                terrain.attributes.insert(*key, key.default());
            }
        }
        terrain
    }
}
