
use std::sync::{Arc, MappedRwLockReadGuard, RwLock, RwLockReadGuard};

use interfaces::{ClientPerspective, Perspective as IPerspective, RandomFn};
use rhai::{Dynamic, Scope};
use rustc_hash::{FxHashMap as HashMap, FxHashSet as HashSet};

use crate::config::environment::Environment;
use crate::handle::Handle;
use crate::map::map::Map;
use crate::map::point::Point;
use crate::map::wrapping_map::Distortion;
use crate::script::custom_action::execute_commander_script;
use crate::script::executor::Executor;
use crate::script::*;
use crate::tags::*;
use crate::terrain::terrain::*;
use crate::units::combat::WeaponType;
use crate::player::*;
use crate::tokens::token::Token;
use crate::map::direction::Direction;
use crate::game::game::*;
use crate::game::fog::*;
use crate::units::hero::{Hero, HeroInfluence};
use crate::units::movement::{Path, TBallast};
use crate::units::unit::Unit;
use super::events::{Event, Effect, UnitStep};
use super::game_view::GameView;

struct EventHandlerInner<D: Direction> {
    game: Handle<Game<D>>,
    events: HashMap<IPerspective, Vec<Event<D>>>,
    random: RandomFn,
    observed_units: HashMap<usize, (Point, Option<usize>, Distortion<D>)>,
    next_observed_unit_id: usize,
}

impl<D: Direction> EventHandlerInner<D> {
    fn new(game: Handle<Game<D>>, random: RandomFn) -> Self {
        let mut events = HashMap::default();
        events.insert(IPerspective::Server, Vec::new());
        events.insert(IPerspective::Neutral, Vec::new());
        for team in game.with(|game| game.get_teams()) {
            events.insert(IPerspective::Team(team), Vec::new());
        }
        Self {
            game,
            events,
            random,
            next_observed_unit_id: 0,
            observed_units: HashMap::default(),
        }
    }

    fn add_event(&mut self, event: Event<D>) {
        self.game.with_mut(|game| event.apply(game));
        for (key, events) in self.events.iter_mut() {
            if let Ok(perspective) = key.try_into() {
                if let Some(event) = event.fog_replacement(&self.game, perspective) {
                    events.push(event);
                }
            }
        }
        self.events.get_mut(&IPerspective::Server).unwrap().push(event);
    }

    fn observe_unit(&mut self, position: Point, unload_index: Option<usize>) -> (usize, Distortion<D>) {
        if let Some((id, (_, _, distortion))) = self.observed_units.iter()
        .find(|(_, (p, i, _))| *p == position && *i == unload_index) {
            (*id, *distortion)
        } else {
            self.observed_units.insert(self.next_observed_unit_id, (position, unload_index, Distortion::neutral()));
            self.next_observed_unit_id += 1;
            (self.next_observed_unit_id - 1, Distortion::neutral())
        }
    }

    fn get_observed_unit(&self, id: usize) -> Option<(Point, Option<usize>, Distortion<D>)> {
        self.observed_units.get(&id).cloned()
    }

    fn get_observed_unit_pos(&self, id: usize) -> Option<(Point, Option<usize>)> {
        self.observed_units.get(&id)
        .map(|(p, unload_index, _)| (*p, *unload_index))
    }

    fn observation_id(&self, position: Point, unload_index: Option<usize>) -> Option<(usize, Distortion<D>)> {
        self.observed_units.iter()
        .find(|(_, (p, i, _))| *p == position && *i == unload_index)
        .map(|(id, (_, _, distortion))| (*id, *distortion))
    }

    fn remove_observed_units_at(&mut self, position: Point) {
        if let Some((id, _)) = self.observation_id(position, None) {
            self.observed_units.remove(&id);
        }
        for i in 0..self.game.environment().config.max_transported() {
            if let Some((id, _)) = self.observation_id(position, Some(i)) {
                self.observed_units.remove(&id);
            }
        }
    }

    fn remove_observed_unit(&mut self, id: usize) {
        self.observed_units.remove(&id);
    }

    fn move_observed_unit(&mut self, id: usize, p: Point, unload_index: Option<usize>, distortion: Distortion<D>) {
        self.observed_units.insert(id, (p, unload_index, distortion));
    }

    fn accept(mut self) -> EventsMap<D> {
        if self.events.get(&IPerspective::Server) == self.events.get(&IPerspective::Neutral) {
            // if no info is hidden, there's no need to store multiple identical entries
            let events = self.events.remove(&IPerspective::Server).unwrap();
            EventsMap::Public(events)
        } else {
            EventsMap::Secrets(self.events)
        }
    }

    fn cancel(mut self) {
        self.game.with_mut(|game| {
            while let Some(event) = self.events.get_mut(&IPerspective::Server).unwrap().pop() {
                event.undo(game);
            }
        })
    }
}

#[derive(Clone)]
pub struct EventHandler<D: Direction> {
    inner: Arc<RwLock<EventHandlerInner<D>>>,
}

impl<D: Direction> EventHandler<D> {
    pub fn new(game: Handle<Game<D>>, random: RandomFn) -> Self {
        Self {
            inner: Arc::new(RwLock::new(EventHandlerInner::new(game, random))),
        }
    }

    fn with<R>(&self, f: impl FnOnce(&EventHandlerInner<D>) -> R) -> R {
        let t = self.inner.read().expect("Unable to read EventHandler");
        f(&*t)
    }

    fn with_mut<R>(&self, f: impl FnOnce(&mut EventHandlerInner<D>) -> R) -> R {
        let mut t = self.inner.write().expect("Unable to write EventHandler");
        f(&mut *t)
    }

    fn borrow<'a>(&'a self) -> RwLockReadGuard<'a, EventHandlerInner<D>> {
        self.inner.read().expect("Unable to borrow EventHandler")
    }

    pub fn get_game<'a>(&'a self) -> MappedRwLockReadGuard<'a, Handle<Game<D>>> {
        RwLockReadGuard::map(self.borrow(), |eh| &eh.game)
    }

    pub fn with_map<R>(&self, f: impl FnOnce(&Map<D>) -> R) -> R {
        self.with(|eh| {
            eh.game.with(|g| {
                f(&g.get_map())
            })
        })
    }

    pub fn with_game<R>(&self, f: impl FnOnce(&Game<D>) -> R) -> R {
        self.with(|eh| {
            eh.game.with(|g| {
                f(g)
            })
        })
    }

    pub fn environment(&self) -> Environment {
        self.with(|eh| {
            eh.game.environment()
        })
    }

    pub fn observe_unit(&mut self, position: Point, unload_index: Option<usize>) -> (usize, Distortion<D>) {
        self.with_mut(|eh| eh.observe_unit(position, unload_index))
    }

    pub fn get_observed_unit(&self, id: usize) -> Option<(Point, Option<usize>, Distortion<D>)> {
        self.with(|eh| eh.get_observed_unit(id))
    }

    pub fn get_observed_unit_pos(&self, id: usize) -> Option<(Point, Option<usize>)> {
        self.with(|eh| eh.get_observed_unit_pos(id))
    }

    fn observation_id(&self, position: Point, unload_index: Option<usize>) -> Option<(usize, Distortion<D>)> {
        self.with(|eh| eh.observation_id(position, unload_index))
    }

    fn remove_observed_units_at(&mut self, position: Point) {
        self.with_mut(|eh| eh.remove_observed_units_at(position))
    }

    fn remove_observed_unit(&mut self, id: usize) {
        self.with_mut(|eh| eh.remove_observed_unit(id))
    }

    fn move_observed_unit(&mut self, id: usize, p: Point, unload_index: Option<usize>, distortion: Distortion<D>) {
        self.with_mut(|eh| eh.move_observed_unit(id, p, unload_index, distortion))
    }
    
    pub fn end_turn(&mut self) {
        // un-exhaust units
        /*for p in self.with_map(|map| map.all_points()) {
            if let Some(unit) = self.with_map(|map| map.get_unit(p).cloned()) {
                if unit.get_owner_id() == owner_id {
                    match unit.get_status() {
                        ActionStatus::Exhausted => self.unit_status(p, ActionStatus::Ready),
                        _ => (),
                    }
                    for (index, u) in unit.get_transported().iter().enumerate() {
                        if u.is_exhausted() {
                            self.unit_status_boarded(p, index, ActionStatus::Ready);
                        }
                    }
                }
            }
        }*/

        // unit end turn event
        self.trigger_all_unit_scripts(
            |game, unit, unit_pos, transporter, heroes| {
                unit.on_end_turn(game, unit_pos, transporter, heroes)
            },
            |_observation_id| {},
            |handler, scripts, unit_pos, unit, _observation_id| {
                if scripts.len() > 0 {
                    let mut scope = Scope::new();
                    scope.push_constant(CONST_NAME_POSITION, unit_pos);
                    scope.push_constant(CONST_NAME_UNIT, unit.clone());
                    let environment = handler.get_game().environment();
                    let engine = environment.get_engine_handler(&handler);
                    let executor = Executor::new(engine, scope, environment);
                    for function_index in scripts {
                        match executor.run(function_index, ()) {
                            Ok(()) => (),
                            Err(e) => {
                                // TODO: log error
                                println!("unit OnEndTurn {function_index}: {e:?}");
                            }
                        }
                    }
                }
            }
        );

        // reset built_this_turn-counter for realties
        /*for p in self.with_map(|map| map.all_points()) {
            self.terrain_built_this_turn(p, 0);
        }*/

        let fog_before = if self.get_game().is_foggy() {
            let team = self.with_game(|game| {
                let next_player = game.players.get((game.current_turn() + 1) % game.players.len()).unwrap();
                next_player.get_team()
            });
            Some(recalculate_fog(&*self.get_game(), team))
        } else {
            None
        };

        self.next_turn();
        let owner_id = self.get_game().current_owner();

        // reset status for repairing units
        /*for p in self.with_map(|map| map.all_points()) {
            if self.with_map(|map| map.get_unit(p)
            .filter(|u| u.get_owner_id() == owner_id && u.get_status() == ActionStatus::Repairing)
            .is_some()) {
                self.unit_status(p, ActionStatus::Ready);
            }
        }

        // reset capture-progress / finish capturing
        for p in self.with_map(|map| map.all_points()) {
            let terrain = self.with_map(|map| map.get_terrain(p).unwrap().clone());
            if let Some((new_owner, progress)) = terrain.get_capture_progress() {
                if new_owner.0 == owner_id {
                    if let Some(unit) = self.with_map(|map| map.get_unit(p)
                    .filter(|u| u.get_owner_id() == owner_id && u.can_capture()).cloned()) {
                        if unit.get_status() == ActionStatus::Capturing {
                            let max_progress = terrain.get_capture_resistance();
                            let progress = progress as u16 + (unit.get_hp() as f32 / 10.).ceil() as u16;
                            if progress < max_progress as u16 {
                                self.terrain_capture_progress(p, Some((new_owner, (progress as u8).into())));
                            } else {
                                // captured
                                let terrain = TerrainBuilder::new(&self.environment(), terrain.typ())
                                .copy_from(&terrain)
                                .set_capture_progress(None)
                                .set_owner_id(new_owner.0)
                                .build_with_defaults();
                                self.terrain_replace(p, terrain);
                            }
                        }
                    } else {
                        self.terrain_capture_progress(p, None);
                    }
                }
            }
            if self.with_map(|map| map.get_unit(p)
            .filter(|u| u.get_owner_id() == owner_id && u.get_status() == ActionStatus::Capturing)
            .is_some()) {
                self.unit_status(p, ActionStatus::Ready);
            }
        }*/

        if let Some((power_index, function_index, charge_cost)) = self.with_game(|game| {
            let commander = &game.current_player().commander;
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
        }) {
            self.commander_charge_sub(owner_id, charge_cost);
            self.commander_power(owner_id, power_index);
            if let Some(function_index) = function_index {
                execute_commander_script(function_index, self, None);
            }
        }

        // end merc powers
        for p in self.with_map(|map| map.all_points()) {
            if let Some(hero) = self.with_map(|map| map.get_unit(p).filter(|u| u.get_owner_id() == owner_id).and_then(|u| u.get_hero()).cloned()) {
                let next_power = hero.get_next_power(&self.environment());
                if hero.can_activate_power(&self.environment(), next_power, true) {
                    // TODO: this skips the custom-action. maybe execute the custom action if no user input is needed
                    self.hero_charge_sub(p, None, hero.power_cost(&self.environment(), next_power));
                    self.hero_power(p, next_power);
                }
            }
        }

        self.start_turn(fog_before);

        if self.with_game(|game| !game.has_ended() && game.current_player().dead) {
            self.end_turn();
        }
    }

    pub fn next_turn(&mut self) {
        self.add_event(Event::NextTurn);
    }

    pub fn start_turn(&mut self, fog_before: Option<HashMap<Point, FogIntensity>>) {
        // hide / reveal player funds if fog started / ended
        let was_foggy = fog_before.is_some();
        if was_foggy != self.get_game().is_foggy() {
            let player_ids: Vec<i8> = self.with_game(|game| game.players.iter().map(|player| player.get_owner_id()).collect());
            if was_foggy {
                for player_id in player_ids {
                    self.add_event(Event::PureRevealFunds(player_id.into()));
                }
            } else {
                for player_id in player_ids {
                    self.add_event(Event::PureHideFunds(player_id.into()));
                }
            }
        }

        let owner_id = self.get_game().current_owner();
        // return drones to their origin if possible or destroy them
        /*let mut drone_parents: HashMap<u16, (Point, usize)> = self.with_map(|map| map.all_points())
        .into_iter()
        .filter_map(|p| self.with_map(|map| map.get_unit(p).and_then(|u| Some((p, u.clone())))))
        .filter(|(_, u)| u.get_owner_id() == owner_id)
        .filter_map(|(p, unit)| {
            if let Some(drone_id) = unit.get_drone_station_id() {
                Some((drone_id, (p, unit.remaining_transport_capacity())))
            } else {
                None
            }
        }).collect();
        let mut dead_drones = HashSet::default();
        for p in self.with_map(|map| map.all_points()) {
            if let Some(unit) = self.with_map(|map| map.get_unit(p).filter(|u| u.get_owner_id() == owner_id).cloned()) {
                if let Some(drone_id) = unit.get_drone_id() {
                    if let Some((destination, capacity)) = drone_parents.get_mut(&drone_id) {
                        // move drone back aboard its parent
                        if let Some((id, distortion)) = self.observation_id(p, None) {
                            self.move_observed_unit(id, *destination, Some(self.with_map(|map| map.get_unit(*destination).unwrap().get_transported().len())), distortion);
                        }
                        let mut u = unit.clone();
                        self.add_event(Event::UnitRemove(p, u.clone()));
                        u.set_en_passant(None);
                        self.add_event(Event::UnitAddBoarded(*destination, u));
                        // one less space in parent
                        *capacity -= 1;
                        if *capacity == 0 {
                            drone_parents.remove(&drone_id);
                        }
                    } else {
                        // no parent available, self-destruct
                        // should this even trigger on_death effects?
                        // yes, but add some DeathCause enum to the script filter so the script-writer can filter this one away
                        dead_drones.insert(p);
                    }
                } else if unit.has_attribute(AttributeKey::EnPassant) {
                    // for drones en-passant is removed before boarding its station instead
                    self.unit_en_passant_opportunity(p, None);
                }
            }
        }*/

        /*self.trigger_all_unit_scripts(
            |game, unit, unit_pos, transporter, heroes| {
                if dead_drones.contains(&unit_pos) {
                    unit.on_death(game, unit_pos, transporter, None, heroes, &[])
                } else {
                    Vec::new()
                }
            },
            |handler| handler.unit_mass_death(&dead_drones),
            |handler, scripts, unit_pos, unit, _observation_id| {
                if scripts.len() > 0 {
                    let mut scope = Scope::new();
                    scope.push_constant(CONST_NAME_POSITION, unit_pos);
                    scope.push_constant(CONST_NAME_UNIT, unit.clone());
                    let environment = handler.get_game().environment();
                    let engine = environment.get_engine_handler(handler);
                    let executor = Executor::new(engine, scope, environment);
                    for function_index in scripts {
                        match executor.run(function_index, ()) {
                            Ok(()) => (),
                            Err(e) => {
                                // TODO: log error
                                println!("unit OnDeath {function_index}: {e:?}");
                            }
                        }
                    }
                }
            }
        );*/

        self.trigger_all_terrain_scripts(
            |game, p, terrain, heroes| {
                terrain.on_start_turn(game, p, heroes)
            },
            |_| {},
            |handler, scripts, p, terrain| {
                if scripts.len() > 0 {
                    let mut scope = Scope::new();
                    scope.push_constant(CONST_NAME_POSITION, p);
                    scope.push_constant(CONST_NAME_TERRAIN, terrain);
                    scope.push_constant(CONST_NAME_OWNER_ID, owner_id as i32);
                    let environment = handler.get_game().environment();
                    let engine = environment.get_engine_handler(handler);
                    let executor = Executor::new(engine, scope, environment);
                    for function_index in scripts {
                        match executor.run(function_index, ()) {
                            Ok(()) => (),
                            Err(e) => {
                                // TODO: log error
                                println!("terrain OnStartTurn {function_index}: {e:?}");
                            }
                        }
                    }
                }
            }
        );

        // has to be recalculated before structures, because the effects of some structures on
        // other players should maybe not be visible
        //self.recalculate_fog(false);

        let income = self.with_game(|game| game.current_player().get_income()) * self.with_map(|map| map.get_income_factor(owner_id));
        if income != 0 {
            self.money_income(owner_id, income);
        }

        // unit start turn event
        self.trigger_all_unit_scripts(
            |game, unit, unit_pos, transporter, heroes| {
                if unit.get_owner_id() == owner_id {
                    unit.on_start_turn(game, unit_pos, transporter, heroes)
                } else {
                    Vec::new()
                }
            },
            |_| {},
            |handler, scripts, unit_pos, unit, _observation_id| {
                if scripts.len() > 0 {
                    let mut scope = Scope::new();
                    scope.push_constant(CONST_NAME_POSITION, unit_pos);
                    scope.push_constant(CONST_NAME_UNIT, unit.clone());
                    scope.push_constant(CONST_NAME_OWNER_ID, owner_id as i32);
                    let environment = handler.get_game().environment();
                    let engine = environment.get_engine_handler(handler);
                    let executor = Executor::new(engine, scope, environment);
                    for function_index in scripts {
                        match executor.run(function_index, ()) {
                            Ok(()) => (),
                            Err(e) => {
                                // TODO: log error
                                println!("unit OnStartTurn {function_index}: {e:?}");
                            }
                        }
                    }
                }
            }
        );

        /*// tick sludge tokens
        for p in self.with_map(|map| map.all_points()) {
            for (index, d) in self.with_map(|map| map.get_tokens(p).to_vec().into_iter().enumerate()) {
                match d {
                    Token::SludgeToken(token) => {
                        if token.get_owner_id() == owner_id {
                            let counter = token.get_counter();
                            self.token_remove(p, index);
                            if counter > 0 {
                                self.token_add(p, Token::SludgeToken(SludgeToken::new(&self.environment().config, owner_id, counter - 1)));
                            }
                        }
                        break;
                    },
                    _ => ()
                }
            }
        }*/

        // structures may have destroyed some units, vision may be reduced due to merc powers ending
        self.recalculate_fog();
    }

    pub fn recalculate_fog(&mut self) {
        let current_team = self.get_game().current_team();
        // only remove fog for the current team
        let mut fog = recalculate_fog(&*self.get_game(), current_team);
        for (p, intensity) in fog.iter_mut() {
            *intensity = self.get_game().get_fog_at(current_team, *p).min(*intensity);
        }
        self.change_fog(current_team, fog);
        // reset fog for other teams
        let mut perspectives: HashSet<ClientPerspective> = self.with_game(|game| game.get_teams()).into_iter()
        .filter(|team| ClientPerspective::Team(*team) != current_team)
        .map(|team| ClientPerspective::Team(team))
        .collect();
        perspectives.insert(ClientPerspective::Neutral);
        for team in perspectives {
            self.recalculate_fog_for(team);
        }
    }

    pub fn recalculate_fog_for(&mut self, team: ClientPerspective) {
        let fog = recalculate_fog(&*self.get_game(), team);
        self.change_fog(team, fog);
    }

    pub fn rng(&self) -> f32 {
        self.with(|eh| (*eh.random)())
    }

    fn add_event(&mut self, event: Event<D>) {
        self.with_mut(|eh| eh.add_event(event));
    }

    pub fn change_fog(&mut self, team: ClientPerspective, changes: HashMap<Point, FogIntensity>) {
        let changes: Vec<(Point, FogIntensity, FogIntensity)> = changes.into_iter()
        .map(|(p, intensity)| (p, self.with_game(|game| game.get_fog_at(team, p)), intensity))
        .filter(|(_, before, after)| before != after)
        .collect();
        if changes.len() > 0 {
            self.add_event(Event::PureFogChange(from_client_perspective(team).into(), changes.try_into().unwrap()));
        }
    }

    pub fn commander_charge_add(&mut self, owner: i8, change: u32) {
        if let Some(player) = self.with_game(|game| game.get_owning_player(owner).cloned()) {
            if !player.commander.can_gain_charge() {
                return;
            }
            let change = change.min(player.commander.get_max_charge() - player.commander.get_charge()) as i32;
            if change > 0 {
                self.add_event(Event::CommanderCharge(owner.into(), change.into()));
            }
        }
    }

    pub fn commander_charge_sub(&mut self, owner: i8, change: u32) {
        if let Some(player) = self.with_game(|game| game.get_owning_player(owner).cloned()) {
            let change = -(change as i32).min(player.commander.get_charge() as i32);
            if change < 0 {
                self.add_event(Event::CommanderCharge(owner.into(), change.into()));
            }
        }
    }

    pub fn commander_power(&mut self, owner: i8, index: usize) {
        if let Some(player) = self.with_game(|game| game.get_owning_player(owner).cloned()) {
            if player.commander.get_active_power() != index {
                self.add_event(Event::CommanderPowerIndex(owner.into(), player.commander.get_active_power().into(), index.into()));
            }
        }
    }

    pub fn token_add(&mut self, position: Point, token: Token<D>) {
        let old_tokens = self.with_map(|map| map.get_tokens(position).to_vec());
        let mut tokens = old_tokens.to_vec();
        tokens.push(token);
        if old_tokens != tokens.as_slice() {
            self.add_event(Event::ReplaceToken(position, old_tokens.try_into().unwrap(), Token::correct_stack(tokens).try_into().unwrap()));
        }
    }

    pub fn token_remove(&mut self, position: Point, index: usize) {
        if let Some(token) = self.with_map(|map| map.get_tokens(position).get(index).cloned()) {
            self.add_event(Event::RemoveToken(position, index.into(), token));
        } else {
            panic!("Missing Token at {position:?}");
        }
    }

    pub fn effect_glitch(&mut self) {
        // TODO
    }

    pub fn effect_fog_surprise(&mut self, position: Point) {
        let team = match self.get_game().current_team() {
            ClientPerspective::Team(team) => team,
            ClientPerspective::Neutral => return, // shouldn't happen
        };
        self.add_event(Event::Effect(Effect::Surprise(position, team.into())));
    }

    fn effect_heal(&mut self, _position: Point) {
        // TODO: add effect
        //self.add_event(Event::Effect(Effect::Repair(position)));
    }

    fn effect_repair(&mut self, position: Point) {
        self.add_event(Event::Effect(Effect::Repair(position)));
    }

    pub fn effect_weapon(&mut self, position: Point, weapon: WeaponType) {
        self.add_event(Event::Effect(weapon.effect(position)));
    }

    pub fn effect_chess(&mut self, _position: Point) {
        // TODO: add effect for taking units
    }

    pub fn unit_set_hero(&mut self, position: Point, hero: Hero) {
        let unit = self.with_map(|map| map.get_unit(position).expect(&format!("Missing unit at {:?}", position)).clone());
        if !unit.is_hero() {
            self.add_event(Event::HeroSet(position, hero));
        }
    }

    pub fn hero_charge_add(&mut self, position: Point, unload_index: Option<usize>, change: u8) {
        self.hero_charge(position, unload_index, change as i8)
    }

    pub fn hero_charge_sub(&mut self, position: Point, unload_index: Option<usize>, change: u8) {
        self.hero_charge(position, unload_index, -(change as i8))
    }

    fn hero_charge(&mut self, position: Point, unload_index: Option<usize>, change: i8) {
        let unit = self.with_map(|map| map.get_unit(position).expect(&format!("Missing unit at {:?}", position)).clone());
        let Some(hero) = unit.get_hero() else {
            return;
        };
        let change = change.max(-(hero.get_charge() as i8)).min((hero.typ().max_charge(&self.environment()) - hero.get_charge()) as i8);
        if change != 0 {
            if let Some(unload_index) = unload_index {
                self.add_event(Event::HeroChargeTransported(position, unload_index.into(), change.into()));
            } else {
                self.add_event(Event::HeroCharge(position, change.into()));
            }
        }
    }

    pub fn hero_power(&mut self, position: Point, index: usize) {
        let unit = self.with_map(|map| map.get_unit(position).expect(&format!("Missing unit at {:?}", position)).clone());
        let Some(hero) = unit.get_hero() else {
            return;
        };
        if hero.get_active_power() != index {
            self.add_event(Event::HeroPower(position, hero.get_active_power().into(), index.into()));
        }
    }

    pub fn money_income(&mut self, owner: i8, change: i32) {
        if change != 0 {
            // TODO: add effect depending on change < 0
            self.add_event(Event::MoneyChange(owner.into(), change.into()));
        }
    }

    pub fn money_change(&mut self, owner: i8, change: i32) {
        if change != 0 {
            // TODO: add effect depending on change < 0
            self.add_event(Event::MoneyChange(owner.into(), change.into()));
        }
    }

    pub fn money_buy(&mut self, owner: i8, cost: i32) {
        if cost > 0 {
            self.add_event(Event::MoneyChange(owner.into(), (-cost).into()));
        }
    }

    pub fn player_dies(&mut self, owner_id: i8) {
        if self.with_game(|game| game.get_owning_player(owner_id).map(|player| !player.dead).unwrap_or(false)) {
            self.add_event(Event::PlayerDies(owner_id.into()));
            // TODO: trigger scripts?
            if self.with_game(|game| game.get_living_teams().len() < 2) {
                self.add_event(Event::GameEnds);
            }
            if self.with_game(|game| !game.has_ended() && game.current_player().dead) {
                self.end_turn();
            }
        }
    }

    pub fn terrain_replace(&mut self, position: Point, terrain: Terrain<D>) {
        let old_terrain = self.with_map(|map| map.get_terrain(position).expect(&format!("Missing terrain at {:?}", position)).clone());
        self.add_event(Event::TerrainChange(position, old_terrain.clone(), terrain));
    }

    pub fn set_terrain_flag(&mut self, position: Point, flag: usize) {
        let terrain = self.with_map(|map| map.get_terrain(position).expect(&format!("Missing terrain at {:?}", position)).clone());
        if !terrain.has_flag(flag) {
            self.add_event(Event::TerrainFlag(position, FlagKey(flag)));
        }
    }
    pub fn remove_terrain_flag(&mut self, position: Point, flag: usize) {
        let terrain = self.with_map(|map| map.get_terrain(position).expect(&format!("Missing terrain at {:?}", position)).clone());
        if terrain.has_flag(flag) {
            self.add_event(Event::TerrainFlag(position, FlagKey(flag)));
        }
    }

    pub fn set_terrain_tag(&mut self, position: Point, key: usize, value: TagValue<D>) {
        let terrain = self.with_map(|map| map.get_terrain(position).expect(&format!("Missing terrain at {:?}", position)).clone());
        if !value.has_valid_type(&self.environment(), key) {
            return;
        }
        if let Some(old) = terrain.get_tag(key) {
            self.add_event(Event::TerrainReplaceTag(position, TagKeyValues(TagKey(key), [old, value])));
        } else {
            self.add_event(Event::TerrainSetTag(position, TagKeyValues(TagKey(key), [value])));
        }
    }
    pub fn remove_terrain_tag(&mut self, position: Point, key: usize) {
        let terrain = self.with_map(|map| map.get_terrain(position).expect(&format!("Missing terrain at {:?}", position)).clone());
        if let Some(old) = terrain.get_tag(key) {
            self.add_event(Event::TerrainRemoveTag(position, TagKeyValues(TagKey(key), [old])));
        }
    }

    /*pub fn terrain_anger(&mut self, position: Point, anger: u8) {
        let old_anger = self.with_map(|map| map.get_terrain(position).expect(&format!("Missing terrain at {:?}", position)).get_anger());
        self.add_event(Event::TerrainAnger(position, old_anger.into(), anger.into()));
    }

    pub fn terrain_capture_progress(&mut self, position: Point, progress: CaptureProgress) {
        let terrain = self.with_map(|map| map.get_terrain(position).expect(&format!("Missing terrain at {:?}", position)).clone());
        let old = terrain.get_capture_progress();
        if terrain.has_attribute(TerrainAttributeKey::CaptureProgress) && old != progress {
            self.add_event(Event::CaptureProgress(position, old, progress));
        }
    }

    pub fn terrain_built_this_turn(&mut self, position: Point, built_this_turn: u8) {
        let terrain = self.with_map(|map| map.get_terrain(position).expect(&format!("Missing terrain at {:?}", position)).clone());
        let old = terrain.get_built_this_turn();
        if terrain.has_attribute(TerrainAttributeKey::BuiltThisTurn) && old != built_this_turn {
            self.add_event(Event::UpdateBuiltThisTurn(position, old.into(), built_this_turn.into()));
        }
    }*/

    pub fn unit_creation(&mut self, position: Point, unit: Unit<D>) {
        if let ClientPerspective::Team(team) = unit.get_team() {
            if self.get_game().is_foggy() && self.with_game(|game| game.is_team_alive(team)) {
                let heroes = Hero::hero_influence_at(&*self.get_game(), position, unit.get_owner_id());
                let changes = unit.get_vision(&*self.get_game(), position, &heroes).into_iter()
                .filter(|(p, intensity)| *intensity < self.get_game().get_fog_at(ClientPerspective::Team(team), *p))
                .collect();
                self.change_fog(ClientPerspective::Team(team), changes);
            }
        }
        self.add_event(Event::UnitAdd(position, unit));
    }

    pub fn unit_add_transported(&mut self, position: Point, unit: Unit<D>) {
        self.add_event(Event::UnitAddBoarded(position, unit));
    }

    pub fn unit_path(&mut self, unload_index: Option<usize>, path: &Path<D>, board_at_the_end: bool, involuntarily: bool) {
        if path.steps.len() == 0 {
            return;
        }
        let mut unit = self.with_map(|map| map.get_unit(path.start).expect(&format!("Missing unit at {:?}", path.start)).clone());
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
        let (unit_id, disto) = self.observe_unit(path.start, unload_index);
        let transformed_unit = self.animate_unit_path(&unit, path, involuntarily);
        let (path_end, distortion) = path.end(&*self.get_game()).unwrap();
        if board_at_the_end {
            self.move_observed_unit(unit_id, path_end, Some(self.with_map(|map| map.get_unit(path_end).unwrap().get_transported().len())), disto + distortion);
            self.add_event(Event::UnitAddBoarded(path_end, transformed_unit));
        } else {
            if self.with_map(|map| map.get_unit(path_end).is_some()) {
                // TODO: this shouldn't happen at all
                panic!("Path would overwrite unit at {path_end:?}");
            }
            self.move_observed_unit(unit_id, path_end, None, disto + distortion);
            self.add_event(Event::UnitAdd(path_end, transformed_unit));
        }
        // update fog in case unit influences other units' vision range
        self.recalculate_fog();
        // remove tokens that were destroyed by the unit moving over them
        /*let income = self.with_game(|game| game.get_owning_player(unit.get_owner_id()).map(|player| player.get_income()))
            .filter(|income| *income != 0);*/
        let mut token_scripts = Vec::new();
        for p in self.with(|eh| path.points(&eh.game)).unwrap() {
            for token in self.get_game().get_tokens(p) {
                if let Some(function_index) = self.environment().config.token_on_unit_path(token.typ()) {
                    token_scripts.push((function_index, p, token));
                }
            }
            /*let tokens: Vec<Token<D>> = old_tokens.clone().into_iter().filter(|token| {
                match token {
                    Token::Pipe(_) => true,
                    Token::Coins1 => {
                        if let Some(income) = income {
                            self.money_change(unit.get_owner_id(), income / 2);
                        }
                        false
                    }
                    Token::Coins2 => {
                        if let Some(income) = income {
                            self.money_change(unit.get_owner_id(), income);
                        }
                        false
                    }
                    Token::Coins3 => {
                        if let Some(income) = income {
                            self.money_change(unit.get_owner_id(), income * 3 / 2);
                        }
                        false
                    }
                    Token::Bubble(owner, _) => {
                        owner.0 == unit.get_owner_id()
                    }
                    /*Token::Skull(skull) => {
                        skull.get_owner_id() == unit.get_owner_id()
                    }*/
                    Token::SludgeToken(_) => true,
                }
            }).collect();
            if tokens != old_tokens {
                self.add_event(Event::ReplaceToken(p, old_tokens.try_into().unwrap(), tokens.try_into().unwrap()));
            }*/
        }
        if token_scripts.len() > 0 {
            let environment = self.environment();
            let mut scope = Scope::new();
            //scope.push_constant(CONST_NAME_TRANSPORTER_POSITION, transporter.as_ref().map(|_| Dynamic::from(path.start)).unwrap_or(().into()));
            //scope.push_constant(CONST_NAME_TRANSPORTER, transporter.map(|u| Dynamic::from(u)).unwrap_or(().into()));
            //scope.push_constant(CONST_NAME_TRANSPORT_INDEX, transport_index.map(|i| Dynamic::from(i as i32)).unwrap_or(().into()));
            scope.push_constant(CONST_NAME_PATH, path.clone());
            scope.push_constant(CONST_NAME_UNIT, unit);
            scope.push_constant(CONST_NAME_UNIT_ID, unit_id);
            let engine = environment.get_engine_handler(self);
            let executor = Executor::new(engine, scope, environment);
            for (function_index, p, token) in token_scripts {
                match executor.run(function_index, (p, token)) {
                    Ok(()) => (),
                    Err(e) => {
                        // TODO: log error
                        println!("token OnUnitPath {function_index}: {e:?}");
                    }
                }
            }
        }
    }

    pub fn animate_unit_path(&mut self, unit: &Unit<D>, path: &Path<D>, involuntarily: bool) -> Unit<D> {
        let unit_team = unit.get_team();
        let owner_id = unit.get_owner_id();
        let heroes = Hero::map_influence(&*self.get_game(), owner_id);
        let mut current = path.start;
        //let mut previous = None;
        let mut transformed_unit = unit.clone();
        transformed_unit.set_en_passant(None);
        let mut steps = Vec::new();
        let mut vision_changes = HashMap::default();
        for (i, step) in path.steps.iter().enumerate() {
            if self.get_game().is_foggy() && !involuntarily && (i == 0 || unit.vision_mode().see_while_moving()) {
                let mut heroes = heroes.get(&(current, owner_id)).map(|h| h.clone()).unwrap_or(Vec::new());
                if let Some(strength) = Hero::aura_range(&*self.get_game(), &transformed_unit, current, None) {
                    heroes.push((transformed_unit.clone(), transformed_unit.get_hero().unwrap().clone(), current, None, strength as u8));
                }
                for (p, vision) in unit.get_vision(&*self.get_game(), current, &heroes) {
                    let vision = vision.min(vision_changes.remove(&p).unwrap_or(FogIntensity::Dark));
                    if vision < self.get_game().get_fog_at(unit_team, p) {
                        vision_changes.insert(p, vision);
                    }
                }
            }
            let (next, distortion) = step.progress(&*self.get_game(), current).unwrap();
            if !involuntarily && transformed_unit.transformed_by_movement(&*self.get_game(), current, next, distortion) {
                steps.push(UnitStep::Transform(current, *step, Some(transformed_unit.clone())));
            } else {
                steps.push(UnitStep::Simple(current, *step));
            }
            //previous = Some(current);
            current = next;
        }
        if self.get_game().is_foggy() {
            let mut heroes = heroes.get(&(current, owner_id)).map(|h| h.clone()).unwrap_or(Vec::new());
            if let Some(strength) = Hero::aura_range(&*self.get_game(), &transformed_unit, current, None) {
                heroes.push((transformed_unit.clone(), transformed_unit.get_hero().unwrap().clone(), current, None, strength as u8));
            }
            for (p, vision) in unit.get_vision(&*self.get_game(), current, &heroes) {
                let vision = vision.min(vision_changes.remove(&p).unwrap_or(FogIntensity::Dark));
                if vision < self.get_game().get_fog_at(unit_team, p) {
                    vision_changes.insert(p, vision);
                }
            }
        }
        self.add_event(Event::UnitPath(Some(unit.clone()), steps.try_into().unwrap()));
        /*if transformed_unit.has_attribute(AttributeKey::EnPassant) && path.steps.len() >= 2 {
            transformed_unit.set_en_passant(previous);
        }*/
        if self.get_game().is_foggy() {
            if unit_team != self.get_game().current_team() {
                self.recalculate_fog_for(unit_team);
            } else {
                self.change_fog(unit_team, vision_changes);
            }
        }
        transformed_unit
    }

    pub fn on_unit_normal_action(&mut self, id: usize, path: Path<D>, interrupted: bool, heroes: &[HeroInfluence<D>], ballast: &[TBallast<D>]) {
        let Some((p, unload_index)) = self.get_observed_unit_pos(id) else {
            return;
        };
        let unit = self.with_map(|map| {
            let mut u = map.get_unit(p).expect(&format!("Missing unit at {p:?}"));
            if let Some(i) = unload_index {
                u = u.get_transported().get(i).expect(&format!("Missing unit at {p:?}, index {i}"));
            }
            u.clone()
        });
        let transporter = self.get_game().get_unit(path.start);
        let other_unit = unload_index.and_then(|_| self.get_game().get_unit(p));
        let environment = self.get_game().environment();
        let scripts = environment.config.unit_normal_action_effects(
            &*self.get_game(),
            &unit,
            (p, unload_index),
            transporter.as_ref().map(|t| (t, path.start)),
            other_unit.as_ref().map(|u| (u, p)),
            heroes,
            ballast,
        );
        if scripts.len() == 0 {
            return;
        }
        let mut scope = Scope::new();
        scope.push_constant(CONST_NAME_TRANSPORTER_POSITION, transporter.as_ref().map(|_| Dynamic::from(path.start)).unwrap_or(().into()));
        scope.push_constant(CONST_NAME_TRANSPORTER, transporter.map(|u| Dynamic::from(u)).unwrap_or(().into()));
        //scope.push_constant(CONST_NAME_TRANSPORT_INDEX, transport_index.map(|i| Dynamic::from(i as i32)).unwrap_or(().into()));
        scope.push_constant(CONST_NAME_PATH, path);
        scope.push_constant(CONST_NAME_POSITION, p);
        scope.push_constant(CONST_NAME_UNIT, unit);
        scope.push_constant(CONST_NAME_UNIT_ID, id);
        scope.push_constant(CONST_NAME_INTERRUPTED, interrupted);
        let engine = environment.get_engine_handler(self);
        let executor = Executor::new(engine, scope, environment);
        for function_index in scripts {
            match executor.run(function_index, ()) {
                Ok(()) => (),
                Err(e) => {
                    // TODO: log error
                    println!("unit OnNormalAction {function_index}: {e:?}");
                }
            }
        }
    }

    pub fn set_unit_flag(&mut self, position: Point, flag: usize) {
        let unit = self.with_map(|map| map.get_unit(position).expect(&format!("Missing unit at {:?}", position)).clone());
        if !unit.has_flag(flag) {
            self.add_event(Event::UnitFlag(position, FlagKey(flag)));
        }
    }
    pub fn remove_unit_flag(&mut self, position: Point, flag: usize) {
        let unit = self.with_map(|map| map.get_unit(position).expect(&format!("Missing unit at {:?}", position)).clone());
        if unit.has_flag(flag) {
            self.add_event(Event::UnitFlag(position, FlagKey(flag)));
        }
    }

    pub fn set_unit_tag(&mut self, position: Point, key: usize, value: TagValue<D>) {
        let unit = self.with_map(|map| map.get_unit(position).expect(&format!("Missing unit at {:?}", position)).clone());
        if !value.has_valid_type(&self.environment(), key) {
            return;
        }
        if let Some(old) = unit.get_tag(key) {
            self.add_event(Event::UnitReplaceTag(position, TagKeyValues(TagKey(key), [old, value])));
        } else {
            self.add_event(Event::UnitSetTag(position, TagKeyValues(TagKey(key), [value])));
        }
    }
    pub fn remove_unit_tag(&mut self, position: Point, key: usize) {
        let unit = self.with_map(|map| map.get_unit(position).expect(&format!("Missing unit at {:?}", position)).clone());
        if let Some(old) = unit.get_tag(key) {
            self.add_event(Event::UnitRemoveTag(position, TagKeyValues(TagKey(key), [old])));
        }
    }

    /*pub fn unit_moved_this_game(&mut self, position: Point) {
        let _ = self.with_map(|map| map.get_unit(position).expect(&format!("Missing unit at {:?}", position)).clone());
        self.add_event(Event::UnitMovedThisGame(position));
    }

    pub fn unit_en_passant_opportunity(&mut self, position: Point, targetable: Option<Point>) {
        let unit = self.with_map(|map| map.get_unit(position).expect(&format!("Missing unit at {:?}", position)).clone());
        if unit.get_en_passant() != targetable {
            self.add_event(Event::EnPassantOpportunity(position, unit.get_en_passant(), targetable));
        }
    }

    pub fn unit_direction(&mut self, position: Point, direction: D) {
        let unit = self.with_map(|map| map.get_unit(position).expect(&format!("Missing unit at {:?}", position)).clone());
        if unit.has_attribute(AttributeKey::Direction) {
            let starting_dir = unit.get_direction();
            self.add_event(Event::UnitDirection(position, starting_dir, direction));
        }
    }

    pub fn unit_status(&mut self, position: Point, status: ActionStatus) {
        let unit = self.with_map(|map| map.get_unit(position).expect(&format!("Missing unit at {:?}", position)).clone());
        if unit.can_have_status(status) && status != unit.get_status() {
            self.add_event(Event::UnitActionStatus(position, unit.get_status(), status));
        }
    }

    pub fn unit_status_boarded(&mut self, position: Point, index: usize, status: ActionStatus) {
        let unit = self.with_map(|map| map.get_unit(position).expect(&format!("Missing unit at {:?}", position)).clone());
        let unit = unit.get_transported().get(index).unwrap();
        if unit.can_have_status(status) && status != unit.get_status() {
            self.add_event(Event::UnitActionStatusBoarded(position, index.into(), unit.get_status(), status));
        }
    }

    pub fn unit_level(&mut self, position: Point, level: u8) {
        let unit = self.with_map(|map| map.get_unit(position).expect(&format!("Missing unit at {:?}", position)).clone());
        let level = level.min(self.environment().config.max_unit_level());
        if unit.has_attribute(AttributeKey::Level) && level != unit.get_level() {
            self.add_event(Event::UnitLevel(position, unit.get_level().into(), level.into()));
        }
    }

    pub fn unit_damage(&mut self, position: Point, damage: u16) {
        let unit = self.with_map(|map| map.get_unit(position).expect(&format!("Missing unit at {:?}", position)).clone());
        self.add_event(Event::UnitHpChange(position, (-(damage.min(unit.get_hp() as u16) as i32)).into(), (-(damage as i32)).max(-999).into()));
    }

    pub fn unit_mass_damage(&mut self, amounts: &HashMap<Point, u16>) {
        //let mut list = Vec::new();
        for (position, damage) in amounts {
            /*let unit = self.with_map(|map| map.get_unit(position).expect(&format!("Missing unit at {:?}", position)).clone());
            let damage = -(damage as i32);
            list.push((position, damage.max(-(unit.get_hp() as i32)).into(), damage.max(-999).into()));*/
            self.unit_damage(*position, *damage);
        }
        //self.add_event(Event::UnitMassHpChange(list.try_into().unwrap()));
    }

    pub fn unit_repair(&mut self, position: Point, heal: u8) {
        let hp = self.with_map(|map| map.get_unit(position).expect(&format!("Missing unit at {:?}", position)).get_hp());
        self.effect_repair(position);
        self.add_event(Event::UnitHpChange(position, (heal.min(100 - hp) as i32).into(), (heal.min(100) as i32).into()));
    }

    pub fn unit_heal(&mut self, position: Point, heal: u8) {
        let unit = self.with_map(|map| map.get_unit(position).expect(&format!("Missing unit at {:?}", position)).clone());
        if !unit.has_attribute(AttributeKey::Hp) {
            return;
        }
        let hp = unit.get_hp();
        self.effect_heal(position);
        self.add_event(Event::UnitHpChange(position, (heal.min(100 - hp) as i32).into(), (heal.min(100) as i32).into()));
    }

    pub fn unit_mass_heal(&mut self, amounts: HashMap<Point, u8>) {
        for (position, damage) in amounts {
            self.unit_heal(position, damage);
        }
    }

    pub fn unit_heal_boarded(&mut self, position: Point, index: usize, heal: u8) {
        let unit = self.with_map(|map| map.get_unit(position).expect(&format!("Missing unit at {:?}", position)).clone());
        let transported = unit.get_transported();
        if transported.len() > index {
            let hp = transported[index].get_hp();
            if hp < 100 {
                self.add_event(Event::UnitHpChangeBoarded(position, index.into(), (heal.min(100 - hp) as i32).into()));
            }
        } else {
            panic!("Can't heal unit at {position:?}, boarded as {index}");
        }
    }

    pub fn unit_damage_boarded(&mut self, position: Point, index: usize, damage: u8) {
        let unit = self.with_map(|map| map.get_unit(position).expect(&format!("Missing unit at {:?}", position)).clone());
        let transported = unit.get_transported();
        if transported.len() > index {
            let hp = transported[index].get_hp();
            if hp > 0 {
                self.add_event(Event::UnitHpChangeBoarded(position, index.into(), (-(damage.min(hp) as i32)).into()));
            }
        } else {
            panic!("Can't damage unit at {position:?}, boarded as {index}");
        }
    }*/

    pub fn unit_remove(&mut self, position: Point) {
        let unit = self.with_map(|map| map.get_unit(position).expect(&format!("Missing unit at {:?}", position)).clone());
        self.remove_observed_units_at(position);
        self.add_event(Event::UnitRemove(position, unit.clone()));
    }

    pub fn unit_death(&mut self, position: Point) {
        self.add_event(Event::Effect(Effect::Explode(position)));
        let unit = self.with_map(|map| map.get_unit(position).expect(&format!("Missing unit at {:?}", position)).clone());
        self.remove_observed_units_at(position);
        self.add_event(Event::UnitRemove(position, unit.clone()));
    }

    pub fn unit_death_boarded(&mut self, position: Point, index: usize) {
        self.add_event(Event::Effect(Effect::Explode(position)));
        let unit = self.with_map(|map| map.get_unit(position).expect(&format!("Missing unit at {:?}", position)).clone());
        let transported = unit.get_transported();
        if transported.len() > index {
            if let Some((id, _)) = self.observation_id(position, Some(index)) {
                self.remove_observed_unit(id);
            }
            self.add_event(Event::UnitRemoveBoarded(position, index.into(), transported[index].clone()));
        } else {
            panic!("Can't damage unit at {position:?}, boarded as {index}");
        }
    }

    pub fn unit_mass_death(&mut self, positions: &HashSet<Point>) {
        // TODO: mass-effect
        for position in positions {
            self.unit_death(*position);
        }
    }

    pub fn unit_replace(&mut self, position: Point, new_unit: Unit<D>) {
        let unit = self.with_map(|map| map.get_unit(position).expect(&format!("Missing unit at {:?}", position)).clone());
        self.add_event(Event::UnitRemove(position, unit.clone()));
        self.add_event(Event::UnitAdd(position, new_unit));
    }

    pub fn effect_kraken_rage(&mut self, position: Point) {
        self.add_event(Event::Effect(Effect::KrakenRage(position)))
    }


    pub fn trigger_all_terrain_scripts(
        &mut self,
        get_script: impl Fn(&Handle<Game<D>>, Point, &Terrain<D>, &[HeroInfluence<D>]) -> Vec<usize>,
        before_executing: impl FnOnce(&mut Self),
        execute_script: impl Fn(&mut Self, Vec<usize>, Point, Terrain<D>),
    ) {
        let hero_auras = Hero::map_influence(&*self.get_game(), -1);
        let mut scripts = Vec::new();
        for p in self.with_map(|map| map.all_points()) {
            let terrain = self.get_game().get_terrain(p).unwrap();
            let heroes = hero_auras.get(&(p, terrain.get_owner_id())).map(|h| h.as_slice()).unwrap_or(&[]);
            let script = get_script(&*self.get_game(), p, &terrain, heroes);
            if script.len() > 0 {
                scripts.push((script, p, terrain));
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
        get_script: impl Fn(&Handle<Game<D>>, &Unit<D>, Point, Option<(&Unit<D>, usize)>, &[HeroInfluence<D>]) -> Vec<usize>,
        before_executing: impl FnOnce(&mut Self),
        execute_script: impl Fn(&mut Self, Vec<usize>, Point, &Unit<D>, usize),
    ) {
        let hero_auras = Hero::map_influence(&*self.get_game(), -1);
        let mut scripts = Vec::new();
        for p in self.with_map(|map| map.all_points()) {
            if let Some(unit) = self.with_map(|map| map.get_unit(p).cloned()) {
                let heroes = hero_auras.get(&(p, unit.get_owner_id())).map(|h| h.as_slice()).unwrap_or(&[]);
                let script = get_script(&*self.get_game(), &unit, p, None, heroes);
                if script.len() > 0 {
                    let id = self.observe_unit(p, None).0;
                    scripts.push((script, unit.clone(), p, id));
                }
                for (i, u) in unit.get_transported().iter().enumerate() {
                    let script = get_script(&*self.get_game(), u, p, Some((&unit, i)), heroes);
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


    pub fn accept(self) -> EventsMap<D> {
        RwLock::into_inner(Arc::into_inner(self.inner).unwrap()).unwrap().accept()
    }

    pub fn cancel(self) {
        RwLock::into_inner(Arc::into_inner(self.inner).unwrap()).unwrap().cancel()
    }
}


