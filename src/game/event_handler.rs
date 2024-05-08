use std::collections::{HashMap, HashSet};

use interfaces::game_interface::GameInterface;
use interfaces::game_interface::{Events, Perspective as IPerspective, ClientPerspective};

use crate::config::environment::Environment;
use crate::map::map::Map;
use crate::map::map_view::MapView;
use crate::map::point::Point;
use crate::map::wrapping_map::Distortion;
use crate::terrain::attributes::CaptureProgress;
use crate::terrain::attributes::TerrainAttributeKey;
use crate::terrain::terrain::*;
use crate::terrain::TerrainType;
use crate::units::attributes::{AttributeKey, ActionStatus};
use crate::units::combat::WeaponType;
use crate::player::*;
use crate::details::{Detail, SludgeToken};
use crate::map::direction::Direction;
use crate::game::game::*;
use crate::game::fog::*;
use crate::units::hero::Hero;
use crate::units::movement::Path;
use crate::units::unit::Unit;
use crate::units::unit_types::UnitType;
use super::commands::Command;
use super::events::{Event, Effect, UnitStep};
use super::game_view::GameView;

pub struct EventHandler<'a, D: Direction> {
    game: &'a mut Game<D>,
    events: HashMap<IPerspective, Vec<Event<D>>>,
    random: Box<dyn Fn() -> f32>,
    observed_units: HashMap<usize, (Point, Option<usize>, Distortion<D>)>,
    next_observed_unit_id: usize,
}

impl<'a, D: Direction> EventHandler<'a, D> {
    pub fn new(game: &'a mut Game<D>, random: Box<dyn Fn() -> f32>) -> Self {
        let mut events = HashMap::new();
        events.insert(IPerspective::Server, vec![]);
        events.insert(IPerspective::Neutral, vec![]);
        for team in game.get_teams() {
            events.insert(IPerspective::Team(team), vec![]);
        }
        EventHandler {
            game,
            events,
            random,
            next_observed_unit_id: 0,
            observed_units: HashMap::new(),
        }
    }

    pub fn get_game(&self) -> &Game<D> {
        &self.game
    }

    pub fn get_map(&self) -> &Map<D> {
        self.game.get_map()
    }

    pub fn environment(&self) -> &Environment {
        self.game.environment()
    }

    pub fn observe_unit(&mut self, position: Point, unload_index: Option<usize>) -> (usize, Distortion<D>) {
        if let Some((id, (_, _, distortion))) = self.observed_units.iter()
        .find(|(_, (p, i, _))| *p == position && *i == unload_index) {
            (*id, *distortion)
        } else {
            self.observed_units.insert(self.next_observed_unit_id, (position, unload_index, Distortion::neutral()));
            self.next_observed_unit_id += 1;
            (self.next_observed_unit_id - 1, Distortion::neutral())
        }
    }
    pub fn get_observed_unit(&self, id: usize) -> Option<&(Point, Option<usize>, Distortion<D>)> {
        self.observed_units.get(&id)
    }

    pub fn get_observed_unit_pos(&self, id: usize) -> Option<(Point, Option<usize>)> {
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
        for i in 0..self.environment().config.max_transported() {
            if let Some((id, _)) = self.observation_id(position, Some(i)) {
                self.observed_units.remove(&id);
            }
        }
    }
    
    pub fn end_turn(&mut self) {
        let owner_id = self.get_game().current_player().get_owner_id();
        // un-exhaust units
        for p in self.get_map().all_points() {
            if let Some(unit) = self.get_map().get_unit(p).cloned() {
                if unit.get_owner_id() == owner_id {
                    match unit.get_status() {
                        ActionStatus::Exhausted => self.unit_status(p, ActionStatus::Ready),
                        _ => (),
                    }
                    for (index, u) in unit.get_transported().iter().enumerate() {
                        if u.is_exhausted() {
                            self.unit_status_boarded(p, index, ActionStatus::Ready);
                        }
                        //if unit.heal_transported() > 0 {
                        //    self.unit_heal_boarded(p, index, unit.heal_transported() as u8);
                        //} else if unit.heal_transported() < 0 {
                        //    self.unit_damage_boarded(position, index, -unit.heal_transported() as u8);
                        //    kill units with 0 HP
                        //}
                    }
                }
            }
        }

        // unit end turn event
        self.trigger_all_unit_scripts(
            |game, unit, unit_pos, transporter, heroes| {
                unit.on_end_turn(game, unit_pos, transporter, heroes)
            },
            |_observation_id| {},
            |this, scripts, unit_pos, unit, _observation_id| {
                for script in scripts {
                    script.trigger(this, unit_pos, unit);
                }
            }
        );

        // reset built_this_turn-counter for realties
        for p in self.get_map().all_points() {
            self.terrain_built_this_turn(p, 0);
        }

        let fog_before = if self.get_game().is_foggy() {
            let next_player = self.get_game().players.get((self.get_game().current_turn() + 1) % self.get_game().players.len()).unwrap();
            Some(self.get_game().recalculate_fog(next_player.get_team()))
        } else {
            None
        };

        self.next_turn();
        let owner_id = self.get_game().current_player().get_owner_id();

        // reset status for repairing units
        for p in self.get_map().all_points() {
            if let Some(unit) = self.get_map().get_unit(p) {
                if unit.get_owner_id() == owner_id && unit.get_status() == ActionStatus::Repairing {
                    self.unit_status(p, ActionStatus::Ready);
                }
            }
        }

        // reset capture-progress / finish capturing
        for p in self.get_map().all_points() {
            let terrain = self.get_map().get_terrain(p).unwrap();
            if let Some((new_owner, progress)) = terrain.get_capture_progress() {
                if new_owner.0 == owner_id {
                    if let Some(unit) = self.get_map().get_unit(p).filter(|u| u.get_owner_id() == owner_id && u.can_capture()) {
                        if unit.get_status() == ActionStatus::Capturing {
                            let max_progress = terrain.get_capture_resistance();
                            let progress = progress as u16 + (unit.get_hp() as f32 / 10.).ceil() as u16;
                            if progress < max_progress as u16 {
                                self.terrain_capture_progress(p, Some((new_owner, (progress as u8).into())));
                            } else {
                                // captured
                                let terrain = TerrainBuilder::new(self.environment(), terrain.typ())
                                .copy_from(terrain)
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
            if self.get_map().get_unit(p).filter(|u| u.get_owner_id() == owner_id && u.get_status() == ActionStatus::Capturing).is_some() {
                self.unit_status(p, ActionStatus::Ready);
            }
        }

        let next_power = self.get_game().current_player().commander.get_next_power();
        if self.get_game().current_player().commander.can_activate_power(next_power, true) {
            Command::activate_power(self, next_power, &[]);
        }

        // end merc powers
        for p in self.get_map().all_points() {
            if let Some(unit) = self.get_map().get_unit(p).filter(|u| u.get_owner_id() == owner_id) {
                let hero = unit.get_hero();
                let next_power = hero.get_next_power(self.environment());
                if hero.can_activate_power(self.environment(), next_power, true) {
                    // TODO: this skips the custom-action. maybe execute the custom action if no user input is needed
                    self.hero_charge_sub(p, None, hero.power_cost(self.environment(), next_power));
                    self.hero_power(p, next_power);
                }
            }
        }

        self.start_turn(fog_before);

        if !self.get_game().has_ended() && self.game.current_player().dead {
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
            let player_ids: Vec<i8> = self.game.players.iter().map(|player| player.get_owner_id()).collect();
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

        let owner_id = self.game.current_player().get_owner_id();
        // return drones to their origin if possible or destroy them
        let mut drone_parents: HashMap<u16, (Point, usize)> = self.get_map().all_points()
        .into_iter()
        .filter_map(|p| self.get_map().get_unit(p).and_then(|u| Some((p, u))))
        .filter(|(_, u)| u.get_owner_id() == owner_id)
        .filter_map(|(p, unit)| {
            if let Some(drone_id) = unit.get_drone_station_id() {
                Some((drone_id, (p, unit.remaining_transport_capacity())))
            } else {
                None
            }
        }).collect();
        let mut dead_drones = HashSet::new();
        for p in self.get_map().all_points() {
            if let Some(unit) = self.game.get_map().get_unit(p) {
                if unit.get_owner_id() != owner_id {
                    continue;
                }
                if let Some(drone_id) = unit.get_drone_id() {
                    if let Some((destination, capacity)) = drone_parents.get_mut(&drone_id) {
                        // move drone back aboard its parent
                        if let Some((id, distortion)) = self.observation_id(p, None) {
                            self.observed_units.insert(id, (*destination, Some(self.get_map().get_unit(*destination).unwrap().get_transported().len()), distortion));
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
        }

        self.trigger_all_unit_scripts(
            |game, unit, unit_pos, transporter, heroes| {
                if dead_drones.contains(&unit_pos) {
                    unit.on_death(game, unit_pos, transporter, None, heroes, &[])
                } else {
                    Vec::new()
                }
            },
            |handler| handler.unit_mass_death(&dead_drones),
            |handler, scripts, unit_pos, unit, _observation_id| {
                let mut unit = unit.clone();
                for script in scripts {
                    script.trigger(handler, &mut unit, unit_pos, None, None);
                }
            }
        );

        // release the kraken
        for p in self.get_map().all_points() {
            if self.get_map().get_terrain(p).unwrap().typ() == TerrainType::TentacleDepths && self.get_map().get_unit(p) == None {
                // TODO: configure which unit is created here
                self.unit_creation(p, UnitType::Tentacle.instance(self.environment()).build_with_defaults());
            }
        }

        // has to be recalculated before structures, because the effects of some structures on
        // other players should maybe not be visible
        //self.recalculate_fog(false);

        let income = self.game.current_player().get_income() * self.get_map().get_income_factor(owner_id);
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
            |this, scripts, unit_pos, unit, _observation_id| {
                for script in scripts {
                    script.trigger(this, unit_pos, unit);
                }
            }
        );

        // tick sludge tokens
        for p in self.get_map().all_points() {
            for (index, d) in self.get_map().get_details(p).iter().enumerate() {
                match d {
                    Detail::SludgeToken(token) => {
                        if token.get_owner_id() == owner_id {
                            let counter = token.get_counter();
                            self.detail_remove(p, index);
                            if counter > 0 {
                                self.detail_add(p, Detail::SludgeToken(SludgeToken::new(&self.environment().config, owner_id, counter - 1)));
                            }
                        }
                        break;
                    },
                    _ => ()
                }
            }
        }

        // structures may have destroyed some units, vision may be reduced due to merc powers ending
        self.recalculate_fog();
    }

    pub fn recalculate_fog(&mut self) {
        let current_team = self.game.current_player().get_team();
        // only remove fog for the current team
        let mut fog = self.game.recalculate_fog(current_team);
        for (p, intensity) in fog.iter_mut() {
            *intensity = self.game.get_fog_at(current_team, *p).min(*intensity);
        }
        self.change_fog(current_team, fog);
        // reset fog for other teams
        let mut perspectives: HashSet<ClientPerspective> = self.game.get_teams().into_iter()
        .filter(|team| ClientPerspective::Team(*team) != current_team)
        .map(|team| ClientPerspective::Team(team))
        .collect();
        perspectives.insert(ClientPerspective::Neutral);
        for team in perspectives {
            self.recalculate_fog_for(team);
        }
    }

    pub fn recalculate_fog_for(&mut self, team: ClientPerspective) {
        let fog = self.game.recalculate_fog(team);
        self.change_fog(team, fog);
    }

    pub fn rng(&self) -> f32 {
        (*self.random)()
    }

    fn add_event(&mut self, event: Event<D>) {
        event.apply(&mut self.game);
        for (key, events) in self.events.iter_mut() {
            if let Ok(perspective) = key.try_into() {
                if let Some(event) = event.fog_replacement(self.game, perspective) {
                    events.push(event);
                }
            }
        }
        self.events.get_mut(&IPerspective::Server).unwrap().push(event);
    }

    pub fn change_fog(&mut self, team: ClientPerspective, changes: HashMap<Point, FogIntensity>) {
        let changes: Vec<(Point, FogIntensity, FogIntensity)> = changes.into_iter()
        .map(|(p, intensity)| (p, self.game.get_fog_at(team, p), intensity))
        .filter(|(_, before, after)| before != after)
        .collect();
        if changes.len() > 0 {
            self.add_event(Event::PureFogChange(from_client_perspective(team).into(), changes.try_into().unwrap()));
        }
    }

    pub fn commander_charge_add(&mut self, owner: i8, change: u32) {
        if let Some(player) = self.get_game().get_owning_player(owner) {
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
        if let Some(player) = self.get_game().get_owning_player(owner) {
            let change = -(change as i32).min(player.commander.get_charge() as i32);
            if change < 0 {
                self.add_event(Event::CommanderCharge(owner.into(), change.into()));
            }
        }
    }

    pub fn commander_power(&mut self, owner: i8, index: usize) {
        if let Some(player) = self.get_game().get_owning_player(owner) {
            if player.commander.get_active_power() != index {
                self.add_event(Event::CommanderPowerIndex(owner.into(), player.commander.get_active_power().into(), index.into()));
            }
        }
    }

    pub fn detail_add(&mut self, position: Point, detail: Detail<D>) {
        let old_details = self.get_map().get_details(position);
        let mut details = old_details.to_vec();
        details.push(detail);
        if old_details != details.as_slice() {
            self.add_event(Event::ReplaceDetail(position, old_details.to_vec().try_into().unwrap(), Detail::correct_stack(details, self.environment()).try_into().unwrap()));
        }
    }

    pub fn detail_remove(&mut self, position: Point, index: usize) {
        let details = self.get_map().get_details(position);
        if details.len() <= index {
            panic!("Missing Detail at {position:?}");
        } else {
            self.add_event(Event::RemoveDetail(position, index.into(), details[index].clone()));
        }
    }

    pub fn effect_fog_surprise(&mut self, position: Point) {
        let team = match self.game.current_player().get_team() {
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
        let unit = self.get_map().get_unit(position).expect(&format!("Missing unit at {:?}", position)).clone();
        let old_hero = unit.get_hero();
        if hero != old_hero {
            self.add_event(Event::HeroSet(position, old_hero, hero));
        }
    }

    pub fn hero_charge_add(&mut self, position: Point, unload_index: Option<usize>, change: u8) {
        self.hero_charge(position, unload_index, change as i8)
    }

    pub fn hero_charge_sub(&mut self, position: Point, unload_index: Option<usize>, change: u8) {
        self.hero_charge(position, unload_index, -(change as i8))
    }

    fn hero_charge(&mut self, position: Point, unload_index: Option<usize>, change: i8) {
        let unit = self.get_map().get_unit(position).expect(&format!("Missing unit at {:?}", position)).clone();
        let hero = unit.get_hero();
        let change = change.max(-(hero.get_charge() as i8)).min((hero.typ().max_charge(self.environment()) - hero.get_charge()) as i8);
        if change != 0 {
            if let Some(unload_index) = unload_index {
                self.add_event(Event::HeroChargeTransported(position, unload_index.into(), change.into()));
            } else {
                self.add_event(Event::HeroCharge(position, change.into()));
            }
        }
    }

    pub fn hero_power(&mut self, position: Point, index: usize) {
        let unit: Unit<D> = self.get_map().get_unit(position).expect(&format!("Missing unit at {:?}", position)).clone();
        let hero = unit.get_hero();
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
        if self.game.get_owning_player(owner_id).map(|player| !player.dead).unwrap_or(false) {
            self.add_event(Event::PlayerDies(owner_id.into()));
            // TODO: trigger scripts?
            if self.game.get_living_teams().len() < 2 {
                self.add_event(Event::GameEnds);
            }
            if !self.game.has_ended() && self.game.current_player().dead {
                self.end_turn();
            }
        }
    }

    pub fn terrain_replace(&mut self, position: Point, terrain: Terrain) {
        let old_terrain = self.get_map().get_terrain(position).expect(&format!("Missing terrain at {:?}", position));
        self.add_event(Event::TerrainChange(position, old_terrain.clone(), terrain));
    }

    pub fn terrain_anger(&mut self, position: Point, anger: u8) {
        let old_anger = self.get_map().get_terrain(position).expect(&format!("Missing terrain at {:?}", position)).get_anger();
        self.add_event(Event::TerrainAnger(position, old_anger.into(), anger.into()));
    }

    pub fn terrain_capture_progress(&mut self, position: Point, progress: CaptureProgress) {
        let terrain = self.get_map().get_terrain(position).expect(&format!("Missing terrain at {:?}", position));
        let old = terrain.get_capture_progress();
        if terrain.has_attribute(TerrainAttributeKey::CaptureProgress) && old != progress {
            self.add_event(Event::CaptureProgress(position, old, progress));
        }
    }

    pub fn terrain_built_this_turn(&mut self, position: Point, built_this_turn: u8) {
        let terrain = self.get_map().get_terrain(position).expect(&format!("Missing terrain at {:?}", position));
        let old = terrain.get_built_this_turn();
        if terrain.has_attribute(TerrainAttributeKey::BuiltThisTurn) && old != built_this_turn {
            self.add_event(Event::UpdateBuiltThisTurn(position, old.into(), built_this_turn.into()));
        }
    }

    pub fn unit_creation(&mut self, position: Point, unit: Unit<D>) {
        if let ClientPerspective::Team(team) = unit.get_team() {
            if self.get_game().is_foggy() && self.get_game().is_team_alive(team) {
                let heroes = Hero::hero_influence_at(self.get_game(), position, unit.get_owner_id());
                let changes = unit.get_vision(self.get_game(), position, &heroes).into_iter()
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
        let mut unit = self.get_map().get_unit(path.start).expect(&format!("Missing unit at {:?}", path.start)).clone();
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
        let transformed_unit = self.animate_unit_path(&unit, path, involuntarily);
        let (path_end, distortion) = path.end(self.get_map()).unwrap();
        if board_at_the_end {
            if let Some((id, disto)) = self.observation_id(path.start, unload_index) {
                self.observed_units.insert(id, (path_end, Some(self.get_map().get_unit(path_end).unwrap().get_transported().len()), disto + distortion));
            }
            self.add_event(Event::UnitAddBoarded(path_end, transformed_unit));
        } else {
            if let Some(_) = self.get_map().get_unit(path_end) {
                // TODO: this shouldn't happen at all
                panic!("Path would overwrite unit at {path_end:?}");
            }
            if let Some((id, disto)) = self.observation_id(path.start, unload_index) {
                self.observed_units.insert(id, (path_end, None, disto + distortion));
            }
            self.add_event(Event::UnitAdd(path_end, transformed_unit));
        }
        // update fog in case unit influences other units' vision range
        self.recalculate_fog();
        // remove details that were destroyed by the unit moving over them
        for p in path.points(self.get_map()).unwrap() {
            let old_details = self.get_map().get_details(p).to_vec();
            let details: Vec<Detail<D>> = old_details.clone().into_iter().filter(|detail| {
                match detail {
                    Detail::Pipe(_) => true,
                    Detail::Coins1 => {
                        if let Some(player) = unit.get_player(self.get_game()) {
                            self.money_change(unit.get_owner_id(), player.get_income() / 2);
                        }
                        false
                    }
                    Detail::Coins2 => {
                        if let Some(player) = unit.get_player(self.get_game()) {
                            self.money_change(unit.get_owner_id(), player.get_income());
                        }
                        false
                    }
                    Detail::Coins3 => {
                        if let Some(player) = unit.get_player(self.get_game()) {
                            self.money_change(unit.get_owner_id(), player.get_income() * 3 / 2);
                        }
                        false
                    }
                    Detail::Bubble(owner, _) => {
                        owner.0 == unit.get_owner_id()
                    }
                    Detail::Skull(skull) => {
                        skull.get_owner_id() == unit.get_owner_id()
                    }
                    Detail::SludgeToken(_) => true,
                }
            }).collect();
            if details != old_details {
                self.add_event(Event::ReplaceDetail(p, old_details.try_into().unwrap(), details.try_into().unwrap()));
            }
        }
    }

    pub fn animate_unit_path(&mut self, unit: &Unit<D>, path: &Path<D>, involuntarily: bool) -> Unit<D> {
        let unit_team = unit.get_team();
        let owner_id = unit.get_owner_id();
        let heroes = Hero::map_influence(self.get_game(), owner_id);
        let mut current = path.start;
        let mut previous = None;
        let mut transformed_unit = unit.clone();
        transformed_unit.set_en_passant(None);
        let mut steps = Vec::new();
        let mut vision_changes = HashMap::new();
        for (i, step) in path.steps.iter().enumerate() {
            if self.get_game().is_foggy() && !involuntarily && (i == 0 || unit.vision_mode().see_while_moving()) {
                let mut heroes = heroes.get(&(current, owner_id)).map(|h| h.clone()).unwrap_or(Vec::new());
                if transformed_unit.is_hero() && Hero::aura_range(self.get_game(), &transformed_unit, current, None).is_some() {
                    heroes.push((transformed_unit.clone(), transformed_unit.get_hero(), current, None));
                }
                for (p, vision) in unit.get_vision(self.get_game(), current, &heroes) {
                    let vision = vision.min(vision_changes.remove(&p).unwrap_or(FogIntensity::Dark));
                    if vision < self.get_game().get_fog_at(unit_team, p) {
                        vision_changes.insert(p, vision);
                    }
                }
            }
            let (next, distortion) = step.progress(self.get_map(), current).unwrap();
            if !involuntarily && transformed_unit.transformed_by_movement(self.get_map(), current, next, distortion) {
                steps.push(UnitStep::Transform(current, *step, Some(transformed_unit.clone())));
            } else {
                steps.push(UnitStep::Simple(current, *step));
            }
            previous = Some(current);
            current = next;
        }
        if self.get_game().is_foggy() {
            let mut heroes = heroes.get(&(current, owner_id)).map(|h| h.clone()).unwrap_or(Vec::new());
            if transformed_unit.is_hero() && Hero::aura_range(self.get_game(), &transformed_unit, current, None).is_some() {
                heroes.push((transformed_unit.clone(), transformed_unit.get_hero(), current, None));
            }
            for (p, vision) in unit.get_vision(self.get_game(), current, &heroes) {
                let vision = vision.min(vision_changes.remove(&p).unwrap_or(FogIntensity::Dark));
                if vision < self.get_game().get_fog_at(unit_team, p) {
                    vision_changes.insert(p, vision);
                }
            }
        }
        self.add_event(Event::UnitPath(Some(unit.clone()), steps.try_into().unwrap()));
        if transformed_unit.has_attribute(AttributeKey::EnPassant) && path.steps.len() >= 2 {
            transformed_unit.set_en_passant(previous);
        }
        if self.get_game().is_foggy() {
            if unit_team != self.game.current_player().get_team() {
                self.recalculate_fog_for(unit_team);
            } else {
                self.change_fog(unit_team, vision_changes);
            }
        }
        transformed_unit
    }

    pub fn unit_moved_this_game(&mut self, position: Point) {
        let _ = self.get_map().get_unit(position).expect(&format!("Missing unit at {:?}", position));
        self.add_event(Event::UnitMovedThisGame(position));
    }

    pub fn unit_en_passant_opportunity(&mut self, unit_pos: Point, targetable: Option<Point>) {
        let unit = self.get_map().get_unit(unit_pos).expect(&format!("Missing unit at {:?}", unit_pos));
        if unit.get_en_passant() != targetable {
            self.add_event(Event::EnPassantOpportunity(unit_pos, unit.get_en_passant(), targetable));
        }
    }

    pub fn unit_direction(&mut self, position: Point, direction: D) {
        let unit = self.get_map().get_unit(position).expect(&format!("Missing unit at {:?}", position));
        if unit.has_attribute(AttributeKey::Direction) {
            let starting_dir = unit.get_direction();
            self.add_event(Event::UnitDirection(position, starting_dir, direction));
        }
    }

    pub fn unit_status(&mut self, position: Point, status: ActionStatus) {
        let unit = self.get_map().get_unit(position).expect(&format!("Missing unit at {:?}", position));
        if unit.can_have_status(status) && status != unit.get_status() {
            self.add_event(Event::UnitActionStatus(position, unit.get_status(), status));
        }
    }

    pub fn unit_status_boarded(&mut self, position: Point, index: usize, status: ActionStatus) {
        let unit = self.get_map().get_unit(position).expect(&format!("Missing unit at {:?}", position));
        let unit = unit.get_transported().get(index).unwrap();
        if unit.can_have_status(status) && status != unit.get_status() {
            self.add_event(Event::UnitActionStatusBoarded(position, index.into(), unit.get_status(), status));
        }
    }

    /*pub fn unit_build_drone(&mut self, position: Point, drone: TransportableDrones) {
        //let unit = self.get_map().get_unit(position).expect(&format!("Missing unit at {:?}", position));
        self.add_event(Event::BuildDrone(position, drone));
    }*/

    /*pub fn unit_exhaust(&mut self, position: Point) {
        let unit = self.get_map().get_unit(position).expect(&format!("Missing unit at {:?}", position));
        if !unit.is_exhausted() {
            self.add_event(Event::UnitExhaust(position));
        } else {
            panic!("Unit at {position:?} is already exhausted!");
        }
    }

    pub fn unit_unexhaust(&mut self, position: Point) {
        let unit = self.get_map().get_unit(position).expect(&format!("Missing unit at {:?}", position));
        if unit.is_exhausted() {
            self.add_event(Event::UnitExhaust(position));
        } else {
            panic!("Unit at {position:?} isn't exhausted!");
        }
    }

    pub fn unit_exhaust_boarded(&mut self, position: Point, index: UnloadIndex) {
        let unit = self.get_map().get_unit(position).expect(&format!("Missing unit at {:?}", position));
        if unit.get_boarded().len() > *index as usize && !unit.get_boarded()[*index as usize].data.exhausted {
            self.add_event(Event::UnitExhaustBoarded(position, index));
        } else {
            panic!("Can't exhaust unit at {position:?}, boarded as {index}");
        }
    }

    pub fn unit_unexhaust_boarded(&mut self, position: Point, index: UnloadIndex) {
        let unit = self.get_map().get_unit(position).expect(&format!("Missing unit at {:?}", position));
        if unit.get_boarded().len() > *index as usize && unit.get_boarded()[*index as usize].data.exhausted {
            self.add_event(Event::UnitExhaustBoarded(position, index));
        } else {
            panic!("Can't unexhaust unit at {position:?}, boarded as {index}");
        }
    }*/

    pub fn unit_damage(&mut self, position: Point, damage: u16) {
        let unit = self.get_map().get_unit(position).expect(&format!("Missing unit at {:?}", position));
        self.add_event(Event::UnitHpChange(position, (-(damage.min(unit.get_hp() as u16) as i32)).into(), (-(damage as i32)).max(-999).into()));
    }

    pub fn unit_mass_damage(&mut self, amounts: &HashMap<Point, u16>) {
        //let mut list = Vec::new();
        for (position, damage) in amounts {
            /*let unit = self.get_map().get_unit(position).expect(&format!("Missing unit at {:?}", position));
            let damage = -(damage as i32);
            list.push((position, damage.max(-(unit.get_hp() as i32)).into(), damage.max(-999).into()));*/
            self.unit_damage(*position, *damage);
        }
        //self.add_event(Event::UnitMassHpChange(list.try_into().unwrap()));
    }

    pub fn unit_repair(&mut self, position: Point, heal: u8) {
        let hp = self.get_map().get_unit(position).expect(&format!("Missing unit at {:?}", position)).get_hp();
        self.effect_repair(position);
        self.add_event(Event::UnitHpChange(position, (heal.min(100 - hp) as i32).into(), (heal.min(100) as i32).into()));
    }

    pub fn unit_heal(&mut self, position: Point, heal: u8) {
        let hp = self.get_map().get_unit(position).expect(&format!("Missing unit at {:?}", position)).get_hp();
        self.effect_heal(position);
        self.add_event(Event::UnitHpChange(position, (heal.min(100 - hp) as i32).into(), (heal.min(100) as i32).into()));
    }

    pub fn unit_mass_heal(&mut self, amounts: HashMap<Point, u8>) {
        for (position, damage) in amounts {
            self.unit_heal(position, damage);
        }
    }

    pub fn unit_heal_boarded(&mut self, position: Point, index: usize, heal: u8) {
        let unit = self.get_map().get_unit(position).expect(&format!("Missing unit at {:?}", position));
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
        let unit = self.get_map().get_unit(position).expect(&format!("Missing unit at {:?}", position));
        let transported = unit.get_transported();
        if transported.len() > index {
            let hp = transported[index].get_hp();
            if hp > 0 {
                self.add_event(Event::UnitHpChangeBoarded(position, index.into(), (-(damage.min(hp) as i32)).into()));
            }
        } else {
            panic!("Can't damage unit at {position:?}, boarded as {index}");
        }
    }

    pub fn unit_death(&mut self, position: Point) {
        self.add_event(Event::Effect(Effect::Explode(position)));
        let unit = self.get_map().get_unit(position).expect(&format!("Missing unit at {:?}", position)).clone();
        self.remove_observed_units_at(position);
        self.add_event(Event::UnitRemove(position, unit.clone()));
    }

    pub fn unit_death_boarded(&mut self, position: Point, index: usize) {
        self.add_event(Event::Effect(Effect::Explode(position)));
        let unit = self.game.get_map().get_unit(position).expect(&format!("Missing unit at {:?}", position));
        let transported = unit.get_transported();
        if transported.len() > index {
            if let Some((id, _)) = self.observation_id(position, Some(index)) {
                self.observed_units.remove(&id);
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
        let unit = self.get_map().get_unit(position).expect(&format!("Missing unit at {:?}", position));
        self.add_event(Event::UnitRemove(position, unit.clone()));
        self.add_event(Event::UnitAdd(position, new_unit));
    }

    pub fn effect_kraken_rage(&mut self, position: Point) {
        self.add_event(Event::Effect(Effect::KrakenRage(position)))
    }


    pub fn trigger_all_unit_scripts<S>(
        &mut self,
        get_script: impl Fn(&Game<D>, &Unit<D>, Point, Option<(&Unit<D>, usize)>, &[(Unit<D>, Hero, Point, Option<usize>)]) -> Vec<S>,
        before_executing: impl FnOnce(&mut Self),
        execute_script: impl Fn(&mut Self, Vec<S>, Point, &Unit<D>, usize),
    ) {
        let hero_auras = Hero::map_influence(self.get_game(), -1);
        let mut scripts = Vec::new();
        for p in self.get_map().all_points() {
            if let Some(unit) = self.get_map().get_unit(p).cloned() {
                let heroes = hero_auras.get(&(p, unit.get_owner_id())).map(|h| h.as_slice()).unwrap_or(&[]);
                let script = get_script(self.get_game(), &unit, p, None, heroes);
                if script.len() > 0 {
                    let id = self.observe_unit(p, None).0;
                    scripts.push((script, unit.clone(), p, id));
                }
                for (i, u) in unit.get_transported().iter().enumerate() {
                    let script = get_script(self.get_game(), u, p, Some((&unit, i)), heroes);
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


    pub fn accept(mut self) -> Events<Game<D>> {
        if self.events.get(&IPerspective::Server) == self.events.get(&IPerspective::Neutral) {
            // if no info is hidden, there's no need to store multiple identical entries
            let events = self.events.remove(&IPerspective::Server).unwrap();
            let bytes = Event::export_list(&events, self.environment());
            Events::Public(events, bytes)
        } else {
            let environment = self.game.environment();
            Events::Secrets(self.events.into_iter()
            .map(|(perspective, events)| {
                let bytes = Event::export_list(&events, environment);
                (perspective, (events, bytes))
            }).collect())
        }
    }

    pub fn cancel(mut self) {
        while let Some(event) = self.events.get_mut(&IPerspective::Server).unwrap().pop() {
            event.undo(&mut self.game);
        }
    }
}


