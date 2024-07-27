use std::collections::{HashSet, HashMap};
use std::fmt::Debug;
use std::hash::Hash;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;

use interfaces::game_interface::ClientPerspective;
use num_rational::Rational32;
use zipper::*;

use crate::commander::commander_type::CommanderType;
use crate::config::environment::Environment;
use crate::config::movement_type_config::MovementPattern;
use crate::game::fog::{FogIntensity, VisionMode, FogSetting};
use crate::game::game::Game;
use crate::game::game_view::GameView;
use crate::game::modified_view::*;
use crate::game::settings::GameSettings;
use crate::commander::Commander;
use crate::map::direction::Direction;
use crate::map::map_view::MapView;
use crate::map::point::Point;
use crate::map::wrapping_map::Distortion;
use crate::player::{Player, Owner};
use crate::script::attack::AttackScript;
use crate::script::death::DeathScript;
use crate::script::defend::DefendScript;
use crate::script::kill::KillScript;
use crate::script::unit::UnitScript;
use crate::terrain::attributes::TerrainAttributeKey;

use super::combat::*;
use super::commands::UnitAction;
use super::movement::*;
use super::unit_types::UnitType;
use super::attributes::*;
use super::hero::*;


#[derive(Clone, Eq)]
pub struct Unit<D: Direction> {
    environment: Environment,
    typ: UnitType,
    attributes: HashMap<AttributeKey, Attribute<D>>,
}

impl<D: Direction> PartialEq for Unit<D> {
    fn eq(&self, other: &Self) -> bool {
        self.typ == other.typ
        && self.attributes == other.attributes
    }
}

impl<D: Direction> Debug for Unit<D> {
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

impl<D: Direction> Unit<D> {
    pub(super) fn new(environment: Environment, typ: UnitType) -> Self {
        Self {
            environment,
            typ,
            attributes: HashMap::default(),
        }
    }

    pub(crate) fn start_game(&mut self, settings: &Arc<GameSettings>) {
        self.environment.start_game(settings);
        if let Some(mut transported) = self.get_transported_mut() {
            for unit in transported.deref_mut() {
                unit.start_game(settings);
            }
        }
        for key in self.environment.unit_attributes(self.typ, self.get_owner_id()) {
            if !self.attributes.contains_key(key) {
                self.attributes.insert(*key, key.default());
            }
        }
    }

    // getters that aren't influenced by attributes
    pub(crate) fn environment(&self) -> &Environment {
        &self.environment
    }

    pub(crate) fn get_attributes(&self) -> &HashMap<AttributeKey, Attribute<D>> {
        &self.attributes
    }

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
        self.environment.unit_transport_capacity(self.typ, self.get_owner_id(), self.get_hero().typ())
    }

    pub fn movement_pattern(&self) -> MovementPattern {
        self.environment.config.movement_pattern(self.typ)
    }

    pub fn movement_type(&self, amphibious: Amphibious) -> MovementType {
        self.environment.config.movement_type(self.typ, amphibious)
    }

    pub fn default_movement_type(&self) -> MovementType {
        self.environment.config.movement_type(self.typ, self.get_amphibious())
    }

    pub fn base_movement_points(&self) -> Rational32 {
        self.environment.config.base_movement_points(self.typ)
    }

    pub fn is_amphibious(&self) -> bool {
        self.attributes.contains_key(&AttributeKey::Amphibious)
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

    pub fn can_have_status(&self, status: ActionStatus) -> bool {
        self.has_attribute(AttributeKey::ActionStatus)
        && self.environment.unit_valid_action_status(self.typ, self.get_owner_id()).contains(&status)
    }

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

    pub fn vision_range(&self, game: &Game<D>, pos: Point, heroes: &[HeroInfluence<D>]) -> usize {
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

    fn true_vision_range(&self, game: &Game<D>, pos: Point, heroes: &[HeroInfluence<D>]) -> usize {
        self.environment.config.unit_true_vision(game, self, pos, heroes)
    }

    pub fn get_vision(&self, game: &Game<D>, pos: Point, heroes: &[HeroInfluence<D>]) -> HashMap<Point, FogIntensity> {
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
                let layers = game.get_map().range_in_layers(pos, vision_range);
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

    pub fn has_attribute(&self, key: AttributeKey) -> bool {
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
    }

    pub fn get_owner_id(&self) -> i8 {
        self.get::<Owner>().0
    }
    pub fn set_owner_id(&mut self, id: i8) {
        if id >= 0 || !self.environment.config.unit_needs_owner(self.typ) {
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
        }
    }

    pub fn get_team(&self) -> ClientPerspective {
        self.environment.get_team(self.get_owner_id())
    }

    pub fn get_player<'a>(&self, game: &'a impl GameView<D>) -> Option<&'a Player> {
        game.get_owning_player(self.get_owner_id())
    }

    pub fn get_commander(&self, game: &impl GameView<D>) -> Commander {
        self.get_player(game)
        .and_then(|player| Some(player.commander.clone()))
        .unwrap_or(Commander::new(&self.environment, CommanderType::None))
    }

    // TODO: return Option?
    pub fn get_direction(&self) -> D {
        self.get::<D>()
    }
    pub fn set_direction(&mut self, direction: D) {
        self.set(direction);
    }

    pub fn distort(&mut self, distortion: Distortion<D>) {
        self.set_direction(distortion.update_direction(self.get_direction()));
    }

    pub fn is_hero(&self) -> bool {
        if let Some(Attribute::Hero(hero)) = self.attributes.get(&AttributeKey::Hero) {
            hero.typ() != HeroType::None
        } else {
            false
        }
    }
    pub fn get_hero(&self) -> Hero {
        self.get::<Hero>()
    }
    pub fn get_hero_mut(&mut self) -> Option<&mut Hero> {
        if let Some(Attribute::Hero(hero)) = self.attributes.get_mut(&AttributeKey::Hero) {
            Some(hero)
        } else {
            None
        }
    }
    pub fn set_hero(&mut self, hero: Hero) {
        // TODO: check if hero is compatible with this unit type
        self.set(hero);
        // update attributes influenced by the hero, e.g. Transported capacity
        self.fix_transported()
    }

    pub fn get_max_charge(&self) -> u8 {
        self.get::<Hero>().max_charge(&self.environment)
    }
    pub fn get_charge(&self) -> u8 {
        self.get::<Hero>().get_charge()
    }
    pub fn set_charge(&mut self, charge: u8) {
        if let Some(Attribute::Hero(hero)) = self.attributes.get_mut(&AttributeKey::Hero) {
            hero.set_charge(&self.environment, charge);
        }
    }

    pub fn get_hp(&self) -> u8 {
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
    }

    pub fn can_capture(&self) -> bool {
        self.can_have_status(ActionStatus::Capturing)
    }

    pub fn can_transport(&self, other: &Self) -> bool {
        self.could_transport(other.typ)
        && self.environment == other.environment
        && self.get_transported().len() < self.transport_capacity()
    }
    pub fn remaining_transport_capacity(&self) -> usize {
        self.transport_capacity() - self.get_transported().len()
    }

    pub fn get_transported(&self) -> &[Unit<D>] {
        if let Some(Attribute::Transported(t)) = self.attributes.get(&AttributeKey::Transported) {
            t
        } else {
            &[]
        }
    }
    pub fn get_transported_mut<'a>(&'a mut self) -> Option<TransportedRef<'a, D>> {
        if self.attributes.contains_key(&AttributeKey::Transported) {
            Some(TransportedRef { unit: self })
        } else {
            None
        }
    }
    fn fix_transported(&mut self) {
        let mut transported = match self.attributes.remove(&AttributeKey::Transported) {
            Some(Attribute::Transported(t)) => t,
            _ => return
        };
        // remove units that can't be transported by self, don't go over capacity
        let transportable = self.environment.config.unit_transportable(self.typ);
        transported = transported.into_iter()
        .filter(|other| {
            other.environment == self.environment
            && transportable.contains(&other.typ)
        })
        .take(self.transport_capacity())
        .collect();
        // some attributes are defined by the transporter. make sure this stays consistent
        for unit in &mut transported {
            for key in self.environment.unit_attributes(unit.typ, self.get_owner_id()) {
                if let Some(f) = Attribute::<D>::build_from_transporter(*key) {
                    let value = f(&self.attributes).expect(&format!("missing value for {key} in transporter"));
                    unit.attributes.insert(*key, value);
                } else if !unit.attributes.contains_key(key) {
                    unit.attributes.insert(*key, key.default());
                }
            }
        }
        self.attributes.insert(AttributeKey::Transported, Attribute::Transported(transported));
    }

    pub fn get_amphibious(&self) -> Amphibious {
        self.get()
    }
    pub fn set_amphibious(&mut self, a: Amphibious) {
        self.set(a);
    }

    pub fn get_unmoved(&self) -> bool {
        self.get::<Unmoved>().0
    }
    pub fn set_unmoved(&mut self, unmoved: bool) {
        self.set(Unmoved(unmoved));
    }

    pub fn get_en_passant(&self) -> Option<Point> {
        self.get::<Option<Point>>()
    }
    pub fn set_en_passant(&mut self, en_passant: Option<Point>) {
        self.set(en_passant);
    }

    pub fn get_zombified(&self) -> bool {
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
    }

    pub fn translate(&mut self, translations: [D::T; 2], odd_if_hex: bool) {
        for attribute in self.attributes.values_mut() {
            attribute.translate(translations, odd_if_hex);
        }
    }

    // influenced by unit_power_config

    // ignores current hp
    pub fn full_price(&self, game: &impl GameView<D>, position: Point, factory: Option<&Unit<D>>, heroes: &[HeroInfluence<D>]) -> i32 {
        self.environment.config.unit_cost(
            game,
            self,
            position,
            factory,
            heroes,
        )
    }

    // full_price reduced by hp lost
    pub fn value(&self, game: &Game<D>, position: Point) -> i32 {
        self.full_price(game, position, None, &[]) * self.get_hp() as i32 / 100
    }

    pub fn movement_points(&self, game: &impl GameView<D>, position: Point, transporter: Option<&Unit<D>>, heroes: &[HeroInfluence<D>]) -> Rational32 {
        self.environment.config.unit_movement_points(
            game,
            self,
            (position, None),
            transporter.map(|u| (u, position)),
            heroes,
        )
    }

    pub fn build_overrides(&self, game: &impl GameView<D>, position: Point, transporter: Option<(&Unit<D>, Point)>, heroes: &[HeroInfluence<D>], temporary_ballast: &[TBallast<D>]) -> HashSet<AttributeOverride> {
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
    }

    // returns a list of damage factors
    // the first element is the selected target, the second element for points next to the target and so on
    pub fn get_splash_damage(&self, game: &impl GameView<D>, unit_pos: Point, heroes: &[HeroInfluence<D>], temporary_ballast: &[TBallast<D>], is_counter: bool) -> Vec<Rational32> {
        self.environment.config.unit_splash_damage(game, self, unit_pos, heroes, temporary_ballast, is_counter)
    }

    pub fn on_start_turn(&self, game: &Game<D>, position: Point, transporter: Option<(&Self, usize)>, heroes: &[HeroInfluence<D>]) -> Vec<UnitScript> {
        self.environment.config.unit_start_turn_effects(
            game,
            self,
            (position, transporter.as_ref().map(|(_, i)| *i)),
            transporter.map(|(u, _)| (u, position)),
            heroes,
        )
    }

    pub fn on_end_turn(&self, game: &Game<D>, position: Point, transporter: Option<(&Self, usize)>, heroes: &[HeroInfluence<D>]) -> Vec<UnitScript> {
        self.environment.config.unit_end_turn_effects(
            game,
            self,
            (position, transporter.as_ref().map(|(_, i)| *i)),
            transporter.map(|(u, _)| (u, position)),
            heroes,
        )
    }

    pub fn on_attack(&self, game: &Game<D>, position: Point, defender: &Self, defender_pos: Point, transporter: Option<(&Unit<D>, Point)>, heroes: &[HeroInfluence<D>], temporary_ballast: &[TBallast<D>], is_counter: bool) -> Vec<AttackScript> {
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

    pub fn on_defend(&self, game: &Game<D>, position: Point, attacker: &Self, attacker_pos: Point, transporter: Option<(&Unit<D>, Point)>, heroes: &[HeroInfluence<D>], temporary_ballast: &[TBallast<D>], is_counter: bool) -> Vec<DefendScript> {
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

    pub fn on_kill(&self, game: &Game<D>, position: Point, defender: &Self, defender_pos: Point, transporter: Option<(&Unit<D>, Point)>, heroes: &[HeroInfluence<D>], temporary_ballast: &[TBallast<D>], is_counter: bool) -> Vec<KillScript> {
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

    pub fn on_death(&self, game: &Game<D>, position: Point, transporter: Option<(&Self, usize)>, attacker: Option<(&Self, Point)>, heroes: &[HeroInfluence<D>], temporary_ballast: &[TBallast<D>]) -> Vec<DeathScript> {
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

    pub fn zip(&self, zipper: &mut Zipper, transporter: Option<(UnitType, i8)>) {
        let units = if let Some((transporter, _)) = transporter {
            self.environment.config.unit_transportable(transporter)
        } else {
            self.environment.config.unit_types()
        };
        let bits = bits_needed_for_max_value(units.len() as u32 - 1);
        zipper.write_u32(units.iter().position(|t| *t == self.typ).unwrap_or(0) as u32, bits);
        let owner = transporter.map(|t| t.1).unwrap_or(self.get_owner_id());
        for key in self.environment.unit_attributes(self.typ, owner) {
            if transporter.is_some() && Attribute::<D>::build_from_transporter(*key).is_some() {
                continue;
            }
            let value = key.default();
            let value = self.attributes.get(key).unwrap_or(&value);
            value.export(&self.environment, zipper, self.typ, transporter.is_some(), owner, self.get_hero().typ());
        }
    }

    pub fn unzip(unzipper: &mut Unzipper, environment: &Environment, transporter: Option<(UnitType, i8)>) -> Result<Self, ZipperError> {
        let units = if let Some((transporter, _)) = transporter {
            environment.config.unit_transportable(transporter)
        } else {
            environment.config.unit_types()
        };
        let bits = bits_needed_for_max_value(units.len() as u32 - 1);
        let index = unzipper.read_u32(bits)? as usize;
        if index >= environment.config.unit_count() {
            return Err(ZipperError::InconsistentData);
        }
        let typ = units[index];
        let mut attributes = HashMap::default();
        let mut owner = transporter.map(|t| t.1).unwrap_or(-1);
        let mut hero = HeroType::None;
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
        }
        Ok(Unit {
            environment: environment.clone(),
            typ,
            attributes,
        })
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
                        return Some(UnitType::Unknown.instance(&self.environment).build_with_defaults())
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
        let mut builder = self.typ.instance(&self.environment);
        let hero = self.get_hero();
        let hidden_attributes = self.environment.unit_attributes_hidden_by_fog(self.typ, &hero, self.get_owner_id());
        for (k, v) in &self.attributes {
            if !hidden_attributes.contains(k) {
                builder.unit.attributes.insert(*k, v.clone());
            }
        }
        Some(builder.build_with_defaults())
    }

    pub fn movable_positions(&self, game: &Game<D>, path_so_far: &Path<D>, transporter: Option<(&Unit<D>, usize)>) -> HashSet<Point> {
        movement_area_game(game, self, path_so_far, 1, transporter)
        .keys()
        .cloned()
        .collect()
    }

    pub fn attackable_positions(&self, game: &Game<D>, path: &Path<D>, transporter: Option<(&Unit<D>, usize)>, ballast: &[TBallast<D>]) -> HashSet<Point> {
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

    pub fn shortest_path_to(&self, game: &Game<D>, path_so_far: &Path<D>, transporter: Option<(&Unit<D>, usize)>, goal: Point) -> Option<(Path<D>, TemporaryBallast<D>)> {
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

    pub fn shortest_path_to_attack(&self, game: &Game<D>, path_so_far: &Path<D>, transporter: Option<(&Unit<D>, usize)>, goal: Point) -> Option<(Path<D>, TemporaryBallast<D>)> {
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

    pub fn transformed_by_movement(&mut self, map: &impl MapView<D>, from: Point, to: Point, distortion: Distortion<D>) -> bool {
        let prev_terrain = map.get_terrain(from).unwrap();
        let terrain = map.get_terrain(to).unwrap();
        let permanent = PermanentBallast::from_unit(self, prev_terrain);
        let changed = permanent.step(distortion, terrain);
        changed.update_unit(self);
        changed != permanent
    }

    pub fn transformed_by_path(&mut self, map: &impl MapView<D>, path: &Path<D>) {
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
            AttackTargeting::OwnedBothUnmoved => {
                self.get_owner_id() == defender.get_owner_id()
                && self.get_unmoved() && defender.get_unmoved()
            }
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

    pub fn options_after_path(&self, game: &Game<D>, path: &Path<D>, transporter: Option<(&Unit<D>, usize)>, ballast: &[TBallast<D>]) -> Vec<UnitAction<D>> {
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
            self.get_hero().add_options_after_path(&mut result, self, game, funds_after_path, path, destination, transporter, &heroes, ballast);
            // buy hero
            if !self.is_hero() && terrain.can_sell_hero(game, destination, self.get_owner_id()) {
                for hero in game.available_heroes(self.get_player(game).unwrap()) {
                    if let Some(cost) = hero.price(game.environment(), self) {
                        if cost <= funds_after_path {
                            result.push(UnitAction::BuyHero(hero));
                        }
                    }
                }
            }
            // custom actions
            for (i, custom_action) in self.environment.config.custom_actions().iter().enumerate() {
                if custom_action.add_as_option(game, self, path, destination, funds_after_path, transporter, None, &heroes, ballast) {
                    result.push(UnitAction::Custom(i, Vec::new()));
                }
            }
            // build units
            /*if self.can_build_units() && self.transport_capacity() > 0 {
                let mut free_space = self.remaining_transport_capacity();
                if let Some(drone_id) = self.get_drone_station_id() {
                    let mut outside = 0;
                    for p in game.all_points() {
                        if let Some(u) = game.get_visible_unit(team, p) {
                            if u.get_drone_id() == Some(drone_id) {
                                outside += 1;
                            }
                        }
                    }
                    free_space = free_space.max(outside) - outside;
                }
                if free_space > 0 {
                    let transporter = transporter.map(|(u, _)| (u, path.start));
                    for (unit, cost) in self.unit_shop(game, destination, transporter, ballast) {
                        if cost <= funds_after_path {
                            result.push(UnitAction::BuyTransportedUnit(unit.typ()));
                        }
                    }
                }
            } else if self.can_build_units() && self.transport_capacity() == 0 {
                let transporter = transporter.map(|(u, _)| (u, path.start));
                for (unit, cost) in self.unit_shop(game, destination, transporter, ballast) {
                    if cost <= funds_after_path {
                        for d in D::list() {
                            if game.get_neighbor(destination, d)
                            .and_then(|(p, _)| game.get_terrain(p).unwrap().movement_cost(unit.default_movement_type()))
                            .is_some() {
                                result.push(UnitAction::BuyUnit(unit.typ(), d));
                            }
                        }
                    }
                }
            }*/
            // attack
            if self.can_attack_after_moving() || path.steps.len() == 0 {
                let transporter = transporter.map(|(u, _)| (u, path.start));
                for attack_vector in AttackVector::find(self, game, destination, None, transporter, ballast, Counter::NoCounter) {
                    result.push(UnitAction::Attack(attack_vector));
                }
            }
            if self.can_capture() && terrain.has_attribute(TerrainAttributeKey::Owner) && terrain.get_team() != self.get_team() {
                result.push(UnitAction::Capture);
            }
            /*if self.get_hp() < 100 && terrain.can_repair() && terrain.can_repair_unit(self.typ)
            && (!terrain.has_attribute(TerrainAttributeKey::Owner) || terrain.get_owner_id() == self.get_owner_id())
            && funds_after_path * 100 >= self.full_price(game, destination, None, heroes.as_slice()) {
                result.push(UnitAction::Repair);
            }*/
            if self.can_have_status(ActionStatus::Exhausted) {
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
            }
        }
        println!("unit actions: {result:?}");
        result
    }

    pub(crate) fn unit_shop_option(&self, game: &impl GameView<D>, pos: Point, unit_type: UnitType, transporter: Option<(&Unit<D>, Point)>, heroes: &[HeroInfluence<D>], ballast: &[TBallast<D>]) -> (Unit<D>, i32) {
        let attr_overrides = self.build_overrides(game, pos, transporter, heroes, ballast);
        let mut builder: UnitBuilder<D> = unit_type.instance(&self.environment)
        .set_status(ActionStatus::Exhausted);
        for attr in &attr_overrides {
            builder = builder.set_attribute(&attr.into());
        }
        if let Some(drone_id) = self.get_drone_station_id() {
            // TODO: only a drone-station should be able to build drones
            builder = builder.set_drone_id(drone_id);
        }
        let mut unit = builder
        .set_owner_id(self.get_owner_id())
        .build_with_defaults();
        if self.has_attribute(AttributeKey::Direction) {
            unit.set_direction(unit.get_direction().rotate_by(self.get_direction()));
        }
        let heroes = Hero::hero_influence_at(game, pos, self.get_owner_id());
        let cost = unit.full_price(game, pos, Some(self), &heroes);
        (unit, cost)
    }

    pub fn unit_shop(&self, game: &impl GameView<D>, pos: Point, transporter: Option<(&Unit<D>, Point)>, ballast: &[TBallast<D>]) -> Vec<(Unit<D>, i32)> {
        let heroes = Hero::hero_influence_at(game, pos, self.get_owner_id());
        self.transportable_units().iter().map(|unit_type| {
            self.unit_shop_option(game, pos, *unit_type, transporter, &heroes, ballast)
        }).collect()
    }
}

impl<D: Direction> SupportedZippable<&Environment> for Unit<D> {
    fn export(&self, zipper: &mut Zipper, _support: &Environment) {
        self.zip(zipper, None);
    }
    fn import(unzipper: &mut Unzipper, support: &Environment) -> Result<Self, ZipperError> {
        Self::unzip(unzipper, support, None)
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
        match self.unit.attributes.get(&AttributeKey::Transported) {
            Some(Attribute::Transported(t)) => t,
            _ => panic!("TransportedRef was unable to find attribute in {:?}", self.unit),
        }
    }
}

impl<'a, D: Direction> DerefMut for TransportedRef<'a, D> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self.unit.attributes.get_mut(&AttributeKey::Transported) {
            Some(Attribute::Transported(t)) => return t,
            _ => ()
        };
        panic!("TransportedRef was unable to find mutable attribute");
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

    pub fn copy_from(mut self, other: &Unit<D>) -> Self {
        if self.unit.environment != other.environment {
            panic!("Can't copy from unit from different environment");
        }
        for (key, value) in &other.attributes {
            if self.unit.has_attribute(*key) {
                self.unit.attributes.insert(*key, value.clone());
            }
        }
        self
    }

    pub fn set_attribute(mut self, attribute: &Attribute<D>) -> Self {
        let key = attribute.key();
        if self.unit.has_attribute(key) {
            self.unit.attributes.insert(key, attribute.clone());
        }
        self
    }

    pub fn set_owner_id(mut self, id: i8) -> Self {
        self.unit.set_owner_id(id);
        self
    }

    pub fn set_direction(mut self, direction: D) -> Self {
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
    }

    pub fn set_hero(mut self, hero: Hero) -> Self {
        self.unit.set_hero(hero);
        self
    }

    pub fn set_status(mut self, status: ActionStatus) -> Self {
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
    }

    pub fn set_transported(mut self, transported: Vec<Unit<D>>) -> Self {
        if self.unit.has_attribute(AttributeKey::Transported) {
            self.unit.attributes.insert(AttributeKey::Transported, Attribute::Transported(transported));
            self.unit.fix_transported();
        }
        self
    }

    pub fn build(&self) -> Option<Unit<D>> {
        for key in self.unit.environment.unit_attributes(self.unit.typ(), self.unit.get_owner_id()) {
            if !self.unit.attributes.contains_key(key) {
                return None;
            }
        }
        Some(self.unit.clone())
    }

    /**
     * Take Care! The following attributes don't have reasonable defaults:
     *  - drone_id
     *  - drone_station_id
     *  - owner_id
     *  - direction
     * TODO: when parsing a config, make sure commanders don't have these attributes
     */
    pub fn build_with_defaults(&self) -> Unit<D> {
        let mut unit = self.unit.clone();
        for key in self.unit.environment.unit_attributes(self.unit.typ(), self.unit.get_owner_id()) {
            if !unit.attributes.contains_key(key) {
                /*if *key == AttributeKey::DroneId || *key == AttributeKey::DroneStationId || *key == AttributeKey::Owner {
                    println!("WARNING: building unit with missing Attribute {key}");
                    //return Err(AttributeError { requested: *key, received: None });
                }*/
                unit.attributes.insert(*key, key.default());
            }
        }
        unit
    }
}

