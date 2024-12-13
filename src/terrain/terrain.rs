use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::Arc;

use interfaces::ClientPerspective;
use num_rational::Rational32;
use zipper::*;

use crate::commander::commander_type::CommanderType;
use crate::commander::Commander;
use crate::config::environment::Environment;
use crate::config::OwnershipPredicate;
use crate::game::fog::{FogIntensity, FogSetting};
use crate::game::game_view::GameView;
use crate::game::settings::GameSettings;
use crate::map::direction::Direction;
use crate::map::point::Point;
use crate::map::wrapping_map::Distortion;
use crate::player::{Owner, Player};
use crate::tags::*;
use crate::units::hero::HeroInfluence;
use crate::units::movement::MovementType;
use crate::units::UnitVisibility;

use super::{TerrainType, ExtraMovementOptions};

#[derive(Clone, PartialEq, Eq)]
pub struct Terrain<D: Direction> {
    environment: Environment,
    typ: TerrainType,
    owner: Owner,
    tags: TagBag<D>,
    //attributes: FxHashMap<TerrainAttributeKey, TerrainAttribute>,
}

impl<D: Direction> Debug for Terrain<D> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}(", self.name())?;
        write!(f, "Owner: {}, ", self.owner.0)?;
        self.tags.debug(f, &self.environment)?;
        /*let mut keys: Vec<_> = self.attributes.keys().collect();
        keys.sort();
        for key in keys {
            write!(f, "{:?}", self.attributes.get(key).unwrap())?;
        }*/
        write!(f, ")")
    }
}

impl<D: Direction> Terrain<D> {
    pub(super) fn new(environment: Environment, typ: TerrainType) -> Self {
        let owner = match environment.config.terrain_ownership(typ) {
            OwnershipPredicate::Always => environment.config.max_player_count() - 1,
            _ => -1
        };
        Self {
            environment,
            typ,
            owner: Owner(owner),
            tags: TagBag::new()
            //attributes: FxHashMap::default(),
        }
    }

    pub(crate) fn start_game(&mut self, settings: &Arc<GameSettings>) {
        self.environment.start_game(settings);
    }

    // getters that aren't influenced by attributes
    pub fn environment(&self) -> &Environment {
        &self.environment
    }

    pub fn typ(&self) -> TerrainType {
        self.typ
    }

    pub fn name(&self) -> &str {
        self.environment.config.terrain_name(self.typ)
    }

    /*pub fn get_capture_resistance(&self) -> u8 {
        self.environment.config.terrain_capture_resistance(self.typ)
    }

    pub fn get_amphibious(&self) -> Option<AmphibiousTyping> {
        self.environment.config.terrain_amphibious(self.typ)
    }*/

    pub fn is_chess(&self) -> bool {
        self.environment.config.terrain_chess(self.typ)
    }

    /*pub fn attack_bonus(&self, unit: &Unit<D>) -> Rational32 {
        let bonus = self.environment.config.terrain_attack_bonus(self.typ, unit.sub_movement_type());
        bonus
    }
    
    pub fn defense_bonus(&self, unit: &Unit<D>) -> Rational32 {
        let bonus = self.environment.config.terrain_defense_bonus(self.typ, unit.sub_movement_type());
        bonus
    }*/

    pub fn income_factor(
        &self,
        game: &impl GameView<D>,
        pos: Point,
        // the heroes affecting this terrain. shouldn't be taken from game since they could have died before this function is called
        heroes: &[HeroInfluence<D>],
    ) -> Rational32 {
        self.environment.config.terrain_income_factor(game, pos, self, heroes)
    }

    pub fn vision_range(
        &self,
        game: &impl GameView<D>,
        pos: Point,
        // the heroes affecting this terrain. shouldn't be taken from game since they could have died before this function is called
        heroes: &[HeroInfluence<D>],
    ) -> Option<usize> {
        /*if self.has_attribute(TerrainAttributeKey::Owner) && self.get_team() == ClientPerspective::Neutral {
            return None;
        }*/
        let mut range = self.environment.config.terrain_vision_range(game, pos, self, heroes)?;
        // TODO: add config column for whether fog_setting should increase vision range instead of this check
        if range == 0 {
            return Some(range);
        }
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

    /*pub fn can_build(
        &self,
        game: &impl GameView<D>,
        pos: Point,
        // the heroes affecting this terrain. shouldn't be taken from game since they could have died before this function is called
        heroes: &[HeroInfluence<D>],
    ) -> bool {
        self.environment.config.terrain_can_build(game, pos, self, heroes)
    }

    pub fn buildable_units(
        &self,
        game: &impl GameView<D>,
        pos: Point,
        _is_bubble: bool,
        // the heroes affecting this terrain. shouldn't be taken from game since they could have died before this function is called
        heroes: &[HeroInfluence<D>],
    ) -> &[UnitType] {
        if self.can_build(game, pos, heroes) {
            self.environment.config.terrain_build(self.typ)
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
    }*/

    pub fn extra_step_options(&self) -> ExtraMovementOptions {
        self.environment.config.terrain_path_extra(self.typ)
    }

    pub fn movement_cost(&self, movement_type: MovementType) -> Option<Rational32> {
        self.environment.config.terrain_movement_cost(self.typ, movement_type)
    }

    // getters + setters that relate to attributes

    /*pub fn has_attribute(&self, key: TerrainAttributeKey) -> bool {
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
    }*/

    pub fn get_owner_id(&self) -> i8 {
        self.owner.0
    }
    pub fn set_owner_id(&mut self, id: i8) {
        match self.environment.config.terrain_ownership(self.typ) {
            OwnershipPredicate::Always if id < 0 => (),
            OwnershipPredicate::Never if id >= 0 => (),
            _ => {
                self.owner.0 = id;
            }
        }
            /*let _owner_before = self.get_owner_id();
            self.set(Owner(id.max(-1).min(self.environment.config.max_player_count() - 1)));
            /*let co_before = self.environment.config.commander_attributes(self.typ, owner_before);
            let co_after = self.environment.config.commander_attributes(self.typ, self.get_owner_id());
            for key in co_before.iter().filter(|k| !co_after.contains(k)) {
                self.attributes.remove(key);
            }
            for key in co_after.iter().filter(|k| !co_before.contains(k)) {
                self.attributes.insert(*key, key.default(self.typ, &self.environment));
            }*/*/
    }

    pub fn get_team(&self) -> ClientPerspective {
        self.environment.get_team(self.get_owner_id())
    }

    pub fn get_player(&self, game: &impl GameView<D>) -> Option<Player> {
        game.get_owning_player(self.get_owner_id())
    }

    pub fn get_commander(&self, game: &impl GameView<D>) -> Commander {
        self.get_player(game)
        .and_then(|player| Some(player.commander.clone()))
        .unwrap_or(Commander::new(&self.environment, CommanderType(0)))
    }

    pub(super) fn copy_from(&mut self, other: &Terrain<D>) {
        if self.environment != other.environment {
            panic!("Can't copy from terrain from different environment");
        }
        for key in other.tags.flags() {
            self.set_flag(*key);
        }
        for (key, value) in other.tags.tags() {
            self.set_tag(*key, value.clone());
        }
    }
    pub fn get_tag_bag(&self) -> &TagBag<D> {
        &self.tags
    }

    pub fn has_flag(&self, key: usize) -> bool {
        self.tags.has_flag(key)
    }
    pub fn set_flag(&mut self, key: usize) {
        self.tags.set_flag(&self.environment, key);
    }
    pub fn remove_flag(&mut self, key: usize) {
        self.tags.remove_flag(key);
    }
    pub fn flip_flag(&mut self, key: usize) {
        if self.has_flag(key) {
            self.remove_flag(key);
        } else {
            self.set_flag(key);
        }
    }

    pub fn get_tag(&self, key: usize) -> Option<TagValue<D>> {
        self.tags.get_tag(key)
    }
    pub fn set_tag(&mut self, key: usize, value: TagValue<D>) {
        self.tags.set_tag(&self.environment, key, value);
    }
    pub fn remove_tag(&mut self, key: usize) {
        self.tags.remove_tag(key);
    }

    /*pub fn get_capture_progress(&self) -> CaptureProgress {
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
    pub fn max_built_this_turn(&self) -> u8 {
        self.environment.config.terrain_max_builds_per_turn(self.typ)
    }

    pub fn get_exhausted(&self) -> bool {
        self.get::<Exhausted>().0
    }
    pub fn set_exhausted(&mut self, exhausted: bool) {
        self.set(Exhausted(exhausted));
    }*/

    // methods that go beyond getter / setter functionality

    pub fn get_vision(
        &self,
        game: &impl GameView<D>,
        pos: Point,
        // the heroes affecting this terrain. shouldn't be taken from game since they could have died before this function is called
        heroes: &[HeroInfluence<D>],
        team: ClientPerspective
    ) -> HashMap<Point, FogIntensity> {
        if self.get_team() != team && self.get_team() != ClientPerspective::Neutral {
            return HashMap::new();
        }
        let vision_range = if let Some(v) = self.vision_range(game, pos, heroes) {
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
        let layers = game.range_in_layers(pos, vision_range);
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
        if intensity == FogIntensity::TrueSight {
            return self.clone();
        }
        let visibility = match intensity {
            FogIntensity::TrueSight => return self.clone(),
            FogIntensity::NormalVision => UnitVisibility::Normal,
            FogIntensity::Light => UnitVisibility::Normal,
            FogIntensity::Dark => UnitVisibility::AlwaysVisible,
        };
        let mut builder = self.typ.instance(&self.environment)
            .set_tag_bag(self.tags.fog_replacement(&self.environment, visibility));
        if self.environment.config.terrain_owner_visibility(self.typ) >= visibility {
            builder = builder.set_owner_id(self.owner.0);
        }
        builder.build_with_defaults()
        /*let hidden_attributes = self.environment.config.terrain_specific_hidden_attributes(self.typ);
        let attributes = self.attributes.iter()
        .filter(|(key, _)| !hidden_attributes.contains(key))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
        Self {
            typ: self.typ,
            environment: self.environment.clone(),
            attributes,
        }*/
    }

    /*pub fn can_sell_hero(&self, map: &impl GameView<D>, pos: Point, owner_id: i8) -> bool {
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
    }*/

    /*fn unit_build_overrides(&self, game: &impl GameView<D>, position: Point, heroes: &[HeroInfluence<D>]) -> HashSet<AttributeOverride> {
        self.environment.config.terrain_unit_attribute_overrides(
            game,
            self,
            position,
            heroes,
        )
        .values()
        .cloned()
        .collect()
    }

    pub(crate) fn unit_shop_option(&self, game: &impl GameView<D>, pos: Point, unit_type: UnitType, heroes: &[HeroInfluence<D>]) -> (Unit<D>, i32) {
        let attr_overrides = self.unit_build_overrides(game, pos, heroes);
        let mut builder: UnitBuilder<D> = unit_type.instance(&self.environment)
        .set_status(ActionStatus::Exhausted);
        for attr in &attr_overrides {
            builder = builder.set_attribute(&attr.into());
        }
        let unit = builder
        .set_owner_id(self.get_owner_id())
        .build_with_defaults();
        let cost = unit.value(game, pos, None, &heroes)
        + self.get_built_this_turn() as i32 * self.environment.built_this_turn_cost_factor();
        (unit, cost)
    }

    pub fn unit_shop(&self, game: &impl GameView<D>, pos: Point, is_bubble: bool) -> Vec<(Unit<D>, i32)> {
        let heroes = Hero::hero_influence_at(game, pos, self.get_owner_id());
        if !self.can_build(game, pos, &heroes) {
            return Vec::new();
        }
        self.buildable_units(game, pos, is_bubble, &heroes).iter().map(|unit_type| {
            self.unit_shop_option(game, pos, *unit_type, &heroes)
        }).collect()
    }*/

    /*pub fn on_start_turn(&self, game: &impl GameView<D>, pos: Point, heroes: &[HeroInfluence<D>]) -> Vec<usize> {
        self.environment.config.terrain_on_start_turn(
            game,
            pos,
            self,
            &heroes,
        )
    }*/

    /*pub fn on_build(&self, game: &impl GameView<D>, pos: Point, is_bubble: bool) -> Vec<usize> {
        let heroes = Hero::hero_influence_at(game, pos, self.get_owner_id());
        self.environment.config.terrain_on_build(
            game,
            pos,
            self,
            is_bubble,
            &heroes,
        )
    }*/

    pub fn distort(&mut self, distortion: Distortion<D>) {
        self.tags.distort(distortion);
    }
}

impl<D: Direction> SupportedZippable<&Environment> for Terrain<D> {
    fn export(&self, zipper: &mut Zipper, support: &Environment) {
        self.typ.export(zipper, support);
        if support.config.terrain_ownership(self.typ) != OwnershipPredicate::Never {
            self.owner.export(zipper, &*self.environment.config);
        }
        self.tags.export(zipper, &self.environment);
        /*for key in support.config.terrain_specific_attributes(self.typ) {
            let value = key.default();
            let value = self.attributes.get(key).unwrap_or(&value);
            value.export(zipper, support, self.typ);
        }*/
    }
    fn import(unzipper: &mut Unzipper, support: &Environment) -> Result<Self, ZipperError> {
        let typ = TerrainType::import(unzipper, support)?;
        let owner = if support.config.terrain_ownership(typ) != OwnershipPredicate::Never {
            Owner::import(unzipper, &*support.config)?
        } else {
            Owner(-1)
        };
        let tags = TagBag::import(unzipper, support)?;
        /*let mut attributes = FxHashMap::default();
        for key in support.config.terrain_specific_attributes(typ) {
            let attr = TerrainAttribute::import(unzipper, support, *key, typ)?;
            attributes.insert(*key, attr);
        }*/
        Ok(Self {
            environment: support.clone(),
            typ,
            owner,
            tags,
            //attributes,
        })
    }
}

#[derive(Clone)]
pub struct TerrainBuilder<D: Direction> {
    terrain: Terrain<D>,
}

impl<D: Direction> TerrainBuilder<D> {
    pub fn new(environment: &Environment, typ: TerrainType) -> Self {
        let terrain = Terrain::new(environment.clone(), typ);
        Self {
            terrain,
        }
    }

    pub fn copy_from(mut self, other: &Terrain<D>) -> Self {
        self.terrain.copy_from(other);
        self
    }

    pub fn set_tag_bag(mut self, bag: TagBag<D>) -> Self {
        self.terrain.tags = bag;
        self
    }

    pub fn set_flag(mut self, key: usize) -> Self {
        self.terrain.set_flag(key);
        self
    }
    pub fn remove_flag(mut self, key: usize) -> Self {
        self.terrain.remove_flag(key);
        self
    }

    pub fn set_tag(mut self, key: usize, value: TagValue<D>) -> Self {
        self.terrain.set_tag(key, value);
        self
    }
    pub fn remove_tag(mut self, key: usize) -> Self {
        self.terrain.remove_tag(key);
        self
    }

    /*pub fn set_attribute(mut self, attribute: &TerrainAttribute) -> Self {
        let key = attribute.key();
        if self.terrain.has_attribute(key) {
            self.terrain.attributes.insert(key, attribute.clone());
        }
        self
    }*/

    pub fn set_owner_id(mut self, id: i8) -> Self {
        self.terrain.set_owner_id(id);
        self
    }

    /*pub fn set_capture_progress(mut self, progress: CaptureProgress) -> Self {
        self.terrain.set_capture_progress(progress);
        self
    }

    pub fn set_anger(mut self, anger: u8) -> Self {
        self.terrain.set_anger(anger);
        self
    }*/

    pub fn build(&self) -> Terrain<D> {
        self.terrain.clone()
    }

    /**
     * TODO: call rhai script to get default flags/tag values?
     */
    pub fn build_with_defaults(&self) -> Terrain<D> {
        self.terrain.clone()
    }
}
