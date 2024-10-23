use std::collections::{HashSet, HashMap};
use std::fmt::Debug;
use std::hash::Hash;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;

use executor::Executor;
use interfaces::ClientPerspective;
use num_rational::Rational32;
use rhai::Scope;
use zipper::*;

use crate::commander::commander_type::CommanderType;
use crate::config::environment::Environment;
use crate::config::movement_type_config::MovementPattern;
use crate::game::fog::{is_unit_attribute_visible, FogIntensity, FogSetting, VisionMode};
use crate::game::game_view::GameView;
use crate::game::modified_view::*;
use crate::game::settings::GameSettings;
use crate::commander::Commander;
use crate::map::direction::Direction;
use crate::map::point::Point;
use crate::map::wrapping_map::Distortion;
use crate::player::{Player, Owner};
use crate::script::*;
use crate::tags::{TagBag, TagValue};

use super::UnitVisibility;
use super::combat::*;
use super::commands::UnitAction;
use super::movement::*;
use super::unit_types::UnitType;
use super::hero::*;


#[derive(Clone, Eq)]
pub struct Unit<D: Direction> {
    environment: Environment,
    typ: UnitType,
    owner: Owner,
    sub_movement_type: MovementType, // in case the unit has multiple movement types
    hero: Option<Hero>,
    tags: TagBag<D>,
    transport: Vec<Self>,
    //attributes: HashMap<AttributeKey, Attribute<D>>,
}

impl<D: Direction> PartialEq for Unit<D> {
    // compare everything except environment
    fn eq(&self, other: &Self) -> bool {
        self.typ == other.typ
        && self.owner == other.owner
        && self.sub_movement_type == other.sub_movement_type
        && self.hero == other.hero
        && self.tags == other.tags
        //&& self.attributes == other.attributes
    }
}

impl<D: Direction> Debug for Unit<D> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}(", self.name())?;
        write!(f, "Owner: {}", self.owner.0)?;
        if let Some(hero) = &self.hero {
            write!(f, "Hero: {hero:?}")?;
        }
        self.tags.debug(f, &self.environment)?;
        if self.transport.len() > 0 {
            write!(f, "Transporting: {:?}", self.transport)?;
        }
        write!(f, ")")
    }
}

impl<D: Direction> Unit<D> {
    fn new(environment: Environment, typ: UnitType) -> Self {
        Self {
            typ,
            owner: Owner(-1),
            sub_movement_type: environment.config.sub_movement_types(environment.config.base_movement_type(typ))[0],
            hero: None,
            tags: TagBag::new(),
            transport: Vec::new(),
            //attributes: HashMap::default(),
            environment,
        }
    }

    pub(crate) fn start_game(&mut self, settings: &Arc<GameSettings>) {
        self.environment.start_game(settings);
        for unit in self.get_transported_mut().deref_mut() {
            unit.start_game(settings);
        }
        /*for key in self.environment.unit_attributes(self.typ, self.get_owner_id()) {
            if !self.attributes.contains_key(key) {
                self.attributes.insert(*key, key.default());
            }
        }*/
    }

    // getters that aren't influenced by attributes
    pub fn environment(&self) -> &Environment {
        &self.environment
    }

    /*pub(crate) fn get_attributes(&self) -> &HashMap<AttributeKey, Attribute<D>> {
        &self.attributes
    }*/

    pub fn typ(&self) -> UnitType {
        self.typ
    }

    pub fn name(&self) -> &str {
        self.environment.config.unit_name(self.typ)
    }

    pub fn transportable_units(&self) -> &[UnitType] {
        self.environment.config.unit_transportable(self.typ)
    }
    pub fn could_transport(&self, other: UnitType) -> bool {
        self.transportable_units().contains(&other)
    }
    pub fn transport_capacity(&self) -> usize {
        self.environment.unit_transport_capacity(self.typ, self.get_owner_id(), self.get_hero().map(|hero| hero.typ()).unwrap_or(HeroType::None))
    }

    pub fn movement_pattern(&self) -> MovementPattern {
        self.environment.config.movement_pattern(self.typ)
    }

    pub fn base_movement_type(&self) -> MovementType {
        self.environment.config.base_movement_type(self.typ)
    }
    pub fn sub_movement_type(&self) -> MovementType {
        self.sub_movement_type
    }
    pub fn set_sub_movement_type(&mut self, sub_movement_type: MovementType) {
        if self.environment.config.sub_movement_types(self.environment.config.base_movement_type(self.typ)).contains(&sub_movement_type) {
            self.sub_movement_type = sub_movement_type;
        }
    }

    pub fn base_movement_points(&self) -> Rational32 {
        self.environment.config.base_movement_points(self.typ)
    }

    pub fn has_stealth(&self) -> bool {
        self.environment.config.has_stealth(self.typ)
    }

    pub fn has_stealth_movement(&self, game: &impl GameView<D>) -> bool {
        self.has_stealth() && !game.is_foggy()
    }

    pub fn can_be_moved_through(&self) -> bool {
        self.environment.config.can_be_moved_through(self.typ)
    }

    /*pub fn can_take(&self) -> bool {
        self.environment.config.can_take(self.typ)
    }*/

    pub fn can_be_taken(&self) -> bool {
        self.environment.config.can_be_taken(self.typ)
    }

    /*pub fn can_have_status(&self, status: ActionStatus) -> bool {
        self.has_attribute(AttributeKey::ActionStatus)
        && self.environment.unit_valid_action_status(self.typ, self.get_owner_id()).contains(&status)
    }*/

    pub fn weapon(&self) -> WeaponType {
        self.environment.config.weapon(self.typ)
    }

    pub fn can_attack(&self) -> bool {
        self.environment.config.can_attack(self.typ)
    }

    pub fn can_attack_after_moving(&self) -> bool {
        self.environment.config.can_attack_after_moving(self.typ)
    }

    pub fn attack_pattern(&self, game: &impl GameView<D>, unit_pos: Point, counter: Counter<D>, heroes: &[HeroInfluence<D>], temporary_ballast: &[TBallast<D>]) -> AttackType {
        self.environment.config.unit_attack_pattern(game, self, unit_pos, counter, heroes, temporary_ballast)
    }

    pub fn attack_targeting(&self) -> AttackTargeting {
        self.environment.config.attack_targeting(self.typ)
    }

    pub fn base_damage(&self, defender: UnitType) -> Option<u16> {
        self.environment.config.base_damage(self.typ, defender)
    }

    pub fn displacement(&self) -> Displacement {
        self.environment.config.displacement(self.typ)
    }

    pub fn displacement_distance(&self, game: &impl GameView<D>, pos: Point, transporter: Option<(&Unit<D>, Point)>, heroes: &[HeroInfluence<D>], temporary_ballast: &[TBallast<D>], is_counter: bool) -> i8 {
        self.environment.config.unit_displacement_distance(game, self, pos, transporter, heroes, temporary_ballast, is_counter)
    }

    pub fn can_be_displaced(&self) -> bool {
        self.environment.config.can_be_displaced(self.typ)
    }

    pub fn vision_mode(&self) -> VisionMode {
        self.environment.config.vision_mode(self.typ)
    }

    pub fn vision_range(&self, game: &impl GameView<D>, pos: Point, heroes: &[HeroInfluence<D>]) -> usize {
        let mut range = self.environment.config.unit_vision(game, self, pos, heroes);
        match game.get_fog_setting() {
            FogSetting::None => (),
            FogSetting::Light(bonus) |
            FogSetting::Sharp(bonus) |
            FogSetting::Fade1(bonus) |
            FogSetting::Fade2(bonus) |
            FogSetting::ExtraDark(bonus) => range += bonus as usize,
        }
        range
    }

    fn true_vision_range(&self, game: &impl GameView<D>, pos: Point, heroes: &[HeroInfluence<D>]) -> usize {
        self.environment.config.unit_true_vision(game, self, pos, heroes)
    }

    pub fn get_vision(&self, game: &impl GameView<D>, pos: Point, heroes: &[HeroInfluence<D>]) -> HashMap<Point, FogIntensity> {
        let mut result = HashMap::new();
        result.insert(pos, FogIntensity::TrueSight);
        let vision_range = self.vision_range(game, pos, heroes);
        let normal_range = match game.get_fog_setting() {
            FogSetting::ExtraDark(_) => 0,
            FogSetting::Fade1(_) => 1.max(vision_range) - 1,
            FogSetting::Fade2(_) => 2.max(vision_range) - 2,
            _ => vision_range
        };
        let true_vision_range = self.true_vision_range(game, pos, heroes);
        match self.vision_mode() {
            VisionMode::Normal => {
                let layers = game.range_in_layers(pos, vision_range);
                for (i, layer) in layers.into_iter().enumerate() {
                    for p in layer {
                        let vision = if i < true_vision_range {
                            FogIntensity::TrueSight
                        } else if i < normal_range {
                            FogIntensity::NormalVision
                        } else {
                            FogIntensity::Light
                        };
                        result.insert(p, vision.min(result.get(&p).cloned().unwrap_or(FogIntensity::Dark)));
                    }
                }
            }
            VisionMode::Movement => {
                let game = IgnoreUnits::new(game);
                movement_search_game(&game, self, &Path::new(pos), 1, None,
                    |_, path, destination, can_continue, can_stop_here, _| {
                    let vision = if path.steps.len() <= true_vision_range {
                        FogIntensity::TrueSight
                    } else if path.steps.len() <= normal_range {
                        FogIntensity::NormalVision
                    } else {
                        FogIntensity::Light
                    };
                    result.insert(destination, vision.min(result.get(&destination).cloned().unwrap_or(FogIntensity::Dark)));
                    if can_continue && path.steps.len() < vision_range {
                        if can_stop_here {
                            PathSearchFeedback::ContinueWithoutStopping
                        } else {
                            PathSearchFeedback::Continue
                        }
                    } else {
                        PathSearchFeedback::Rejected
                    }
                });            
            }
        }
        result
    }

    // getters and setters that care about attributes

    /*pub fn has_attribute(&self, key: AttributeKey) -> bool {
        self.environment.unit_attributes(self.typ, self.get_owner_id()).any(|a| *a == key)
    }

    fn get<T: TrAttribute<D>>(&self) -> T {
        if let Some(a) = self.attributes.get(&T::key()) {
            T::try_from(a.clone()).expect("Impossible! attribute of wrong type")
        } else {
            //println!("Units of type {:?} don't have {} attribute, but it was requested anyways", self.typ, T::key());
            T::try_from(T::key().default()).expect("Impossible! attribute defaults to wrong type")
        }
    }

    fn set<T: TrAttribute<D>>(&mut self, value: T) -> bool {
        self.set_attribute(value.into())
    }

    pub(crate) fn set_attribute(&mut self, value: Attribute<D>) -> bool {
        if self.has_attribute(value.key()) {
            self.attributes.insert(value.key(), value);
            true
        } else {
            false
        }
    }*/

    pub fn get_owner_id(&self) -> i8 {
        self.owner.0
    }
    pub fn set_owner_id(&mut self, id: i8) {
        if id < 0 && self.environment.config.unit_needs_owner(self.typ) {
            return;
        }
        self.owner.0 = id;
        self.fix_transported();
        /*if id >= 0 || !self.environment.config.unit_needs_owner(self.typ) {
            let owner_before = self.get_owner_id();
            self.set(Owner(id.max(-1).min(self.environment.config.max_player_count() - 1)));
            let co_before = self.environment.config.commander_attributes(self.environment.get_commander(owner_before), self.typ);
            let co_after = self.environment.config.commander_attributes(self.environment.get_commander(self.get_owner_id()), self.typ);
            for key in co_before.iter().filter(|k| !co_after.contains(k)) {
                self.attributes.remove(key);
            }
            for key in co_after.iter().filter(|k| !co_before.contains(k)) {
                self.attributes.insert(*key, key.default());
            }
            self.fix_transported();
        }*/
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
        .unwrap_or(Commander::new(&self.environment, CommanderType::None))
    }

    pub fn get_flag(&self, key: usize) -> bool {
        self.tags.get_flag(key)
    }
    pub fn set_flag(&mut self, key: usize) {
        self.tags.set_flag(&self.environment, key);
    }
    pub fn remove_flag(&mut self, key: usize) {
        self.tags.remove_flag(key);
    }
    pub fn flip_flag(&mut self, key: usize) {
        if self.get_flag(key) {
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

    // TODO: hardcoded for movement
    // replace when movement can be customized
    pub fn get_pawn_direction(&self) -> D {
        match self.environment.tag_by_name("PawnDirection")
        .and_then(|key| self.tags.get_tag(key)) {
            Some(TagValue::Direction(d)) => d,
            _ => D::angle_0()
        }
    }
    pub fn set_pawn_direction(&mut self, direction: D) {
        if let Some(key) = self.environment.tag_by_name("PawnDirection") {
            self.tags.set_tag(&self.environment, key, TagValue::Direction(direction));
        }
    }

    pub fn distort(&mut self, distortion: Distortion<D>) {
        self.tags.distort(distortion);
    }
    pub fn translate(&mut self, translations: [D::T; 2], odd_if_hex: bool) {
        self.tags.translate(translations, odd_if_hex);
    }

    pub fn is_hero(&self) -> bool {
        self.hero.is_some()
    }
    pub fn get_hero(&self) -> Option<&Hero> {
        self.hero.as_ref()
    }
    pub fn get_hero_mut(&mut self) -> Option<&mut Hero> {
        self.hero.as_mut()
    }
    pub fn set_hero(&mut self, hero: Hero) {
        // TODO: check if hero is compatible with this unit type
        self.hero = Some(hero);
        // update attributes influenced by the hero, e.g. Transported capacity
        self.fix_transported()
    }
    pub fn remove_hero(&mut self) {
        self.hero = None;
        // update attributes influenced by the hero, e.g. Transported capacity
        self.fix_transported()
    }

    pub fn get_max_charge(&self) -> u8 {
        self.hero.as_ref()
        .map(|hero| hero.max_charge(&self.environment))
        .unwrap_or(0)
    }
    pub fn get_charge(&self) -> u8 {
        self.hero.as_ref()
        .map(|hero| hero.get_charge())
        .unwrap_or(0)
    }
    pub fn set_charge(&mut self, charge: u8) {
        if let Some(hero) = &mut self.hero {
            hero.set_charge(&self.environment, charge);
        }
    }

    /*pub fn get_hp(&self) -> u8 {
        self.get::<Hp>().0
    }
    pub fn set_hp(&mut self, hp: u8) {
        self.set(Hp(hp.min(100)));
    }

    pub fn get_drone_id(&self) -> Option<u16> {
        if let Some(Attribute::DroneId(id)) = self.attributes.get(&AttributeKey::DroneId) {
            Some(*id)
        } else {
            None
        }
    }
    pub fn set_drone_id(&mut self, id: u16) {
        self.set(DroneId(id));
    }

    pub fn get_drone_station_id(&self) -> Option<u16> {
        if let Some(Attribute::DroneStationId(id)) = self.attributes.get(&AttributeKey::DroneStationId) {
            Some(*id)
        } else {
            None
        }
    }
    pub fn set_drone_station_id(&mut self, id: u16) {
        self.set(DroneStationId(id));
    }

    pub fn get_status(&self) -> ActionStatus {
        self.get()
    }
    pub fn set_status(&mut self, status: ActionStatus) {
        if self.can_have_status(status) {
            self.set(status);
        }
    }
    pub fn is_exhausted(&self) -> bool {
        self.get_status() != ActionStatus::Ready
    }*/

    /*pub fn can_capture(&self) -> bool {
        self.can_have_status(ActionStatus::Capturing)
    }*/

    pub fn can_transport(&self, other: &Self) -> bool {
        self.could_transport(other.typ)
        && self.environment == other.environment
        && self.get_transported().len() < self.transport_capacity()
    }
    pub fn remaining_transport_capacity(&self) -> usize {
        self.transport_capacity() - self.get_transported().len()
    }

    pub fn get_transported(&self) -> &[Unit<D>] {
        &self.transport
    }
    pub fn get_transported_mut<'a>(&'a mut self) -> TransportedRef<'a, D> {
        TransportedRef { unit: self }
    }
    fn fix_transported(&mut self) {
        // remove units that can't be transported by self, don't go over capacity
        let transportable = self.environment.config.unit_transportable(self.typ);
        let capacity = self.transport_capacity();
        self.transport = self.transport.drain(..)
        .filter(|other| {
            other.environment == self.environment
            && transportable.contains(&other.typ)
        })
        .take(capacity)
        .collect();
        // some attributes are defined by the transporter. make sure this stays consistent
    }

    /*pub fn get_unmoved(&self) -> bool {
        self.get::<Unmoved>().0
    }
    pub fn set_unmoved(&mut self, unmoved: bool) {
        self.set(Unmoved(unmoved));
    }*/

    // TODO: hardcoded for movement
    // replace when movement can be customized
    pub fn get_en_passant(&self) -> Option<Point> {
        match self.environment.tag_by_name("EnPassant")
        .and_then(|key| self.tags.get_tag(key)) {
            Some(TagValue::Point(p)) => Some(p),
            _ => None
        }
    }
    pub fn set_en_passant(&mut self, en_passant: Option<Point>) {
        if let Some(key) = self.environment.tag_by_name("EnPassant") {
            if let Some(p) = en_passant {
                self.tags.set_tag(&self.environment, key, TagValue::Point(p));
            } else {
                self.tags.remove_tag(key);
            }
        }
    }

    /*pub fn get_zombified(&self) -> bool {
        self.get::<Zombified>().0
    }
    pub fn set_zombified(&mut self, zombified: bool) {
        self.set(Zombified(zombified));
    }

    pub fn get_level(&self) -> u8 {
        self.get::<Level>().0
    }
    pub fn set_level(&mut self, level: u8) {
        self.set(Level(level.min(self.environment.config.max_unit_level())));
    }*/

    // influenced by unit_power_config

    // ignores current hp
    pub fn value(&self, game: &impl GameView<D>, position: Point, factory: Option<&Unit<D>>, heroes: &[HeroInfluence<D>]) -> i32 {
        self.environment.config.unit_value(
            game,
            self,
            position,
            factory,
            heroes,
        )
    }

    // full_price reduced by hp lost
    /*pub fn value(&self, game: &impl GameView<D>, position: Point) -> i32 {
        self.full_price(game, position, None, &[]) * self.get_hp() as i32 / 100
    }*/

    pub fn movement_points(&self, game: &impl GameView<D>, position: Point, transporter: Option<&Unit<D>>, heroes: &[HeroInfluence<D>]) -> Rational32 {
        self.environment.config.unit_movement_points(
            game,
            self,
            (position, None),
            transporter.map(|u| (u, position)),
            heroes,
        )
    }

    /*pub fn build_overrides(&self, game: &impl GameView<D>, position: Point, transporter: Option<(&Unit<D>, Point)>, heroes: &[HeroInfluence<D>], temporary_ballast: &[TBallast<D>]) -> HashSet<AttributeOverride> {
        let overrides = self.environment.config.unit_attribute_overrides(
            game,
            self,
            position,
            transporter,
            heroes,
            temporary_ballast,
        );
        overrides.values()
        .cloned()
        .collect()
    }*/

    // returns a list of damage factors
    // the first element is the selected target, the second element for points next to the target and so on
    pub fn get_splash_damage(&self, game: &impl GameView<D>, unit_pos: Point, heroes: &[HeroInfluence<D>], temporary_ballast: &[TBallast<D>], is_counter: bool) -> Vec<Rational32> {
        self.environment.config.unit_splash_damage(game, self, unit_pos, heroes, temporary_ballast, is_counter)
    }

    pub fn on_start_turn(&self, game: &impl GameView<D>, position: Point, transporter: Option<(&Self, usize)>, heroes: &[HeroInfluence<D>]) -> Vec<usize> {
        self.environment.config.unit_start_turn_effects(
            game,
            self,
            (position, transporter.as_ref().map(|(_, i)| *i)),
            transporter.map(|(u, _)| (u, position)),
            heroes,
        )
    }

    pub fn on_end_turn(&self, game: &impl GameView<D>, position: Point, transporter: Option<(&Self, usize)>, heroes: &[HeroInfluence<D>]) -> Vec<usize> {
        self.environment.config.unit_end_turn_effects(
            game,
            self,
            (position, transporter.as_ref().map(|(_, i)| *i)),
            transporter.map(|(u, _)| (u, position)),
            heroes,
        )
    }

    pub fn on_attack(&self, game: &impl GameView<D>, position: Point, defender: &Self, defender_pos: Point, transporter: Option<(&Unit<D>, Point)>, heroes: &[HeroInfluence<D>], temporary_ballast: &[TBallast<D>], is_counter: bool) -> Vec<usize> {
        self.environment.config.unit_attack_effects(
            game,
            self,
            position,
            defender,
            defender_pos,
            transporter,
            heroes,
            temporary_ballast,
            is_counter,
        )
    }

    pub fn on_defend(&self, game: &impl GameView<D>, position: Point, attacker: &Self, attacker_pos: Point, transporter: Option<(&Unit<D>, Point)>, heroes: &[HeroInfluence<D>], temporary_ballast: &[TBallast<D>], is_counter: bool) -> Vec<usize> {
        self.environment.config.unit_defend_effects(
            game,
            self,
            position,
            attacker,
            attacker_pos,
            transporter,
            heroes,
            temporary_ballast,
            is_counter,
        )
    }

    pub fn on_kill(&self, game: &impl GameView<D>, position: Point, defender: &Self, defender_pos: Point, transporter: Option<(&Unit<D>, Point)>, heroes: &[HeroInfluence<D>], temporary_ballast: &[TBallast<D>], is_counter: bool) -> Vec<usize> {
        self.environment.config.unit_kill_effects(
            game,
            self,
            position,
            defender,
            defender_pos,
            transporter,
            heroes,
            temporary_ballast,
            is_counter,
        )
    }

    pub fn on_death(&self, game: &impl GameView<D>, position: Point, transporter: Option<(&Self, usize)>, attacker: Option<(&Self, Point)>, heroes: &[HeroInfluence<D>], temporary_ballast: &[TBallast<D>]) -> Vec<usize> {
        self.environment.config.unit_death_effects(
            game,
            self,
            (position, transporter.as_ref().map(|(_, i)| *i)),
            transporter.map(|(u, _)| (u, position)),
            attacker,
            heroes,
            temporary_ballast,
        )
    }

    // methods that go beyond getter / setter functionality

    pub fn zip(&self, zipper: &mut Zipper, transported: bool) {
        self.typ.export(zipper, &self.environment);
        /*let owner = transporter.map(|t| t.1).unwrap_or(self.get_owner_id());
        for key in self.environment.unit_attributes(self.typ, owner) {
            if transporter.is_some() && Attribute::<D>::build_from_transporter(*key).is_some() {
                continue;
            }
            let value = key.default();
            let value = self.attributes.get(key).unwrap_or(&value);
            value.export(&self.environment, zipper, self.typ, transporter.is_some(), owner, self.get_hero().typ());
        }*/
        self.owner.export(zipper, &*self.environment.config);
        let sub_movement_types = self.environment.config.sub_movement_types(self.base_movement_type());
        if sub_movement_types.len() > 1 {
            let bits = bits_needed_for_max_value(sub_movement_types.len() as u32 - 1);
            zipper.write_u32(sub_movement_types.iter().position(|mt| *mt == self.sub_movement_type).unwrap() as u32, bits);
        }
        self.hero.export(zipper, &self.environment);
        self.tags.export(zipper, &self.environment);
        if !transported && self.transport_capacity() > 0 {
            zipper.write_u32(self.transport_capacity() as u32, bits_needed_for_max_value(self.transport_capacity() as u32));
            for unit in &self.transport {
                unit.zip(zipper, true);
            }
        }
    }

    pub fn unzip(unzipper: &mut Unzipper, environment: &Environment, transported: bool) -> Result<Self, ZipperError> {
        let typ = UnitType::import(unzipper, environment)?;
        let owner = Owner::import(unzipper, &*environment.config)?;
        let sub_movement_types = environment.config.sub_movement_types(environment.config.base_movement_type(typ));
        let mut sub_movement_type = sub_movement_types[0];
        if sub_movement_types.len() > 1 {
            let bits = bits_needed_for_max_value(sub_movement_types.len() as u32 - 1);
            sub_movement_type = sub_movement_types[unzipper.read_u32(bits)? as usize];
        }
        let hero = Option::<Hero>::import(unzipper, environment)?;
        /*let mut attributes = HashMap::default();
        for key in environment.config.unit_specific_attributes(typ) {
            if transporter.is_some() && Attribute::<D>::build_from_transporter(*key).is_some() {
                continue;
            }
            let attr = Attribute::import(unzipper, environment, *key, typ, transporter.is_some(), owner, hero)?;
            match &attr {
                Attribute::Owner(o) => owner = *o,
                Attribute::Hero(h) => hero = h.typ(),
                _ => (),
            }
            attributes.insert(*key, attr);
        }
        for key in environment.config.commander_attributes(environment.get_commander(owner), typ) {
            if transporter.is_some() && Attribute::<D>::build_from_transporter(*key).is_some() {
                continue;
            }
            let attr = Attribute::import(unzipper, environment, *key, typ, transporter.is_some(), owner, hero)?;
            attributes.insert(*key, attr);
        }
        if let Some(Attribute::Transported(mut units)) = attributes.remove(&AttributeKey::Transported) {
            for unit in &mut units {
                for key in environment.unit_attributes(unit.typ, owner) {
                    if let Some(f) = Attribute::<D>::build_from_transporter(*key) {
                        let value = f(&attributes).expect(&format!("missing value for {key} in transporter"));
                        unit.attributes.insert(*key, value);
                    }
                }
            }
            attributes.insert(AttributeKey::Transported, Attribute::Transported(units));
        }*/
        let tags = TagBag::import(unzipper, environment)?;
        let mut result = Self {
            environment: environment.clone(),
            typ,
            owner,
            sub_movement_type,
            hero,
            tags,
            transport: Vec::new(),
        };
        if !transported && result.transport_capacity() > 0 {
            let transport_len = unzipper.read_u32(bits_needed_for_max_value(result.transport_capacity() as u32))?;
            let mut transported = result.get_transported_mut();
            for _ in 0..transport_len {
                transported.push(Self::unzip(unzipper, environment, true)?);
            }
        }
        Ok(result)
    }

    pub fn fog_replacement(&self, game: &impl GameView<D>, pos: Point, intensity: FogIntensity) -> Option<Self> {
        // for now, heroes don't affect unit visibility.
        // when they do in the future, the heroes should be given to this method instead of calculating here
        // it could also be necessary to add this unit's hero to the heroes list here manually (if it isn't already in there)
        let visibility = self.environment.config.unit_visibility(game, self, pos, &[]);
        match intensity {
            FogIntensity::TrueSight => return Some(self.clone()),
            FogIntensity::NormalVision => {
                if visibility == UnitVisibility::Stealth {
                    return None
                } else {
                    return Some(self.clone());
                }
            }
            FogIntensity::Light => {
                match visibility {
                    UnitVisibility::Stealth => return None,
                    UnitVisibility::Normal => {
                        let mut builder = self.environment.config.unknown_unit()
                            .instance(&self.environment)
                            .set_tag_bag(self.tags.fog_replacement(&self.environment, UnitVisibility::AlwaysVisible));
                        if let Some(hero) = self.hero.as_ref()
                        .filter(|hero| self.environment.hero_visibility(game, &self, pos, hero.typ()) >= UnitVisibility::AlwaysVisible) {
                            builder = builder.set_hero(hero.clone());
                        }
                        return Some(builder.build_with_defaults())
                    }
                    UnitVisibility::AlwaysVisible => (),
                }
            }
            FogIntensity::Dark => {
                // normal units don't have AlwaysVisible so far, but doesn't hurt
                if visibility != UnitVisibility::AlwaysVisible {
                    return None
                }
            }
        }
        // unit is visible, hide some attributes maybe
        let mut builder = self.typ.instance(&self.environment)
            .set_owner_id(self.owner.0)
            .set_movement_type(self.sub_movement_type)
            .set_tag_bag(self.tags.fog_replacement(&self.environment, UnitVisibility::Normal));
        if let Some(hero) = self.hero.as_ref()
        .filter(|hero| self.environment.hero_visibility(game, &self, pos, hero.typ()) >= UnitVisibility::Normal) {
            builder = builder.set_hero(hero.clone());
        }
        let transport_visibility = self.environment.unit_transport_visibility(game, self, pos, &[]);
        if is_unit_attribute_visible(intensity, visibility, transport_visibility) {
            let transport = Vec::new();
            builder = builder.set_transported(transport);
        }
        /*let hero = self.get_hero();
        let hidden_attributes = self.environment.unit_attributes_hidden_by_fog(self.typ, &hero, self.get_owner_id());
        for (k, v) in &self.attributes {
            if !hidden_attributes.contains(k) {
                builder.unit.attributes.insert(*k, v.clone());
            }
        }*/
        Some(builder.build_with_defaults())
    }

    pub fn movable_positions(&self, game: &impl GameView<D>, path_so_far: &Path<D>, transporter: Option<(&Unit<D>, usize)>) -> HashSet<Point> {
        movement_area_game(game, self, path_so_far, 1, transporter)
        .keys()
        .cloned()
        .collect()
    }

    pub fn attackable_positions(&self, game: &impl GameView<D>, path: &Path<D>, transporter: Option<(&Unit<D>, usize)>, ballast: &[TBallast<D>]) -> HashSet<Point> {
        let mut result = HashSet::new();
        let mut game = UnitMovementView::new(game);
        if let Some((destination, this)) = game.unit_path_without_placing(transporter.map(|(_, i)| i), path) {
            if (this.can_attack_after_moving() || path.steps.len() == 0) && game.get_unit(destination).is_none() {
                game.put_unit(destination, this.clone());
                let heroes = Hero::hero_influence_at(&game, destination, self.get_owner_id());
                for attack_vector in AttackVector::search(&this, &game, destination, None, transporter.map(|(u, _)| (u, path.start)), ballast, Counter::NoCounter) {
                    for (point, _, _) in attack_vector.get_splash(&this, &game, destination, &heroes, ballast, Counter::NoCounter) {
                        result.insert(point);
                    }
                }
            }
        }
        result
    }

    pub fn shortest_path_to(&self, game: &impl GameView<D>, path_so_far: &Path<D>, transporter: Option<(&Unit<D>, usize)>, goal: Point) -> Option<(Path<D>, TemporaryBallast<D>)> {
        search_path(game, self, path_so_far, transporter, |_path, p, can_stop_here, _| {
            if goal == p {
                PathSearchFeedback::Found
            } else if can_stop_here {
                PathSearchFeedback::Continue
            } else {
                PathSearchFeedback::ContinueWithoutStopping
            }
        })
    }

    pub fn shortest_path_to_attack(&self, game: &impl GameView<D>, path_so_far: &Path<D>, transporter: Option<(&Unit<D>, usize)>, goal: Point) -> Option<(Path<D>, TemporaryBallast<D>)> {
        /*if AttackType::None == self.attack_pattern() {
            return None;
        }*/
        search_path(game, self, path_so_far, transporter, |path, p, can_stop_here, ballast| {
            let mut takes = PathStepTakes::Allow;
            for ballast in ballast.get_entries() {
                match ballast {
                    TBallast::Takes(t) => takes = *t,
                    _ => (),
                }
            }
            if !can_stop_here {
                return PathSearchFeedback::ContinueWithoutStopping
            } else if goal == p && can_stop_here && takes != PathStepTakes::Deny {
                return PathSearchFeedback::Found
            } else if path.steps.len() == 0 || self.can_attack_after_moving() {
                let mut game = UnitMovementView::new(game);
                if let Some((destination, this)) = game.unit_path_without_placing(transporter.map(|(_, i)| i), path) {
                    if (this.can_attack_after_moving() || path.steps.len() == 0) && game.get_unit(destination).is_none() {
                        game.put_unit(destination, this.clone());
                        if AttackVector::search(&this, &game, destination, Some(goal), transporter.map(|(u, _)| (u, path.start)), ballast.get_entries(), Counter::NoCounter).len() > 0 {
                            return PathSearchFeedback::Found
                        }
                    }
                }
            }
            PathSearchFeedback::Continue
        })
    }

    pub fn transformed_by_movement(&mut self, map: &impl GameView<D>, _from: Point, to: Point, distortion: Distortion<D>) -> bool {
        let terrain = map.get_terrain(to).unwrap();
        let permanent = PermanentBallast::from_unit(self);
        let changed = permanent.step(distortion, &terrain);
        changed.update_unit(self);
        changed != permanent
    }

    pub fn transformed_by_path(&mut self, map: &impl GameView<D>, path: &Path<D>) {
        let mut current = path.start;
        for step in &path.steps {
            let (next, distortion) = match step.progress(map, current) {
                Ok(n) => n,
                _ => return,
            };
            self.transformed_by_movement(map, current, next, distortion);
            current = next;
        }
    }

    pub fn could_attack(&self, defender: &Self, allow_friendly_fire: bool) -> bool {
        let base_damage = self.base_damage(defender.typ());
        if base_damage.is_none() {
            return false;
        }
        if self.displacement() == Displacement::InsteadOfAttack && !defender.can_be_displaced() {
            return false;
        }
        if self.displacement() == Displacement::None && base_damage == Some(0) {
            return false;
        }
        if !allow_friendly_fire && !match self.attack_targeting() {
            AttackTargeting::All => true,
            AttackTargeting::Enemy => self.get_team() != defender.get_team(),
            AttackTargeting::Friendly => self.get_team() == defender.get_team(),
            AttackTargeting::Owned => self.get_owner_id() == defender.get_owner_id(),
            /*AttackTargeting::OwnedBothUnmoved => {
                self.get_owner_id() == defender.get_owner_id()
                && self.get_unmoved() && defender.get_unmoved()
            }*/
        } {
            return false;
        }
        true
    }

    pub fn could_take(&self, defender: &Self, takes: PathStepTakes) -> bool {
        takes != PathStepTakes::Deny && defender.can_be_taken() && self.get_team() != defender.get_team()
    }

    pub fn threatens(&self, defender: &Self) -> bool {
        //let terrain = game.get_map().get_terrain(target_pos).unwrap();
        //let in_water = terrain.is_water();
        self.could_attack(defender, false) && defender.get_team() != self.get_team()
    }

    pub fn options_after_path(&self, game: &impl GameView<D>, path: &Path<D>, transporter: Option<(&Unit<D>, usize)>, ballast: &[TBallast<D>]) -> Vec<UnitAction<D>> {
        let mut game = UnitMovementView::new(game);
        if let Some((end, this)) = game.unit_path_without_placing(transporter.map(|(_, i)| i), path) {
            this._options_after_path_transformed(&game, path, end, transporter, ballast)
        } else {
            Vec::new()
        }
    }

    fn _options_after_path_transformed(&self, game: &impl GameView<D>, path: &Path<D>, destination: Point, transporter: Option<(&Unit<D>, usize)>, ballast: &[TBallast<D>]) -> Vec<UnitAction<D>> {
        let mut result = Vec::new();
        let funds_after_path = game.get_owning_player(self.get_owner_id()).unwrap().funds_after_path(game, path);
        // terrain has to exist since destination point was found from path
        let terrain = game.get_terrain(destination).unwrap();
        let team = self.get_team();
        let blocking_unit = game.get_visible_unit(team, destination);
        let mut takes = PathStepTakes::Allow;
        for ballast in ballast {
            match ballast {
                TBallast::Takes(t) => takes = *t,
                _ => (),
            }
        }
        if path.start != destination && blocking_unit.is_some() {
            if self.could_take(blocking_unit.as_ref().unwrap(), takes) {
                result.push(UnitAction::Take);
            }
            if let Some(transporter) = game.get_visible_unit(team, destination) {
                if transporter.can_transport(self) {
                    result.push(UnitAction::Enter);
                }
            }
        } else if blocking_unit.is_none() {
            let mut game = UnitMovementView::new(game);
            game.put_unit(destination, self.clone());
            let game = &game;
            let heroes = Hero::hero_influence_at(game, destination, self.get_owner_id());
            // hero power
            if let Some(hero) = &self.hero {
                hero.add_options_after_path(&mut result, game);
            }
            // buy hero
            /*if !self.is_hero() && terrain.can_sell_hero(game, destination, self.get_owner_id()) {
                for hero in game.available_heroes(&self.get_player(game).unwrap()) {
                    if let Some(cost) = game.environment().config.hero_price_after_moving(game, hero, &path, destination, self.clone(), transporter.map(|(_, i)| i)) {
                        if cost <= funds_after_path {
                            result.push(UnitAction::BuyHero(hero));
                        }
                    }
                }
            }*/
            // custom actions
            let custom_actions = self.environment.config.custom_actions();
            if custom_actions.len() > 0 {
                let engine = game.environment().get_engine(game);
                // build scope
                let mut scope = Scope::new();
                scope.push_constant(CONST_NAME_UNIT, self.clone());
                scope.push_constant(CONST_NAME_PATH, path.clone());
                scope.push_constant(CONST_NAME_POSITION, destination);
                scope.push_constant(CONST_NAME_TRANSPORT_INDEX, transporter.map(|(_, i)| i));
                scope.push_constant(CONST_NAME_TRANSPORTER, transporter.map(|(t, _)| t.clone()));
                scope.push_constant(CONST_NAME_TRANSPORTER_POSITION, path.start);
                // TODO: heroes and ballast (put them into Arc<>s)
                let executor = Arc::new(Executor::new(engine, scope, game.environment()));
                for (i, custom_action) in custom_actions.iter().enumerate() {
                    if custom_action.add_as_option(game, self, path, destination, funds_after_path, transporter, None, &heroes, ballast, &executor) {
                        result.push(UnitAction::custom(i, Vec::new()));
                    }
                }
            }
            // attack
            if self.can_attack_after_moving() || path.steps.len() == 0 {
                let transporter = transporter.map(|(u, _)| (u, path.start));
                for attack_vector in AttackVector::find(self, game, destination, None, transporter, ballast, Counter::NoCounter) {
                    result.push(UnitAction::Attack(attack_vector));
                }
            }
            /*if self.can_capture() && terrain.has_attribute(TerrainAttributeKey::Owner) && terrain.get_team() != self.get_team() {
                result.push(UnitAction::Capture);
            }*/
            /*if self.can_have_status(ActionStatus::Exhausted) {
                let mut take_instead_of_wait = false;
                if takes != PathStepTakes::Deny && self.has_attribute(AttributeKey::EnPassant) {
                    for dp in game.all_points() {
                        if let Some(u) = game.get_visible_unit(team, dp) {
                            if self.could_take(&u, takes) && u.get_en_passant() == Some(destination) {
                                take_instead_of_wait = true;
                                break;
                            }
                        }
                    }
                }
                if take_instead_of_wait {
                    result.push(UnitAction::Take);
                } else {
                    result.push(UnitAction::Wait);
                }
            }*/
            result.push(UnitAction::Wait);
        }
        println!("unit actions: {result:?}");
        result
    }
}

impl<D: Direction> SupportedZippable<&Environment> for Unit<D> {
    fn export(&self, zipper: &mut Zipper, _support: &Environment) {
        self.zip(zipper, false);
    }
    fn import(unzipper: &mut Unzipper, support: &Environment) -> Result<Self, ZipperError> {
        Self::unzip(unzipper, support, false)
    }
}

impl<D: Direction> Hash for Unit<D> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        let mut zipper = Zipper::new();
        self.export(&mut zipper, &self.environment);
        zipper.finish().hash(state);
    }
}

pub struct TransportedRef<'a, D: Direction> {
    unit: &'a mut Unit<D>,
}

impl<'a, D: Direction> Drop for TransportedRef<'a, D> {
    fn drop(&mut self) {
        self.unit.fix_transported()
    }
}

impl<'a, D: Direction> Deref for TransportedRef<'a, D> {
    type Target = Vec<Unit<D>>;
    fn deref(&self) -> &Self::Target {
        &self.unit.transport
    }
}

impl<'a, D: Direction> DerefMut for TransportedRef<'a, D> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.unit.transport
    }
}

#[derive(Clone)]
pub struct UnitBuilder<D: Direction> {
    unit: Unit<D>,
}

impl<D: Direction> UnitBuilder<D> {
    pub fn new(environment: &Environment, typ: UnitType) -> Self {
        let unit = Unit::new(environment.clone(), typ);
        Self {
            unit,
        }
    }

    pub fn environment(&self) -> &Environment {
        &self.unit.environment
    }

    pub fn copy_from(mut self, other: &Unit<D>) -> Self {
        if self.unit.environment != other.environment {
            panic!("Can't copy from unit from different environment");
        }
        for key in other.tags.flags() {
            self.unit.set_flag(*key);
        }
        for (key, value) in other.tags.tags() {
            self.unit.set_tag(*key, value.clone());
        }
        self
    }

    pub fn set_tag_bag(mut self, bag: TagBag<D>) -> Self {
        self.unit.tags = bag;
        self
    }

    pub fn set_flag(&mut self, key: usize) {
        self.unit.set_flag(key);
    }
    pub fn remove_flag(&mut self, key: usize) {
        self.unit.remove_flag(key);
    }

    pub fn set_tag(&mut self, key: usize, value: TagValue<D>) {
        self.unit.set_tag(key, value);
    }
    pub fn remove_tag(&mut self, key: usize) {
        self.unit.remove_tag(key);
    }

    pub fn set_owner_id(mut self, owner_id: i8) -> Self {
        let owner_id = owner_id.min((self.unit.environment.config.max_player_count() - 1) as i8);
        self.unit.set_owner_id(owner_id);
        self
    }

    pub fn set_movement_type(mut self, sub_movement_type: MovementType) -> Self {
        self.unit.set_sub_movement_type(sub_movement_type);
        self
    }

    /*pub fn set_direction(mut self, direction: D) -> Self {
        self.unit.set_direction(direction);
        self
    }

    pub fn set_drone_station_id(mut self, id: u16) -> Self {
        self.unit.set_drone_station_id(id);
        self
    }

    pub fn set_drone_id(mut self, id: u16) -> Self {
        self.unit.set_drone_id(id);
        self
    }

    pub fn set_hp(mut self, hp: u8) -> Self {
        self.unit.set_hp(hp);
        self
    }*/

    pub fn set_hero(mut self, hero: Hero) -> Self {
        self.unit.set_hero(hero);
        self
    }

    /*pub fn set_status(mut self, status: ActionStatus) -> Self {
        self.unit.set_status(status);
        self
    }

    pub fn set_amphibious(mut self, amphibious: Amphibious) -> Self {
        self.unit.set_amphibious(amphibious);
        self
    }

    pub fn set_zombified(mut self, zombified: bool) -> Self {
        self.unit.set_zombified(zombified);
        self
    }*/

    pub fn set_transported(mut self, transported: Vec<Unit<D>>) -> Self {
        *self.unit.get_transported_mut() = transported;
        self
    }

    pub fn build(&self) -> Unit<D> {
        self.unit.clone()
    }

    /**
     * TODO: call rhai script to get default flags/tag values?
     */
    pub fn build_with_defaults(&self) -> Unit<D> {
        self.unit.clone()
    }
}

