use std::marker::PhantomData;
use std::ptr::with_exposed_provenance_mut;

use interfaces::GameInterface;
use interfaces::{ClientPerspective, Perspective as IPerspective, RandomFn};
use rhai::{Dynamic, Scope};
use rustc_hash::{FxHashMap as HashMap, FxHashSet as HashSet};

use crate::config::environment::Environment;
use crate::config::global_events::GlobalEventConfig;
use crate::map::board::{Board, BoardView};
use crate::map::map::*;
use crate::map::point::Point;
use crate::map::wrapping_map::Distortion;
use crate::script::custom_action::execute_commander_script;
use crate::script::executor::Executor;
use crate::script::*;
use crate::tags::*;
use crate::terrain::terrain::*;
use crate::tokens::MAX_STACK_SIZE;
use crate::player::*;
use crate::tokens::token::Token;
use crate::map::direction::Direction;
use crate::game::game::*;
use crate::game::fog::*;
use crate::units::hero::HeroMap;
use crate::units::hero::{Hero, HeroInfluence};
use crate::units::movement::{Path, TBallast};
use crate::units::unit::Unit;
use crate::units::UnitData;
use crate::units::UnitId;
use super::event_fx::*;
use super::events::Event;

pub struct EventHandler<'a, D: Direction> {
    game: &'a mut Game<D>,
    // Self::board contains an immutable reference to Self::game
    // this should be safe because Self::game is only modified in Self::add_event
    // which needs a mutable reference to self
    // so there can't be an immutable referance to Self::board at the same time
    board: Board<'a, D>,
    events: HashMap<IPerspective, Vec<Event<D>>>,
    random: RandomFn,
    observed_units: HashMap<usize, (Point, Option<usize>, Distortion<D>)>,
    next_observed_unit_id: usize,
}

impl<'a, D: Direction> EventHandler<'a, D> {
    pub fn new(game: &'a mut Game<D>, random: RandomFn) -> Self {
        let mut events = HashMap::default();
        events.insert(IPerspective::Server, Vec::new());
        events.insert(IPerspective::Neutral, Vec::new());
        for team in game.get_teams() {
            events.insert(IPerspective::Team(team), Vec::new());
        }
        let r = &raw const *game;
        let h = unsafe {&*r};
        let board = Board::from(h);
        Self {
            game,
            board,
            events,
            random,
            next_observed_unit_id: 0,
            observed_units: HashMap::default(),
        }
    }

    pub fn environment(&self) -> &Environment {
        &self.get_game().environment()
    }

    pub fn get_game(&self) -> &Game<D> {
        &self.game
    }

    pub fn get_board<'b>(&'b self) -> &'b Board<'b, D> {
        &self.board
    }

    pub fn rng(&self) -> f32 {
        (*self.random)()
    }

    pub fn observe_unit(&mut self, position: Point, unload_index: Option<usize>) -> UnitId<D> {
        if let Some((id, (_, _, distortion))) = self.observed_units.iter()
        .find(|(_, (p, i, _))| *p == position && *i == unload_index) {
            UnitId(*id, *distortion)
        } else {
            self.observed_units.insert(self.next_observed_unit_id, (position, unload_index, Distortion::neutral()));
            self.next_observed_unit_id += 1;
            UnitId(self.next_observed_unit_id - 1, Distortion::neutral())
        }
    }

    pub fn get_observed_unit_id(&self, position: Point, unload_index: Option<usize>) -> Option<UnitId<D>> {
        self.observed_units.iter()
            .find(|(_, (p, i, _))| *p == position && *i == unload_index)
            .map(|(id, (_, _, distortion))| UnitId(*id, *distortion))
    }

    pub fn get_observed_unit(&self, id: usize) -> Option<(Point, Option<usize>, Distortion<D>)> {
        self.observed_units.get(&id).cloned()
    }

    pub fn get_observed_unit_pos(&self, id: usize) -> Option<(Point, Option<usize>)> {
        self.observed_units.get(&id)
        .map(|(p, unload_index, _)| (*p, *unload_index))
    }

    fn observation_id(&self, position: Point, unload_index: Option<usize>) -> Option<UnitId<D>> {
        self.observed_units.iter()
        .find(|(_, (p, i, _))| *p == position && *i == unload_index)
        .map(|(id, (_, _, distortion))| UnitId(*id, *distortion))
    }

    fn remove_observed_units_at(&mut self, position: Point) {
        if let Some(UnitId(id, _)) = self.observation_id(position, None) {
            self.observed_units.remove(&id);
        }
        for i in 0..self.environment().config.max_transported() {
            if let Some(UnitId(id, _)) = self.observation_id(position, Some(i)) {
                self.observed_units.remove(&id);
            }
        }
    }

    pub fn move_observed_unit(&mut self, id: usize, p: Point, unload_index: Option<usize>, distortion: Distortion<D>) {
        self.observed_units.insert(id, (p, unload_index, distortion));
    }

    fn add_event(&mut self, event: Event<D>) {
        event.apply(self.game);
        for (key, events) in self.events.iter_mut() {
            if let Ok(perspective) = key.try_into() {
                for event in event.fog_replacement(&*self.game, perspective) {
                    events.push(event);
                }
            }
        }
        self.events.get_mut(&IPerspective::Server).unwrap().push(event);
    }

    pub fn end_turn(&mut self) {

        self.trigger_all_global_events(|conf| conf.on_end_turn);

        let fog_before = if self.get_game().has_secrets() {
            let team = self.get_game().players.get((self.get_game().current_turn() + 1) % self.get_game().players.len()).unwrap().get_team();
            Some(recalculate_fog(&self.board, team))
        } else {
            None
        };

        self.next_turn();
        let owner_id = self.get_game().current_owner();

        if let Some((power_index, function_index, charge_cost)) = {
            let commander = &self.get_game().current_player().commander;
            Some(commander.get_next_power())
            .filter(|power| commander.can_activate_power(*power, true))
            .map(|power| {
                (
                    power,
                    commander.power_activation_script(power)
                        .filter(|(input_script, _)| input_script.is_none())
                        .map(|(_, function_index)| function_index),
                    commander.power_cost(power),
                )
            })
        } {
            self.add_commander_charge(owner_id, -(charge_cost as i32));
            self.commander_power(owner_id, power_index);
            if let Some(function_index) = function_index {
                execute_commander_script(function_index, self, None);
            }
        }

        // end merc powers
        for p in valid_points(self.get_game()) {
            if let Some(hero) = self.get_game().get_map().get_unit(p).filter(|u| u.get_owner_id() == owner_id).and_then(|u| u.get_hero()).cloned() {
                let next_power = hero.get_next_power(self.environment());
                if hero.can_activate_power(self.environment(), next_power, true) {
                    // TODO: this skips the custom-action. maybe execute the custom action if no user input is needed
                    self.add_hero_charge(p, None, -(hero.power_cost(self.environment(), next_power) as i32));
                    self.hero_power(p, next_power);
                }
            }
        }

        self.start_turn(fog_before);

        if !self.get_game().has_ended() && self.get_game().current_player().dead {
            self.end_turn();
        }
    }

    pub fn next_turn(&mut self) {
        self.add_event(Event::NextTurn);
    }

    pub fn start_turn(&mut self, fog_before: Option<HashMap<Point, FogIntensity>>) {
        // hide / reveal player funds if fog started / ended
        let was_foggy = fog_before.is_some();
        if was_foggy != self.get_game().has_secrets() {
            self.add_event(Event::PurePlayerFog);
        }

        self.trigger_all_global_events(|conf| conf.on_start_turn);

        // structures may have destroyed some units, vision may be reduced due to merc powers ending
        self.recalculate_fog();
    }

    pub fn recalculate_fog(&mut self) {
        let current_team = self.get_game().current_team();
        // only remove fog for the current team
        let mut fog = recalculate_fog(&self.board, current_team);
        for (p, intensity) in fog.iter_mut() {
            *intensity = self.get_game().get_fog_at(current_team, *p).min(*intensity);
        }
        self.change_fog(current_team, fog);
        // reset fog for other teams
        let mut perspectives: HashSet<ClientPerspective> = self.get_game().get_teams().into_iter()
        .filter(|team| ClientPerspective::Team(*team) != current_team)
        .map(|team| ClientPerspective::Team(team))
        .collect();
        perspectives.insert(ClientPerspective::Neutral);
        for team in perspectives {
            self.recalculate_fog_for(team);
        }
    }

    pub fn recalculate_fog_for(&mut self, team: ClientPerspective) {
        let fog = recalculate_fog(&self.board, team);
        self.change_fog(team, fog);
    }

    pub fn change_fog(&mut self, team: ClientPerspective, changes: HashMap<Point, FogIntensity>) {
        let changes: Vec<(Point, FogIntensity, FogIntensity)> = changes.into_iter()
        .map(|(p, intensity)| (p, self.get_game().get_fog_at(team, p), intensity))
        .filter(|(_, before, after)| before != after)
        .collect();
        if changes.len() > 0 {
            self.add_event(Event::PureFogChange(from_client_perspective(team).into(), changes.try_into().unwrap()));
        }
    }

    pub fn add_commander_charge(&mut self, owner: i8, delta: i32) {
        if let Some(player) = self.get_game().get_owning_player(owner).cloned() {
            if !player.commander.can_gain_charge() {
                return;
            }
            let delta = delta.max(-(player.commander.get_charge() as i32)).min((player.commander.get_max_charge() - player.commander.get_charge()) as i32);
            if delta != 0 {
                self.add_event(Event::CommanderCharge(owner.into(), delta.into()));
            }
        }
    }

    pub fn commander_power(&mut self, owner: i8, index: usize) {
        if let Some(player) = self.get_game().get_owning_player(owner).cloned() {
            if player.commander.get_active_power() != index {
                self.add_event(Event::CommanderPowerIndex(owner.into(), player.commander.get_active_power().into(), index.into()));
            }
        }
    }

    pub fn token_add(&mut self, position: Point, token: Token<D>) {
        let old_tokens = self.get_game().get_tokens(position).to_vec();
        // should be same as Token::correct_stack
        let mut tokens: Vec<Token<D>> = old_tokens.iter()
        .filter(|t| t.typ() != token.typ() || t.get_owner_id() != token.get_owner_id())
        .cloned().collect();
        if tokens.len() < MAX_STACK_SIZE as usize {
            tokens.push(token);
            self.add_event(Event::ReplaceToken(position, old_tokens.try_into().unwrap(), Token::correct_stack(tokens).try_into().unwrap()));
        }
    }

    pub fn token_remove(&mut self, position: Point, index: usize) {
        if let Some(token) = self.get_game().get_tokens(position).get(index).cloned() {
            self.add_event(Event::RemoveToken(position, index.into(), token));
        } else {
            panic!("Missing Token at {position:?}");
        }
    }

    pub fn effect(&mut self, effect: Effect<D>) {
        self.add_event(Event::Effect(effect));
    }

    pub fn effects(&mut self, mut effects: Vec<Effect<D>>) {
        match effects.len() {
            0 => return,
            1 => self.effect(effects.pop().unwrap()),
            _ => {
                match effects.try_into() {
                    Ok(effects) => self.add_event(Event::Effects(effects)),
                    Err(_) => self.effect_glitch(),
                };
                
            }
        }
    }

    pub fn effect_glitch(&mut self) {
        self.add_event(Event::Effect(Effect::new_glitch()));
    }

    pub fn effect_fog_surprise(&mut self, position: Point) {
        self.add_event(Event::Effect(Effect::new_fog_surprise(position)));
    }

    pub fn unit_set_hero(&mut self, position: Point, hero: Hero) {
        let unit = self.get_game().get_unit(position).expect(&format!("Missing unit at {:?}", position)).clone();
        if !unit.is_hero() {
            self.add_event(Event::HeroSet(position, hero));
        }
    }

    pub fn add_hero_charge(&mut self, position: Point, unload_index: Option<usize>, delta: i32) {
        let unit = self.get_game().get_unit(position).expect(&format!("Missing unit at {:?}", position)).clone();
        let Some(hero) = unit.get_hero() else {
            return;
        };
        let delta = delta.max(-(hero.get_charge() as i32)).min((hero.typ().max_charge(self.environment()) - hero.get_charge()) as i32);
        if delta != 0 {
            if let Some(unload_index) = unload_index {
                self.add_event(Event::HeroChargeTransported(position, unload_index.into(), delta.into()));
            } else {
                self.add_event(Event::HeroCharge(position, delta.into()));
            }
        }
    }

    pub fn set_hero_charge(&mut self, position: Point, unload_index: Option<usize>, charge: i32) {
        let unit = self.get_game().get_unit(position).expect(&format!("Missing unit at {:?}", position)).clone();
        let Some(hero) = unit.get_hero() else {
            return;
        };
        let delta = charge - hero.get_charge() as i32;
        let delta = delta.max(-(hero.get_charge() as i32)).min((hero.typ().max_charge(self.environment()) - hero.get_charge()) as i32);
        if delta != 0 {
            if let Some(unload_index) = unload_index {
                self.add_event(Event::HeroChargeTransported(position, unload_index.into(), delta.into()));
            } else {
                self.add_event(Event::HeroCharge(position, delta.into()));
            }
        }
    }

    pub fn hero_power(&mut self, position: Point, index: usize) {
        let unit = self.get_game().get_unit(position).expect(&format!("Missing unit at {:?}", position)).clone();
        let Some(hero) = unit.get_hero() else {
            return;
        };
        if hero.get_active_power() != index {
            self.add_event(Event::HeroPower(position, hero.get_active_power().into(), index.into()));
        }
    }

    pub fn player_dies(&mut self, owner_id: i8) {
        if self.get_game().get_owning_player(owner_id).map(|player| !player.dead).unwrap_or(false) {
            self.add_event(Event::PlayerDies(owner_id.into()));
            // TODO: trigger scripts?
            if self.get_game().get_living_teams().len() < 2 {
                self.add_event(Event::GameEnds);
            }
            if !self.get_game().has_ended() && self.get_game().current_player().dead {
                self.end_turn();
            }
        }
    }

    pub fn set_player_flag(&mut self, owner_id: i8, flag: usize) {
        if self.get_game().get_owning_player(owner_id).map(|p| !p.has_flag(flag)).unwrap_or(false) {
            self.add_event(Event::PlayerFlag(Owner(owner_id), FlagKey(flag)));
        }
    }
    pub fn remove_player_flag(&mut self, owner_id: i8, flag: usize) {
        if self.get_game().get_owning_player(owner_id).map(|p| p.has_flag(flag)).unwrap_or(false) {
            self.add_event(Event::PlayerFlag(Owner(owner_id), FlagKey(flag)));
        }
    }

    pub fn set_player_tag(&mut self, owner_id: i8, key: usize, value: TagValue<D>) {
        if !value.has_valid_type(self.environment(), key) {
            return;
        }
        match self.get_game().get_owning_player(owner_id).map(|p| p.get_tag(key)) {
            None => (), // player doesn't exist
            Some(None) => self.add_event(Event::PlayerSetTag(Owner(owner_id), TagKeyValues(TagKey(key), [value]))),
            Some(Some(old)) => {
                if old != value {
                    self.add_event(Event::PlayerReplaceTag(Owner(owner_id), TagKeyValues(TagKey(key), [old, value])));
                }
            }
        }
    }
    pub fn remove_player_tag(&mut self, owner_id: i8, key: usize) {
        if let Some(old) = self.get_game().get_owning_player(owner_id).and_then(|p| p.get_tag(key)) {
            self.add_event(Event::PlayerRemoveTag(Owner(owner_id), TagKeyValues(TagKey(key), [old])));
        }
    }

    pub fn terrain_replace(&mut self, position: Point, terrain: Terrain<D>) {
        let old_terrain = self.get_game().get_terrain(position).expect(&format!("Missing terrain at {:?}", position)).clone();
        self.add_event(Event::TerrainChange(position, old_terrain.clone(), terrain));
    }

    pub fn set_terrain_flag(&mut self, position: Point, flag: usize) {
        let terrain = self.get_game().get_terrain(position).expect(&format!("Missing terrain at {:?}", position)).clone();
        if !terrain.has_flag(flag) {
            self.add_event(Event::TerrainFlag(position, FlagKey(flag)));
        }
    }
    pub fn remove_terrain_flag(&mut self, position: Point, flag: usize) {
        let terrain = self.get_game().get_terrain(position).expect(&format!("Missing terrain at {:?}", position)).clone();
        if terrain.has_flag(flag) {
            self.add_event(Event::TerrainFlag(position, FlagKey(flag)));
        }
    }

    pub fn set_terrain_tag(&mut self, position: Point, key: usize, value: TagValue<D>) {
        let terrain = self.get_game().get_terrain(position).expect(&format!("Missing terrain at {:?}", position)).clone();
        if !value.has_valid_type(self.environment(), key) {
            return;
        }
        if let Some(old) = terrain.get_tag(key) {
            self.add_event(Event::TerrainReplaceTag(position, TagKeyValues(TagKey(key), [old, value])));
        } else {
            self.add_event(Event::TerrainSetTag(position, TagKeyValues(TagKey(key), [value])));
        }
    }
    pub fn remove_terrain_tag(&mut self, position: Point, key: usize) {
        let terrain = self.get_game().get_terrain(position).expect(&format!("Missing terrain at {:?}", position)).clone();
        if let Some(old) = terrain.get_tag(key) {
            self.add_event(Event::TerrainRemoveTag(position, TagKeyValues(TagKey(key), [old])));
        }
    }

    pub fn unit_creation(&mut self, position: Point, unit: Unit<D>) {
        self.add_event(Event::UnitAdd(position, unit.clone()));
        if let ClientPerspective::Team(team) = unit.get_team() {
            if self.get_game().has_secrets() && self.get_game().is_team_alive(team) {
                let heroes = HeroMap::new(self.get_board(), Some(unit.get_owner_id()));
                let changes = unit.get_vision(self.get_board(), position, &heroes).into_iter()
                .filter(|(p, intensity)| *intensity < self.get_game().get_fog_at(ClientPerspective::Team(team), *p))
                .collect();
                self.change_fog(ClientPerspective::Team(team), changes);
            }
        }
    }

    pub fn unit_add_transported(&mut self, position: Point, unit: Unit<D>) {
        self.add_event(Event::UnitAddBoarded(position, unit));
    }

    pub fn unit_path(&mut self, unload_index: Option<usize>, path: &Path<D>, board_at_the_end: bool, involuntarily: bool) {
        if path.steps.len() == 0 {
            return;
        }
        let mut unit = self.get_game().get_unit(path.start).expect(&format!("Missing unit at {:?}", path.start)).clone();
        if let Some(unload_index) = unload_index {
            if let Some(u) = unit.get_transported().get(unload_index) {
                self.add_event(Event::UnitRemoveBoarded(path.start, unload_index.into(), u.clone()));
                unit = u.clone();
            } else {
                panic!("Attempted to unboard unit that doesn't exist!");
            }
        } else {
            self.add_event(Event::UnitRemove(path.start, unit.clone()));
        }
        let unit_team = unit.get_team();
        let UnitId(unit_id, disto) = self.observe_unit(path.start, unload_index);
        let (effect, transformed_unit, vision_changes) = self.animate_unit_path(&unit, path, involuntarily);
        self.effect(effect);
        let (path_end, distortion) = path.end(&*self.get_game()).unwrap();
        if board_at_the_end {
            self.move_observed_unit(unit_id, path_end, Some(self.get_game().get_unit(path_end).unwrap().get_transported().len()), disto + distortion);
            self.add_event(Event::UnitAddBoarded(path_end, transformed_unit));
        } else {
            if self.get_game().get_unit(path_end).is_some() {
                // chess unit takes
                self.unit_death(path_end);
            }
            self.move_observed_unit(unit_id, path_end, None, disto + distortion);
            self.add_event(Event::UnitAdd(path_end, transformed_unit));
        }
        if self.get_game().has_secrets() {
            // provide vision along the unit's path
            if unit_team == self.get_game().current_team() {
                self.change_fog(unit_team, vision_changes);
            }
            // update fog in case unit influences other units' vision range
            self.recalculate_fog();
        }
        // remove tokens that were destroyed by the unit moving over them
        let mut token_scripts = Vec::new();
        for p in path.points(self.get_game()).unwrap() {
            for token in self.get_game().get_tokens(p) {
                if let Some(function_index) = self.environment().config.token_on_unit_path(token.typ()) {
                    token_scripts.push((function_index, p, token.clone()));
                }
            }
        }
        if token_scripts.len() > 0 {
            let environment = self.environment().clone();
            let mut scope = Scope::new();
            // TODO: information about the transporter the unit moved out of?
            scope.push_constant(CONST_NAME_PATH, path.clone());
            scope.push_constant(CONST_NAME_UNIT, unit);
            scope.push_constant(CONST_NAME_UNIT_ID, unit_id);
            let executor = self.executor(scope);
            for (function_index, p, token) in token_scripts {
                match executor.run::<D, ()>(function_index, (p, token)) {
                    Ok(()) => (),
                    Err(e) => {
                        environment.log_rhai_error("token OnUnitPath", environment.get_rhai_function_name(function_index), &e);
                    }
                }
            }
        }
    }

    pub fn animate_unit_path(&self, unit: &Unit<D>, path: &Path<D>, involuntarily: bool) -> (Effect<D>, Unit<D>, HashMap<Point, FogIntensity>) {
        let unit_team = unit.get_team();
        let owner_id = unit.get_owner_id();
        let heroes = HeroMap::new(self.get_board(), Some(owner_id));
        let mut current = path.start;
        let mut transformed_unit = unit.clone();
        //transformed_unit.set_en_passant(None);
        let mut steps = Vec::new();
        let mut vision_changes = HashMap::default();
        for (i, step) in path.steps.iter().enumerate() {
            if self.get_game().has_secrets() && !involuntarily && (i == 0 || transformed_unit.vision_mode().see_while_moving()) {
                //let heroes = heroes.with(self.get_board(), current, &transformed_unit);
                for (p, vision) in transformed_unit.get_vision(self.get_board(), current, &heroes) {
                    let vision = vision.min(vision_changes.remove(&p).unwrap_or(FogIntensity::Dark));
                    if vision < self.get_game().get_fog_at(unit_team, p) {
                        vision_changes.insert(p, vision);
                    }
                }
            }
            let (next, distortion) = step.progress(&*self.get_game(), current).unwrap();
            if !involuntarily && transformed_unit.transformed_by_movement(self.get_board(), current, next, distortion) {
                steps.push(EffectStep::Replace(current, *step, Some(EffectData::Unit(transformed_unit.clone()))));
            } else {
                steps.push(EffectStep::Simple(current, *step));
            }
            current = next;
        }
        if self.get_game().has_secrets() {
            //let heroes = heroes.with(self.get_board(), current, &transformed_unit);
            for (p, vision) in transformed_unit.get_vision(self.get_board(), current, &heroes) {
                let vision = vision.min(vision_changes.remove(&p).unwrap_or(FogIntensity::Dark));
                if vision < self.get_game().get_fog_at(unit_team, p) {
                    vision_changes.insert(p, vision);
                }
            }
        }
        //self.add_event(Event::Effect(Effect::new_unit_path(unit.clone(), steps)));
        (Effect::new_unit_path(unit.clone(), steps), transformed_unit, vision_changes)
    }

    pub fn on_unit_normal_action(&mut self, id: usize, path: Path<D>, interrupted: bool, heroes: &HeroMap<D>, ballast: &[TBallast<D>]) {
        let Some((p, unload_index)) = self.get_observed_unit_pos(id) else {
            return;
        };
        let unit = {
            let mut u = self.get_game().get_unit(p).expect(&format!("Missing unit at {p:?}"));
            if let Some(i) = unload_index {
                u = u.get_transported().get(i).expect(&format!("Missing unit at {p:?}, index {i}"));
            }
            u.clone()
        };
        let transporter = self.get_game().get_unit(path.start);
        let destination_unit = unload_index.and_then(|_| self.get_game().get_unit(p));
        let environment = self.environment().clone();
        let scripts = environment.config.unit_normal_action_effects(
            self.get_board(),
            &unit,
            (p, unload_index),
            transporter.as_ref().map(|t| (*t, path.start)),
            destination_unit.as_ref().map(|u| UnitData {
                unit: u,
                pos: p,
                unload_index: None,
                ballast: &[],
                original_transporter: None, // no recursive transportation
            }),
            heroes,
            ballast,
        );
        if scripts.len() == 0 {
            return;
        }
        let mut scope = Scope::new();
        scope.push_constant(CONST_NAME_TRANSPORTER_POSITION, transporter.map(|_| Dynamic::from(path.start)).unwrap_or(().into()));
        scope.push_constant(CONST_NAME_TRANSPORTER, transporter.map(|u| Dynamic::from(u.clone())).unwrap_or(().into()));
        //scope.push_constant(CONST_NAME_TRANSPORT_INDEX, transport_index.map(|i| Dynamic::from(i as i32)).unwrap_or(().into()));
        scope.push_constant(CONST_NAME_PATH, path);
        scope.push_constant(CONST_NAME_POSITION, p);
        scope.push_constant(CONST_NAME_UNIT, unit);
        scope.push_constant(CONST_NAME_UNIT_ID, UnitId(id, self.get_observed_unit(id).unwrap().2));
        scope.push_constant(CONST_NAME_INTERRUPTED, interrupted);
        let executor = self.executor(scope);
        for function_index in scripts {
            match executor.run::<D, ()>(function_index, ()) {
                Ok(()) => (),
                Err(e) => {
                    environment.log_rhai_error("unit OnNormalAction", environment.get_rhai_function_name(function_index), &e);
                }
            }
        }
    }

    pub fn set_unit_flag(&mut self, position: Point, flag: usize) {
        let unit = self.get_game().get_unit(position).expect(&format!("Missing unit at {:?}", position)).clone();
        if !unit.has_flag(flag) {
            self.add_event(Event::UnitFlag(position, FlagKey(flag)));
        }
    }
    pub fn remove_unit_flag(&mut self, position: Point, flag: usize) {
        let unit = self.get_game().get_unit(position).expect(&format!("Missing unit at {:?}", position)).clone();
        if unit.has_flag(flag) {
            self.add_event(Event::UnitFlag(position, FlagKey(flag)));
        }
    }

    pub fn set_unit_tag(&mut self, position: Point, key: usize, value: TagValue<D>) {
        if !value.has_valid_type(self.environment(), key) {
            return;
        }
        let unit = self.get_game().get_unit(position).expect(&format!("Missing unit at {:?}", position)).clone();
        if let Some(old) = unit.get_tag(key) {
            self.add_event(Event::UnitReplaceTag(position, TagKeyValues(TagKey(key), [old, value])));
        } else {
            self.add_event(Event::UnitSetTag(position, TagKeyValues(TagKey(key), [value])));
        }
    }
    pub fn remove_unit_tag(&mut self, position: Point, key: usize) {
        let unit = self.get_game().get_unit(position).expect(&format!("Missing unit at {:?}", position)).clone();
        if let Some(old) = unit.get_tag(key) {
            self.add_event(Event::UnitRemoveTag(position, TagKeyValues(TagKey(key), [old])));
        }
    }

    pub fn set_unit_flag_boarded(&mut self, position: Point, unload_index: usize, flag: usize) {
        let unit = self.get_game().get_unit(position).expect(&format!("Missing unit at {:?}", position)).clone();
        if !unit.get_transported()[unload_index].has_flag(flag) {
            self.add_event(Event::UnitFlagBoarded(position, unload_index.into(), FlagKey(flag)));
        }
    }
    pub fn remove_unit_flag_boarded(&mut self, position: Point, unload_index: usize, flag: usize) {
        let unit = self.get_game().get_unit(position).expect(&format!("Missing unit at {:?}", position)).clone();
        if unit.get_transported()[unload_index].has_flag(flag) {
            self.add_event(Event::UnitFlagBoarded(position, unload_index.into(), FlagKey(flag)));
        }
    }

    pub fn set_unit_tag_boarded(&mut self, position: Point, unload_index: usize, key: usize, value: TagValue<D>) {
        if !value.has_valid_type(self.environment(), key) {
            return;
        }
        let unit = self.get_game().get_unit(position).expect(&format!("Missing unit at {:?}", position)).clone();
        if let Some(old) = unit.get_transported()[unload_index].get_tag(key) {
            self.add_event(Event::UnitReplaceTagBoarded(position, unload_index.into(), TagKeyValues(TagKey(key), [old, value])));
        } else {
            self.add_event(Event::UnitSetTagBoarded(position, unload_index.into(), TagKeyValues(TagKey(key), [value])));
        }
    }
    pub fn remove_unit_tag_boarded(&mut self, position: Point, unload_index: usize, key: usize) {
        let unit = self.get_game().get_unit(position).expect(&format!("Missing unit at {:?}", position)).clone();
        if let Some(old) = unit.get_transported()[unload_index].get_tag(key) {
            self.add_event(Event::UnitRemoveTagBoarded(position, unload_index.into(), TagKeyValues(TagKey(key), [old])));
        }
    }

    pub fn unit_remove(&mut self, position: Point) {
        let unit = self.get_game().get_unit(position).expect(&format!("Missing unit at {:?}", position)).clone();
        self.remove_observed_units_at(position);
        self.add_event(Event::UnitRemove(position, unit));
    }

    fn unit_death(&mut self, position: Point) {
        let unit = self.get_game().get_unit(position).expect(&format!("Missing unit at {:?}", position)).clone();
        self.remove_observed_units_at(position);
        self.add_event(Event::UnitRemove(position, unit));
    }

    pub fn unit_mass_death(&mut self, positions: &HashSet<Point>) {
        for position in positions {
            self.unit_death(*position);
        }
    }

    pub fn unit_replace(&mut self, position: Point, new_unit: Unit<D>) {
        let unit = self.get_game().get_unit(position).expect(&format!("Missing unit at {:?}", position)).clone();
        self.add_event(Event::UnitRemove(position, unit.clone()));
        self.add_event(Event::UnitAdd(position, new_unit));
    }


    pub fn trigger_all_terrain_scripts(
        &mut self,
        get_script: impl Fn(&Board<D>, Point, &Terrain<D>, &[HeroInfluence<D>]) -> Vec<usize>,
        before_executing: impl FnOnce(&mut Self),
        execute_script: impl Fn(&mut Self, Vec<usize>, Point, Terrain<D>),
    ) {
        let hero_auras = HeroMap::new(self.get_board(), None);
        let mut scripts = Vec::new();
        for p in valid_points(self.get_game()) {
            let terrain = self.get_game().get_terrain(p).unwrap();
            let heroes = hero_auras.get(p, terrain.get_owner_id());
            let script = get_script(self.get_board(), p, terrain, heroes);
            if script.len() > 0 {
                scripts.push((script, p, terrain.clone()));
            }
        }
        before_executing(self);
        for (scripts, p, terrain) in scripts {
            // the unit may not be at unit_pos anymore
            execute_script(self, scripts, p, terrain);
        }
    }

    pub fn trigger_all_unit_scripts(
        &mut self,
        get_script: impl Fn(&Board<D>, &Unit<D>, Point, Option<(&Unit<D>, usize)>, &HeroMap<D>) -> Vec<usize>,
        before_executing: impl FnOnce(&mut Self),
        execute_script: impl Fn(&mut Self, Vec<usize>, Point, &Unit<D>, usize),
    ) {
        let heroes = HeroMap::new(self.get_board(), None);
        let mut scripts = Vec::new();
        for p in valid_points(self.get_game()) {
            if let Some(unit) = self.get_game().get_unit(p).cloned() {
                let script = get_script(self.get_board(), &unit, p, None, &heroes);
                if script.len() > 0 {
                    let id = self.observe_unit(p, None).0;
                    scripts.push((script, unit.clone(), p, id));
                }
                for (i, u) in unit.get_transported().iter().enumerate() {
                    let script = get_script(self.get_board(), u, p, Some((&unit, i)), &heroes);
                    if script.len() > 0 {
                        let id = self.observe_unit(p, Some(i)).0;
                        scripts.push((script, u.clone(), p, id));
                    }
                }
            }
        }
        before_executing(self);
        for (scripts, unit, unit_pos, observation_id) in scripts {
            // the unit may not be at unit_pos anymore
            execute_script(self, scripts, unit_pos, &unit, observation_id);
        }
    }

    pub fn trigger_all_global_events(
        &mut self,
        get_script: impl Fn(&GlobalEventConfig) -> Option<usize>,
    ) {
        let hero_auras = HeroMap::new(self.get_board(), None);
        let all_points = valid_points(self.get_game());
        let environment = self.environment().clone();
        for (i, conf) in environment.config.global_events.iter().enumerate() {
            let Some(script) = get_script(conf) else {
                continue;
            };
            let mut scripts = Vec::new();
            {
                // commander scripts
                if let Some(scope) = conf.typ.test_global(&*self.get_game()) {
                    scripts.push((script, scope))
                } else {
                    // terrain, token, unit scripts
                    for p in all_points.iter().cloned() {
                        for scope in conf.typ.test_local(self, p, &hero_auras) {
                            scripts.push((script, scope));
                        }
                    }
                }
            }
            for (function_index, scope) in scripts {
                let executor = self.executor(scope);
                match executor.run::<D, ()>(function_index, ()) {
                    Ok(()) => (),
                    Err(e) => {
                        let environment = self.environment();
                        environment.log_rhai_error(&format!("global_event #{i}"), environment.get_rhai_function_name(function_index), &e);
                    }
                }
            }
        }
    }

    pub fn accept(mut self) -> EventsMap<D> {
        if self.events.get(&IPerspective::Server) == self.events.get(&IPerspective::Neutral) {
            // if no info is hidden, there's no need to store multiple identical entries
            let events = self.events.remove(&IPerspective::Server).unwrap();
            EventsMap::Public(events)
        } else {
            EventsMap::Secrets(self.events)
        }
    }

    pub fn cancel(mut self) {
        while let Some(event) = self.events.get_mut(&IPerspective::Server).unwrap().pop() {
            event.undo(self.game);
        }
    }

    pub fn executor<'b>(&'b mut self, mut scope: Scope<'b>) -> Executor<'b> {
        scope.push_constant(CONST_NAME_EVENT_HANDLER, EventHandlerPointer::from(self));
        self.board.executor(scope)
    }
}

/// Newtype wrapping a reference (pointer) cast into 'usize'
#[derive(Debug, Eq, PartialEq, Clone, Hash)]
pub(crate) struct EventHandlerPointer<D: Direction> {
    ptr: usize,
    _pd: PhantomData<D>,
}

impl<D: Direction> EventHandlerPointer<D> {
    fn from(value: *mut EventHandler<D>) -> Self {
        let ptr = value.expose_provenance();
        Self {
            ptr,
            _pd: PhantomData,
        }
    }

    pub(crate) fn as_mut<'a>(&'a mut self) -> &'a mut EventHandler<'a, D> {
        let ptr: *mut EventHandler<'a, D> = with_exposed_provenance_mut(self.ptr);
        unsafe {ptr.as_mut()}
            .unwrap()
    }
}
