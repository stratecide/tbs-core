use std::collections::{HashSet, HashMap};
use std::fmt::Debug;
use std::hash::Hash;
use std::ops::{Deref, DerefMut};

use interfaces::ClientPerspective;
use num_rational::Rational32;
use rhai::Scope;
use uniform_smart_pointer::Urc;
use zipper::*;

use crate::combat::{AllowedAttackInputDirectionSource, AttackCounterState, AttackInput, AttackPattern};
use crate::commander::commander_type::CommanderType;
use crate::config::environment::Environment;
use crate::config::movement_type_config::MovementPattern;
use crate::config::OwnershipPredicate;
use crate::game::fog::{get_visible_unit, is_unit_attribute_visible, FogIntensity, FogSetting, VisionMode};
use crate::game::settings::GameSettings;
use crate::commander::Commander;
use crate::map::board::{Board, BoardView};
use crate::map::direction::Direction;
use crate::map::map::get_neighbors_layers;
use crate::map::point::Point;
use crate::map::wrapping_map::Distortion;
use crate::player::{Player, Owner};
use crate::script::*;
use crate::tags::{TagBag, TagValue};
use crate::units::UnitData;

use super::UnitVisibility;
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
}

impl<D: Direction> PartialEq for Unit<D> {
    // compare everything except environment
    fn eq(&self, other: &Self) -> bool {
        self.typ == other.typ
        && self.owner == other.owner
        && self.sub_movement_type == other.sub_movement_type
        && self.hero == other.hero
        && self.tags == other.tags
        && self.transport == other.transport
    }
}

impl<D: Direction> Debug for Unit<D> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}(", self.name())?;
        write!(f, "Owner: {}, ", self.owner.0)?;
        if self.environment.config.sub_movement_types(self.environment.config.base_movement_type(self.typ)).len() > 0 {
            write!(f, "MovementType: {}, ", self.environment.config.movement_type_name(self.sub_movement_type))?;
        }
        if let Some(hero) = &self.hero {
            write!(f, "Hero: {hero:?}, ")?;
        }
        self.tags.debug(f, &self.environment)?;
        if self.transport.len() > 0 {
            write!(f, ", Transporting: {:?}", self.transport)?;
        }
        write!(f, ")")
    }
}

impl<D: Direction> Unit<D> {
    pub(super) fn new(environment: Environment, typ: UnitType) -> Self {
        let owner = match environment.config.unit_ownership(typ) {
            OwnershipPredicate::Always => environment.config.max_player_count() - 1,
            _ => -1
        };
        Self {
            typ,
            owner: Owner(owner),
            sub_movement_type: environment.config.sub_movement_types(environment.config.base_movement_type(typ))[0],
            hero: None,
            tags: TagBag::new(),
            transport: Vec::new(),
            environment,
        }
    }

    pub(crate) fn start_game(&mut self, settings: &Urc<GameSettings>) {
        self.environment.start_game(settings);
        for unit in self.get_transported_mut().deref_mut() {
            unit.start_game(settings);
        }
    }

    // getters that aren't influenced by attributes
    pub fn environment(&self) -> &Environment {
        &self.environment
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
        self.environment.unit_transport_capacity(self.typ, self.get_owner_id(), self.get_hero().map(|hero| hero.typ()))
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

    pub fn can_pass_enemy_units(&self, game: &Board<D>, unit_pos: (Point, Option<usize>), transporter: Option<(&Unit<D>, Point)>, heroes: &HeroMap<D>) -> bool {
        self.environment.config.unit_can_pass_enemy_units(game, self, unit_pos, transporter, heroes)
    }

    pub fn can_be_moved_through(&self) -> bool {
        self.environment.config.can_be_moved_through(self.typ)
    }

    pub fn can_be_taken(&self) -> bool {
        self.environment.config.can_be_taken(self.typ)
    }

    pub fn can_attack_after_moving(&self) -> bool {
        self.environment.config.can_attack_after_moving(self.typ)
    }

    pub fn can_target(&self, game: &Board<D>, unit_pos: Point, transporter: Option<(&Unit<D>, Point)>, target: UnitData<D>, is_counter: bool, heroes: &HeroMap<D>) -> bool {
        self.environment.config.unit_is_target_valid(game, self, unit_pos, target, transporter, is_counter, heroes)
    }

    pub fn attack_pattern(&self, game: &Board<D>, unit_pos: Point, counter: &AttackCounterState<D>, heroes: &HeroMap<D>, temporary_ballast: &[TBallast<D>]) -> AttackPattern {
        self.environment.config.unit_attack_pattern(game, self, unit_pos, counter, heroes, temporary_ballast)
    }

    pub fn attack_pattern_directions(&self, game: &Board<D>, unit_pos: Point, counter: &AttackCounterState<D>, heroes: &HeroMap<D>, temporary_ballast: &[TBallast<D>]) -> AllowedAttackInputDirectionSource {
        self.environment.config.unit_attack_directions(game, self, unit_pos, counter, heroes, temporary_ballast)
    }

    pub fn can_be_displaced(&self, game: &Board<D>, pos: Point, attacker: &Self, attacker_pos: Point, heroes: &HeroMap<D>, is_counter: bool) -> bool {
        self.environment.config.unit_can_be_displaced(game, self, pos, attacker, attacker_pos, heroes, is_counter)
    }

    pub fn vision_mode(&self) -> VisionMode {
        self.environment.config.vision_mode(self.typ)
    }

    pub fn vision_range(&self, game: &Board<D>, pos: Point, heroes: &HeroMap<D>) -> usize {
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

    fn true_vision_range(&self, game: &Board<D>, pos: Point, heroes: &HeroMap<D>) -> usize {
        self.environment.config.unit_true_vision(game, self, pos, heroes)
    }

    pub fn get_vision(&self, game: &Board<D>, pos: Point, heroes: &HeroMap<D>) -> HashMap<Point, FogIntensity> {
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
                let layers = get_neighbors_layers(game, pos, vision_range);
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
                let game = game.ignore_units();
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

    pub fn get_owner_id(&self) -> i8 {
        self.owner.0
    }
    pub fn set_owner_id(&mut self, id: i8) {
        match self.environment.config.unit_ownership(self.typ) {
            OwnershipPredicate::Always if id < 0 => (),
            OwnershipPredicate::Never if id >= 0 => (),
            _ => {
                self.owner.0 = id;
                self.fix_transported();
            }
        }
    }

    pub fn get_team(&self) -> ClientPerspective {
        self.environment.get_team(self.get_owner_id())
    }

    pub fn get_player<'a>(&self, game: &'a impl BoardView<D>) -> Option<&'a Player<D>> {
        game.get_owning_player(self.get_owner_id())
    }

    pub fn get_commander(&self, game: &impl BoardView<D>) -> Commander {
        self.get_player(game)
        .and_then(|player| Some(player.commander.clone()))
        .unwrap_or(Commander::new(&self.environment, CommanderType(0)))
    }

    pub(super) fn copy_from(&mut self, other: &TagBag<D>) {
        for key in other.flags() {
            self.set_flag(*key);
        }
        for (key, value) in other.tags() {
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

    // TODO: hardcoded for movement
    // replace when movement can be customized
    pub fn get_pawn_direction(&self) -> D {
        match self.environment.config.tag_by_name("PawnDirection")
        .and_then(|key| self.tags.get_tag(key)) {
            Some(TagValue::Direction(d)) => d,
            _ => D::angle_0()
        }
    }
    pub fn set_pawn_direction(&mut self, direction: D) {
        if let Some(key) = self.environment.config.tag_by_name("PawnDirection") {
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
        self.hero = Some(hero);
        // hero might influence transport-capacity
        self.fix_transported()
    }
    pub fn remove_hero(&mut self) {
        self.hero = None;
        // hero might influence transport-capacity
        self.fix_transported()
    }

    pub fn get_max_charge(&self) -> u32 {
        self.hero.as_ref()
        .map(|hero| hero.max_charge(&self.environment))
        .unwrap_or(0)
    }
    pub fn get_charge(&self) -> u32 {
        self.hero.as_ref()
        .map(|hero| hero.get_charge())
        .unwrap_or(0)
    }
    pub fn set_charge(&mut self, charge: u32) {
        if let Some(hero) = &mut self.hero {
            hero.set_charge(&self.environment, charge);
        }
    }

    pub fn can_move(&self, board: &Board<D>, pos: Point) -> bool {
        // can the unit be moved?
        let environment = self.environment().clone();
        let is_unit_movable_rhai = environment.is_unit_movable_rhai();
        let mut scope = Scope::new();
        scope.push_constant(CONST_NAME_POSITION, pos);
        scope.push_constant(CONST_NAME_UNIT, self.clone());
        let executor = board.executor(scope);
        match executor.run::<D, bool>(is_unit_movable_rhai, ()) {
            Ok(movable) => movable,
            Err(e) => {
                let environment = self.environment();
                environment.log_rhai_error("is_unit_movable_rhai", environment.get_rhai_function_name(is_unit_movable_rhai), &e);
                false
            }
        }
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
    }

    // TODO: hardcoded for movement
    // replace when movement can be customized
    pub fn get_en_passant(&self) -> Option<Point> {
        let key = self.environment.config.tag_by_name("EnPassant")?;
        match self.tags.get_tag(key) {
            Some(TagValue::Point(p)) => Some(p),
            _ => None
        }
    }

    // influenced by unit_power_config

    pub fn value(&self, game: &Board<D>, position: Point, factory: Option<&Unit<D>>, heroes: &HeroMap<D>) -> i32 {
        self.environment.config.unit_value(
            game,
            self,
            position,
            factory,
            heroes,
        )
    }

    pub fn movement_points(&self, game: &Board<D>, position: Point, transporter: Option<&Unit<D>>, heroes: &HeroMap<D>) -> Rational32 {
        self.environment.config.unit_movement_points(
            game,
            self,
            (position, None),
            transporter.map(|u| (u, position)),
            heroes,
        )
    }

    pub fn on_death(&self, game: &Board<D>, position: Point, transporter: Option<(&Self, usize)>, attacker: Option<UnitData<D>>, heroes: &HeroMap<D>, temporary_ballast: &[TBallast<D>]) -> Vec<usize> {
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
        self.owner.export(zipper, &*self.environment.config);
        let sub_movement_types = self.environment.config.sub_movement_types(self.base_movement_type());
        if sub_movement_types.len() > 1 {
            let bits = bits_needed_for_max_value(sub_movement_types.len() as u32 - 1);
            zipper.write_u32(sub_movement_types.iter().position(|mt| *mt == self.sub_movement_type).unwrap() as u32, bits);
        }
        self.hero.export(zipper, &self.environment);
        self.tags.export(zipper, &self.environment);
        if !transported && self.transport_capacity() > 0 {
            zipper.write_u32(self.transport.len() as u32, bits_needed_for_max_value(self.transport_capacity() as u32));
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

    pub fn visibility(&self, game: &Board<D>, pos: Point) -> UnitVisibility {
        // for now, heroes don't affect unit visibility.
        // when they do in the future, the heroes should be given to this method instead of calculating here
        // it could also be necessary to add this unit's hero to the heroes list here manually (if it isn't already in there)
        self.environment.config.unit_visibility(game, self, pos)
    }

    pub fn fog_replacement(&self, game: &Board<D>, pos: Point, intensity: FogIntensity) -> Option<Self> {
        let visibility = self.visibility(game, pos);
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
                        return Some(builder.build())
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
        Some(builder.build())
    }

    pub fn movable_positions(&self, game: &Board<D>, path_so_far: &Path<D>, transporter: Option<(&Unit<D>, usize)>) -> HashSet<Point> {
        movement_area_game(game, self, path_so_far, 1, transporter)
        .keys()
        .cloned()
        .collect()
    }

    pub fn attackable_positions(&self, game: &Board<D>, path: &Path<D>, transporter: Option<(&Unit<D>, usize)>, ballast: &[TBallast<D>]) -> HashSet<Point> {
        let mut result = HashSet::new();
        if let Some((game, destination, this)) = game.unit_path_without_placing(transporter.map(|(_, i)| i), path) {
            if (this.can_attack_after_moving() || path.steps.len() == 0) && game.get_unit(destination).is_none() {
                let game = game.replace_unit(destination, Some(this.clone()));
                let heroes = HeroMap::new(&game, Some(self.get_owner_id()));
                for input in AttackInput::attackable_positions(&game, self, destination, transporter.map(|(u, _)| (u, path.start)), ballast, &heroes) {
                    result.insert(input.target());
                }
            }
        }
        result
    }

    pub fn shortest_path_to(&self, game: &Board<D>, path_so_far: &Path<D>, transporter: Option<(&Unit<D>, usize)>, goal: Point) -> Option<(Path<D>, TemporaryBallast<D>)> {
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

    pub fn shortest_path_to_attack(&self, game: &Board<D>, path_so_far: &Path<D>, transporter: Option<(&Unit<D>, usize)>, goal: Point) -> Option<(Path<D>, TemporaryBallast<D>)> {
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
            } else if (path.steps.len() == 0 || self.can_attack_after_moving()) && self.attackable_positions(game, path, transporter, ballast.get_entries()).contains(&goal) {
                // TODO: check if unit at goal can be targeted
                return PathSearchFeedback::Found
            }
            PathSearchFeedback::Continue
        })
    }

    pub fn transformed_by_movement(&mut self, map: &Board<D>, _from: Point, to: Point, distortion: Distortion<D>) -> bool {
        let terrain = map.get_terrain(to).unwrap();
        let permanent = PermanentBallast::from_unit(self);
        let changed = permanent.step(distortion, &terrain);
        changed.update_unit(self);
        changed != permanent
    }

    pub fn transformed_by_path(&mut self, map: &Board<D>, path: &Path<D>) {
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

    pub fn could_take(&self, defender: &Self, takes: PathStepTakes) -> bool {
        takes != PathStepTakes::Deny && defender.can_be_taken() && self.get_team() != defender.get_team()
    }

    pub fn options_after_path(&self, game: &Board<D>, path: &Path<D>, transporter: Option<(&Unit<D>, usize)>, ballast: &[TBallast<D>]) -> Vec<UnitAction<D>> {
        if let Some((game, end, this)) = game.unit_path_without_placing(transporter.map(|(_, i)| i), path) {
            this._options_after_path_transformed(&game, path, end, transporter, ballast)
        } else {
            Vec::new()
        }
    }

    fn _options_after_path_transformed(&self, game: &Board<D>, path: &Path<D>, destination: Point, transporter: Option<(&Unit<D>, usize)>, ballast: &[TBallast<D>]) -> Vec<UnitAction<D>> {
        let mut result = Vec::new();
        let team = self.get_team();
        let blocking_unit = get_visible_unit(game, team, destination);
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
            if let Some(transporter) = get_visible_unit(game, team, destination) {
                if transporter.can_transport(self) {
                    result.push(UnitAction::Enter);
                }
            }
        } else if blocking_unit.is_none() {
            let game = game.replace_unit(destination, Some(self.clone()));
            let heroes = HeroMap::new(&game, Some(self.get_owner_id()));
            // hero power
            if let Some(hero) = &self.hero {
                hero.add_options_after_path(&mut result, &game);
            }
            // custom actions
            let custom_actions = self.environment.config.custom_actions();
            if custom_actions.len() > 0 {
                let unit_data = UnitData {
                    unit: self,
                    pos: destination,
                    unload_index: None,
                    original_transporter: transporter.map(|(u, _)| (u, path.start)),
                    ballast,
                };
                for (i, custom_action) in custom_actions.iter().enumerate() {
                    if custom_action.unit_filter.iter().all(|f| f.check(&game, unit_data, None, &heroes, false)) {
                        result.push(UnitAction::custom(i, Vec::new()));
                    }
                }
            }
            // attack
            if self.can_attack_after_moving() || path.steps.len() == 0 {
                let transporter = transporter.map(|(u, _)| (u, path.start));
                for input in AttackInput::attackable_positions(&game, self, destination, transporter, ballast, &heroes) {
                    if let Some(defender) = game.get_unit(input.target()) {
                        let target = UnitData {
                            unit: &defender,
                            pos: input.target(),
                            unload_index: None,
                            ballast: &[],
                            original_transporter: None,
                        };
                        if self.can_target(&game, destination, transporter, target, false, &heroes) {
                            result.push(UnitAction::Attack(input));
                        }
                    }
                }
            }
            result.push(UnitAction::Wait);
        }
        //tracing::debug!("unit actions: {result:?}");
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
        self.unit.copy_from(other.get_tag_bag());
        self
    }

    pub fn set_tag_bag(mut self, bag: TagBag<D>) -> Self {
        self.unit.tags = bag;
        self
    }

    pub fn set_flag(mut self, key: usize) -> Self {
        self.unit.set_flag(key);
        self
    }
    pub fn remove_flag(mut self, key: usize) -> Self {
        self.unit.remove_flag(key);
        self
    }

    pub fn set_tag(mut self, key: usize, value: TagValue<D>) -> Self {
        self.unit.set_tag(key, value);
        self
    }
    pub fn remove_tag(mut self, key: usize) -> Self {
        self.unit.remove_tag(key);
        self
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

    pub fn set_hero(mut self, hero: Hero) -> Self {
        self.unit.set_hero(hero);
        self
    }

    pub fn set_transported(mut self, transported: Vec<Unit<D>>) -> Self {
        *self.unit.get_transported_mut() = transported;
        self
    }

    /**
     * TODO: call rhai script to get default flags/tag values?
     */
    pub fn build(&self) -> Unit<D> {
        self.unit.clone()
    }
}

