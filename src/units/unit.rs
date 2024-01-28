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
use crate::details::Detail;
use crate::game::event_handler::EventHandler;
use crate::game::fog::{FogIntensity, VisionMode, FogSetting};
use crate::game::game::Game;
use crate::game::settings::GameSettings;
use crate::commander::Commander;
use crate::map::direction::Direction;
use crate::map::map::Map;
use crate::map::point::Point;
use crate::map::wrapping_map::Distortion;
use crate::player::{Player, Owner};
use crate::script::attack::AttackScript;
use crate::script::kill::KillScript;
use crate::terrain::AmphibiousTyping;
use crate::terrain::attributes::TerrainAttributeKey;
use crate::terrain::terrain::Terrain;

use super::combat::*;
use super::commands::UnitAction;
use super::movement::*;
use super::unit_types::UnitType;
use super::attributes::*;
use super::hero::*;


#[derive(Clone, PartialEq, Eq)]
pub struct Unit<D: Direction> {
    environment: Environment,
    typ: UnitType,
    attributes: HashMap<AttributeKey, Attribute<D>>,
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
                self.attributes.insert(*key, key.default(&self.environment));
            }
        }
    }

    // getters that aren't influenced by attributes

    pub fn typ(&self) -> UnitType {
        self.typ
    }

    pub fn name(&self) -> &str {
        self.environment.config.unit_name(self.typ)
    }

    pub fn value(&self) -> i32 {
        self.typ.price(&self.environment, self.get_owner_id()) * self.get_hp() as i32 / 100
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

    pub fn movement_points(&self) -> Rational32 {
        self.environment.config.movement_points(self.typ)
    }

    pub fn is_amphibious(&self) -> bool {
        self.attributes.contains_key(&AttributeKey::Amphibious)
    }

    pub fn has_stealth(&self) -> bool {
        self.environment.config.has_stealth(self.typ)
    }

    pub fn has_stealth_movement(&self, game: &Game<D>) -> bool {
        self.has_stealth() && !game.is_foggy()
    }

    pub fn can_be_moved_through(&self) -> bool {
        self.environment.config.can_be_moved_through(self.typ)
    }

    pub fn can_take(&self) -> bool {
        self.environment.config.can_take(self.typ)
    }

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

    pub fn attack_pattern(&self) -> AttackType {
        self.environment.config.attack_pattern(self.typ)
    }

    pub fn attack_targeting(&self) -> AttackTargeting {
        self.environment.config.attack_targeting(self.typ)
    }

    pub fn base_damage(&self, defender: UnitType) -> Option<u16> {
        self.environment.config.base_damage(self.typ, defender)
    }

    pub fn can_build_units(&self) -> bool {
        self.environment.config.can_build_units(self.typ)
    }

    pub fn displacement(&self) -> Displacement {
        self.environment.config.displacement(self.typ)
    }

    pub fn displacement_distance(&self) -> i8 {
        self.environment.config.displacement_distance(self.typ)
    }

    pub fn can_be_displaced(&self) -> bool {
        self.environment.config.can_be_displaced(self.typ)
    }

    pub fn vision_mode(&self) -> VisionMode {
        self.environment.config.vision_mode(self.typ)
    }

    pub fn vision_range(&self, game: &Game<D>, _pos: Point) -> usize {
        let mut range = self.environment.config.vision_range(self.typ);
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

    fn true_vision_range(&self, _game: &Game<D>, _pos: Point) -> usize {
        1
    }

    pub fn get_vision(&self, game: &Game<D>, pos: Point) -> HashMap<Point, FogIntensity> {
        let mut result = HashMap::new();
        result.insert(pos, FogIntensity::TrueSight);
        let vision_range = self.vision_range(game, pos);
        let normal_range = match game.get_fog_setting() {
            FogSetting::ExtraDark(_) => 0,
            FogSetting::Fade1(_) => 1.max(vision_range) - 1,
            FogSetting::Fade2(_) => 2.max(vision_range) - 2,
            _ => vision_range
        };
        let true_vision_range = self.true_vision_range(game, pos);
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
                movement_search_game(game, self, &Path::new(pos), 1,
                    |_| None,
                    |_, path, destination, can_continue, can_stop_here| {
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
            T::try_from(T::key().default(&self.environment)).expect("Impossible! attribute defaults to wrong type")
        }
    }

    fn set<T: TrAttribute<D>>(&mut self, value: T) -> bool {
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
        if id >= 0 || !self.environment.config.unit_needs_owner(self.typ) {
            let owner_before = self.get_owner_id();
            self.set(Owner(id.max(-1).min(self.environment.config.max_player_count() - 1)));
            let co_before = self.environment.config.commander_attributes(self.environment.get_commander(owner_before), self.typ);
            let co_after = self.environment.config.commander_attributes(self.environment.get_commander(self.get_owner_id()), self.typ);
            for key in co_before.iter().filter(|k| !co_after.contains(k)) {
                self.attributes.remove(key);
            }
            for key in co_after.iter().filter(|k| !co_before.contains(k)) {
                self.attributes.insert(*key, key.default(&self.environment));
            }
            self.fix_transported();
        }
    }

    pub fn get_team(&self) -> ClientPerspective {
        self.environment.get_team(self.get_owner_id())
    }

    pub fn get_player<'a>(&self, game: &'a Game<D>) -> Option<&'a Player> {
        game.get_owning_player(self.get_owner_id())
    }

    pub fn get_commander(&self, game: &Game<D>) -> Commander {
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

    // returns a list of damage factors
    // the first element is the selected target, the second element for points next to the target and so on
    pub fn get_splash_damage(&self) -> &[Rational32] {
        self.environment.config.splash_damage(self.typ)
    }

    pub fn get_charge(&self) -> u8 {
        self.get::<Hero>().get_charge()
    }
    pub fn set_charge(&mut self, charge: u8) {
        if let Some(Attribute::Hero(hero)) = self.attributes.get_mut(&AttributeKey::Hero) {
            hero.set_charge(charge);
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

    // "scripts"

    pub fn build_overrides(&self, game: &Game<D>, position: Point) -> HashSet<AttributeOverride> {
        let mut overrides = HashMap::new();
        for ov in self.environment.config.commander_unit_attribute_overrides(&self.get_commander(game), self, game, position) {
            overrides.insert(ov.key(), ov.clone());
        }
        for (p, hero_unit, hero) in game.get_map().hero_influence_at(position, self.get_owner_id()) {
            for ov in self.environment.config.hero_attribute_overrides(game, self, position, &hero_unit, p, &hero) {
                overrides.insert(ov.key(), ov.clone());
            }
        }
        overrides.values()
        .cloned()
        .collect()
    }

    pub fn on_start_turn(&self, handler: &mut EventHandler<D>, position: Point) {
        let game = handler.get_game();
        let mut scripts = self.get_commander(game).unit_start_turn_scripts(self, game, position);
        for (p, hero_unit, hero) in game.get_map().hero_influence_at(position, self.get_owner_id()) {
            for script in self.environment.config.hero_start_turn_scripts(game, self, position, &hero_unit, p, &hero) {
                scripts.push(script.clone());
            }
        }
        for script in scripts {
            script.trigger(handler, position, self)
        }
    }

    pub fn on_end_turn(&self, handler: &mut EventHandler<D>, position: Point) {
        let game = handler.get_game();
        let mut scripts = self.get_commander(game).unit_end_turn_scripts(self, game, position);
        for (p, hero_unit, hero) in game.get_map().hero_influence_at(position, self.get_owner_id()) {
            for script in self.environment.config.hero_end_turn_scripts(game, self, position, &hero_unit, p, &hero) {
                scripts.push(script.clone());
            }
        }
        for script in scripts {
            script.trigger(handler, position, self)
        }
    }

    pub fn on_death(&self, handler: &mut EventHandler<D>, position: Point) {
        let game = handler.get_game();
        let mut scripts = self.get_commander(game).unit_death_scripts(self, game, position);
        for (p, hero_unit, hero) in game.get_map().hero_influence_at(position, self.get_owner_id()) {
            for script in self.environment.config.hero_death_scripts(game, self, position, &hero_unit, p, &hero) {
                scripts.push(script.clone());
            }
        }
        for script in scripts {
            script.trigger(handler, position, self)
        }
    }

    pub fn get_attack_scripts(&self, game: &Game<D>, position: Point, defender: &Unit<D>, defender_pos: Point) -> Vec<AttackScript> {
        let mut scripts = self.get_commander(game).unit_attack_scripts(self, game, position, defender, defender_pos);
        for (p, hero_unit, hero) in game.get_map().hero_influence_at(position, self.get_owner_id()) {
            for script in self.environment.config.hero_attack_scripts(game, self, position, &hero_unit, p, defender, defender_pos, &hero) {
                scripts.push(script.clone());
            }
        }
        scripts
    }

    pub fn get_kill_scripts(&self, game: &Game<D>, position: Point, defender: &Unit<D>, defender_pos: Point) -> Vec<KillScript> {
        let mut scripts = self.get_commander(game).unit_kill_scripts(self, game, position, defender, defender_pos);
        for (p, hero_unit, hero) in game.get_map().hero_influence_at(position, self.get_owner_id()) {
            for script in self.environment.config.hero_kill_scripts(game, self, position, &hero_unit, p, defender, defender_pos, &hero) {
                scripts.push(script.clone());
            }
        }
        scripts
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
            let value = key.default(&self.environment);
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
        for key in environment.unit_attributes(typ, owner) {
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

    pub fn fog_replacement(&self, game: &Game<D>, pos: Point, intensity: FogIntensity) -> Option<Self> {
        let hero = self.get_hero();
        let visibility = self.environment.config.commander_unit_visibility(&self.get_commander(game), self, game, pos);
        match intensity {
            FogIntensity::TrueSight => return Some(self.clone()),
            FogIntensity::NormalVision => {
                if visibility == UnitVisibility::Stealth {
                    return None
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
        let hidden_attributes = self.environment.unit_attributes_hidden_by_fog(self.typ, &hero, self.get_owner_id());
        for (k, v) in &self.attributes {
            if !hidden_attributes.contains(k) {
                builder.unit.attributes.insert(*k, v.clone());
            }
        }
        Some(builder.build_with_defaults())
    }

    pub fn movable_positions(&self, game: &Game<D>, path_so_far: &Path<D>) -> HashSet<Point> {
        movement_area_game(game, self, path_so_far, 1)
        .keys()
        .cloned()
        .collect()
    }

    pub fn attackable_positions(&self, game: &Game<D>, path: &Path<D>, get_fog: impl Fn(Point) -> FogIntensity) -> HashSet<Point> {
        let mut result = HashSet::new();
        if let Ok((destination, _)) = path.end(game.get_map()) {
            let mut this = self.clone();
            this.transformed_by_path(game.get_map(), path);
            if this.can_attack_after_moving() || path.steps.len() == 0 {
                for attack_vector in AttackVector::find(&this, game, destination, None, &get_fog) {
                    for (point, _, _) in attack_vector.get_splash(&this, game, destination, &get_fog) {
                        result.insert(point);
                    }
                }
            }
        }
        result
    }

    pub fn shortest_path_to(&self, game: &Game<D>, path_so_far: &Path<D>, goal: Point) -> Option<Path<D>> {
        search_path(game, self, path_so_far, None, |_path, p, can_stop_here| {
            if goal == p {
                PathSearchFeedback::Found
            } else if can_stop_here {
                PathSearchFeedback::Continue
            } else {
                PathSearchFeedback::ContinueWithoutStopping
            }
        })
    }

    pub fn shortest_path_to_attack(&self, game: &Game<D>, path_so_far: &Path<D>, goal: Point) -> Option<Path<D>> {
        if AttackType::None == self.attack_pattern() {
            return None;
        }
        let get_fog = |p| {
            FogIntensity::TrueSight
        };
        if path_so_far.steps.len() == 0 && AttackVector::find(self, game, path_so_far.start, Some(goal), get_fog).len() > 0 {
            return Some(path_so_far.clone());
        }
        if !self.can_attack_after_moving() && !self.can_take() {
            // no need to look for paths if the unit can't attack after moving
            return None;
        }
        search_path(game, self, path_so_far, None, |path, p, can_stop_here| {
            if !can_stop_here {
                PathSearchFeedback::ContinueWithoutStopping
            } else if goal == p && can_stop_here && self.can_take() {
                PathSearchFeedback::Found
            } else if self.can_attack_after_moving() && AttackVector::find(self, game, p, Some(goal), get_fog).len() > 0 {
                PathSearchFeedback::Found
            } else {
                PathSearchFeedback::Continue
            }
        })
    }

    pub fn transformed_by_movement(&mut self, map: &Map<D>, from: Point, to: Point, distortion: Distortion<D>) -> bool {
        let prev_terrain = map.get_terrain(from).unwrap();
        let mut changed = HashMap::new();
        if let Some(Attribute::Amphibious(amphibious)) = self.attributes.get(&AttributeKey::Amphibious) {
            let new_amph = match map.get_terrain(to).unwrap().get_amphibious() {
                None => None,
                Some(AmphibiousTyping::Beach) |
                Some(AmphibiousTyping::Land) => Some(Amphibious::OnLand),
                Some(AmphibiousTyping::Sea) => Some(Amphibious::InWater),
            };
            if new_amph.is_some() && new_amph != Some(*amphibious) {
                changed.insert(AttributeKey::Amphibious, Attribute::Amphibious(new_amph.unwrap()));
            }
        }
        if let Some(Attribute::Direction(dir)) = self.attributes.get(&AttributeKey::Direction) {
            let new_dir = distortion.update_direction(*dir);
            if new_dir != *dir {
                changed.insert(AttributeKey::Direction, Attribute::Direction(new_dir));
            }
        }
        if changed.len() > 0 {
            for (k, v) in changed {
                self.attributes.insert(k, v);
            }
            true
        } else {
            false
        }
    }

    pub fn transformed_by_path(&mut self, map: &Map<D>, path: &Path<D>) -> bool {
        let mut current = path.start;
        let mut changed = false;
        for step in &path.steps {
            let (next, distortion) = match step.progress(map, current) {
                Ok(n) => n,
                _ => return changed,
            };
            changed = self.transformed_by_movement(map, current, next, distortion) || changed;
            current = next;
        }
        changed
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

    pub fn threatens(&self, defender: &Self) -> bool {
        //let terrain = game.get_map().get_terrain(target_pos).unwrap();
        //let in_water = terrain.is_water();
        self.could_attack(defender, false) && defender.get_team() != self.get_team()
    }

    pub fn options_after_path(&self, game: &Game<D>, path: &Path<D>) -> Vec<UnitAction<D>> {
        let mut this = self.clone();
        this.transformed_by_path(game.get_map(), path);
        if let Ok((end, _)) = path.end(game.get_map()) {
            this._options_after_path_transformed(game, path, end)
        } else {
            Vec::new()
        }
    }

    fn _options_after_path_transformed(&self, game: &Game<D>, path: &Path<D>, destination: Point) -> Vec<UnitAction<D>> {
        let fog = game.get_fog().get(&self.get_team());
        let get_fog = |p| {
            fog.and_then(|f| f.get(&p)).cloned().unwrap_or(FogIntensity::TrueSight)
        };
        let mut result = Vec::new();
        let path_points: HashSet<Point> = path.points(game.get_map()).unwrap().into_iter().collect();
        let player = self.environment.settings.as_ref().unwrap().players.get(self.get_owner_id() as usize).unwrap();
        let mut funds_after_path = *game.current_player().funds;
        let income = player.get_income();
        for p in path_points {
            for detail in game.get_map().get_details(p) {
                match detail.fog_replacement(get_fog(p)) {
                    Some(Detail::Coins1) => funds_after_path += income / 2,
                    Some(Detail::Coins2) => funds_after_path += income,
                    Some(Detail::Coins4) => funds_after_path += income * 2,
                    _ => {}
                }
            }
        }
        // terrain has to exist since destination point was found from path
        let terrain = game.get_map().get_terrain(destination).unwrap();
        let blocking_unit = game.get_map().get_unit(destination).and_then(|u| u.fog_replacement(game, destination, get_fog(destination)));
        if path.start != destination && blocking_unit.is_some() {
            if let Some(transporter) = game.get_map().get_unit(destination) {
                if transporter.can_transport(self) {
                    result.push(UnitAction::Enter);
                }
            }
        } else if blocking_unit.is_none() || path.start == destination && blocking_unit.as_ref() == Some(self) {
            // hero power
            self.get_hero().add_options_after_path(&mut result, self, game, path, destination, get_fog);
            // build units
            if self.can_build_units() && self.transport_capacity() > 0 {
                let mut free_space = self.remaining_transport_capacity();
                if let Some(drone_id) = self.get_drone_station_id() {
                    let mut outside = 0;
                    for p in game.get_map().all_points() {
                        if let Some(u) = game.get_map().get_unit(p) {
                            if u.get_drone_id() == Some(drone_id) {
                                outside += 1;
                            }
                        }
                    }
                    free_space = free_space.max(outside) - outside;
                }
                if free_space > 0 {
                    for unit in self.transportable_units() {
                        if unit.price(&self.environment, self.get_owner_id()) <= funds_after_path {
                            result.push(UnitAction::BuyTransportedUnit(*unit));
                        }
                    }
                }
            } else if self.can_build_units() && self.transport_capacity() == 0 {
                let attr_overrides = self.build_overrides(game, destination);
                for unit in self.transportable_units() {
                    if unit.price(&self.environment, self.get_owner_id()) <= funds_after_path {
                        let mut amphibious = Amphibious::default();
                        for attr_override in &attr_overrides {
                            if !self.environment.unit_attributes(*unit, self.get_owner_id()).any(|k| *k == attr_override.key()) {
                                continue;
                            }
                            match attr_override {
                                AttributeOverride::InWater => amphibious = Amphibious::InWater,
                                AttributeOverride::OnLand => amphibious = Amphibious::OnLand,
                                _ => (),
                            }
                        }
                        for d in D::list() {
                            if game.get_map().get_neighbor(destination, d)
                            .and_then(|(p, _)| game.get_map().get_terrain(p).unwrap().movement_cost(self.environment.config.movement_type(*unit, amphibious)))
                            .is_some() {
                                result.push(UnitAction::BuyUnit(*unit, d));
                            }
                        }
                    }
                }
            }
            // attack
            if self.can_attack_after_moving() || path.steps.len() == 0 {
                for attack_vector in AttackVector::find(self, game, destination, None, get_fog) {
                    result.push(UnitAction::Attack(attack_vector));
                }
            }
            if self.can_capture() && terrain.has_attribute(TerrainAttributeKey::Owner) && terrain.get_team() != self.get_team() {
                result.push(UnitAction::Capture);
            }
            if self.get_hp() < 100 && terrain.can_repair() && terrain.can_repair_unit(self.typ)
            && (!terrain.has_attribute(TerrainAttributeKey::Owner) || terrain.get_owner_id() == self.get_owner_id())
            && funds_after_path * 100 >= self.typ.price(&self.environment, self.get_owner_id()) {
                result.push(UnitAction::Repair);
            }
            result.push(UnitAction::Wait);
        }
        println!("unit actions: {result:?}");
        result
    }
}

impl<D: Direction> SupportedZippable<&Environment> for Unit<D> {
    fn export(&self, zipper: &mut Zipper, support: &Environment) {
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
        let mut unit = Unit::new(environment.clone(), typ);
        Self {
            unit,
        }
    }

    pub fn copy_from(mut self, other: &Unit<D>) -> Self {
        if self.unit.environment != other.environment {
            panic!("Can't copy from unit from different environment");
        }
        for (key, value) in &other.attributes {
            // TODO: consider all attributes, not just unit-specific ones
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
        if let Some(mut transported_mut) = self.unit.get_transported_mut() {
            for unit in transported {
                transported_mut.push(unit);
            }
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
                unit.attributes.insert(*key, key.default(&self.unit.environment));
            }
        }
        unit
    }
}

