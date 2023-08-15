use std::collections::{HashMap, HashSet};

use interfaces::game_interface::{CommandInterface, EventInterface, Events, Perspective as IPerspective, ClientPerspective, GameInterface};
use zipper::*;
use zipper::zipper_derive::*;

use crate::commanders::{MAX_CHARGE, CommanderPower};
use crate::map::map::{Map, FieldData};
use crate::map::point::Point;
use crate::map::point_map::{self, MAX_AREA};
use crate::units::combat::WeaponType;
use crate::units::normal_units::{NormalUnits, NormalUnit, TransportableDrones, TransportedUnit, UnitData, DroneId, UnitActionStatus};
use crate::units::structures::{LASER_CANNON_RANGE, Structure, Structures};
use crate::{player::*, details};
use crate::terrain::{Terrain, BuiltThisTurn, Realty, CaptureProgress};
use crate::details::Detail;
use crate::units::*;
use crate::map::direction::Direction;
use crate::game::game::*;
use crate::units::mercenary::{Mercenaries, MaybeMercenary};
use crate::units::chess::*;
use crate::units::commands::{UnitCommand, UnloadIndex};
use crate::units::movement::{Path, PathStep, PathStepExt, MovementType};
use super::events::{Event, fog_change_index, Effect};

pub struct EventHandler<'a, D: Direction> {
    game: &'a mut Game<D>,
    events: HashMap<IPerspective, Vec<Event<D>>>,
    random: Box<dyn Fn() -> f32>,
}

impl<'a, D: Direction> EventHandler<'a, D> {
    pub fn new(game: &'a mut Game<D>, random: Box<dyn Fn() -> f32>) -> Self {
        let mut events = HashMap::new();
        events.insert(IPerspective::Server, vec![]);
        events.insert(IPerspective::Neutral, vec![]);
        for team in game.get_teams() {
            events.insert(IPerspective::Team(*team as u8), vec![]);
        }
        EventHandler {
            game,
            events,
            random,
        }
    }

    pub fn get_game(&self) -> &Game<D> {
        &self.game
    }

    pub fn get_map(&self) -> &Map<D> {
        self.game.get_map()
    }

    pub fn next_turn(&mut self) {
        self.add_event(Event::NextTurn);
        // update fog manually if it's random
        match self.get_game().get_fog_mode() {
            FogMode::Random(_, _, _, forecast) => {
                self.add_event(Event::RandomFogNextTurn(forecast[0]));
                FogMode::forecast(self);
            }
            _ => (),
        }
    }

    pub fn start_turn(&mut self, was_foggy: bool) {
        // hide / reveal player funds if fog started / ended
        if was_foggy != self.get_game().is_foggy() {
            let players: Vec<Owner> = self.game.players.iter().map(|player| player.owner_id).collect();
            if was_foggy {
                for player in players {
                    self.add_event(Event::PureRevealFunds(player));
                }
            } else {
                for player in players {
                    self.add_event(Event::PureHideFunds(player));
                }
            }
        }

        let owner_id = self.game.current_player().owner_id;
        // return drones to their origin if possible or destroy them
        let mut drone_parents: HashMap<DroneId, (Point, usize)> = self.get_map().all_points()
        .into_iter()
        .filter_map(|p| self.get_map().get_unit(p).and_then(|u| Some((p, u))))
        .filter(|(_, u)| u.get_owner() == Some(owner_id))
        .filter_map(|(p, unit)| match unit {
            UnitType::Normal(NormalUnit {typ: NormalUnits::DroneBoat(boarded, id), ..}) => {
                if boarded.remaining_capacity() > 0 {
                    Some((*id, (p, boarded.remaining_capacity())))
                } else {
                    None
                }
            }
            UnitType::Structure(Structure {typ: Structures::DroneTower(Some((_, boarded, id))), ..}) => {
                if boarded.remaining_capacity() > 0 {
                    Some((*id, (p, boarded.remaining_capacity())))
                } else {
                    None
                }
            }
            _ => None,
        }).collect();
        for p in self.get_map().all_points() {
            if let Some(unit) = self.get_map().get_unit(p) {
                if unit.get_owner() != Some(self.game.current_player().owner_id) {
                    continue;
                }
                match unit {
                    UnitType::Normal(NormalUnit {typ: NormalUnits::HeavyDrone(id), ..}) |
                    UnitType::Normal(NormalUnit {typ: NormalUnits::LightDrone(id), ..}) => {
                        if let Some((_destination, capacity)) = drone_parents.get_mut(id) {
                            // move drone back aboard its parent
                            // one less space in parent
                            if *capacity > 0 {
                                *capacity -= 1;
                            } else {
                                let id = *id;
                                drone_parents.remove(&id);
                            }
                        } else {
                            // no parent available, self-destruct
                            self.unit_death(p);
                        }
                    }
                    _ => (),
                }
            }
        }

        // has to be recalculated before structures, because the effects of some structures on
        // other players should maybe not be visible
        self.recalculate_fog(false);

        let income = (*self.game.current_player().income as isize * self.get_map().get_income_factor(self.game.current_player().owner_id)) as i32;
        if income != 0 {
            self.money_income(self.game.current_player().owner_id, income);
        }

        // fire structures
        for p in self.get_map().all_points() {
            if let Some(UnitType::Structure(structure)) = self.get_map().get_unit(p) {
                let structure = structure.clone();
                structure.start_turn(self, p);
            }
        }

        // structures may have destroyed some units
        self.recalculate_fog(false);
    }

    pub fn recalculate_fog(&mut self, keep_current_team: bool) {
        let mut perspectives: HashSet<Perspective> = self.game.get_teams().into_iter().map(|team| Some(team)).collect();
        if keep_current_team {
            perspectives.remove(&Some(self.game.current_player().team));
        }
        perspectives.insert(None);
        for team in perspectives {
            let perspective = to_client_perspective(&team);
            let fog = self.game.recalculate_fog(team);
            let mut changes = Vec::new();
            for p in self.get_map().all_points() {
                if let Some(index) = fog_change_index(self.game.get_vision(perspective, p), fog.get(&p).cloned()) {
                    changes.push((p, index));
                }
            }
            if changes.len() > 0 {
                self.change_fog(team, changes.try_into().unwrap());
            }
        }
    }

    // TODO: de-duplicate
    pub fn recalculate_fog_for(&mut self, team: Team) {
        let perspective = to_client_perspective(&Some(team));
        let fog = self.game.recalculate_fog(Some(team));
        let mut changes = Vec::new();
        for p in self.get_map().all_points() {
            if let Some(index) = fog_change_index(self.game.get_vision(perspective, p), fog.get(&p).cloned()) {
                changes.push((p, index));
            }
        }
        if changes.len() > 0 {
            self.change_fog(Some(team), changes.try_into().unwrap());
        }
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

    pub fn change_fog(&mut self, team: Perspective, changes: LVec<(Point, U<2>), {point_map::MAX_AREA}>) {
        self.add_event(Event::PureFogChange(team, changes));
    }

    pub fn fog_forecast(&mut self, value: bool, duration: usize) {
        self.add_event(Event::RandomFogForecast(value, duration.into()));
    }

    pub fn commander_charge_add(&mut self, owner: Owner, change: u32) {
        if let Some(player) = self.get_game().get_owning_player(owner) {
            let change = (change as i32).min(*player.commander.charge_potential());
            if change > 0 {
                self.add_event(Event::CommanderCharge(owner, change.into()));
            }
        }
    }

    pub fn commander_charge_sub(&mut self, owner: Owner, change: u32) {
        if let Some(player) = self.get_game().get_owning_player(owner) {
            let change = -(change as i32).min(*player.commander.charge());
            if change < 0 {
                self.add_event(Event::CommanderCharge(owner, change.into()));
            }
        }
    }

    pub fn commander_power_start(&mut self, owner: Owner) {
        if let Some(player) = self.get_game().get_owning_player(owner) {
            if !player.commander.power_active() {
                self.add_event(Event::CommanderFlipActiveSimple(owner));
            }
        }
    }

    pub fn commander_power_end(&mut self, owner: Owner) {
        if let Some(player) = self.get_game().get_owning_player(owner) {
            if player.commander.power_active() {
                self.add_event(Event::CommanderFlipActiveSimple(owner));
            }
        }
    }

    pub fn detail_add(&mut self, position: Point, detail: Detail) {
        let old_details = self.get_map().get_details(position);
        let mut details = old_details.clone();
        details.push(detail);
        self.add_event(Event::ReplaceDetail(position, old_details.try_into().unwrap(), Detail::correct_stack(details).try_into().unwrap()));
    }

    pub fn detail_remove(&mut self, position: Point, index: usize) {
        let details = self.get_map().get_details(position);
        if details.len() <= index {
            panic!("Missing Detail at {position:?}");
        } else {
            self.add_event(Event::RemoveDetail(position, index.into(), details[index].clone()));
        }
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

    pub fn mercenary_charge_add(&mut self, position: Point, change: u8) {
        self.mercenary_charge(position, change as i8)
    }

    pub fn mercenary_charge_sub(&mut self, position: Point, change: u8) {
        self.mercenary_charge(position, -(change as i8))
    }

    fn mercenary_charge(&mut self, position: Point, change: i8) {
        let mut unit = self.get_map().get_unit(position).expect(&format!("Missing unit at {:?}", position)).clone();
        if let UnitType::Normal(NormalUnit { data: UnitData { mercenary: MaybeMercenary::Some { mercenary, .. }, .. }, .. }) = &mut unit {
            let change = change.max(-(mercenary.charge() as i8)).min(mercenary.charge_potential() as i8);
            if change != 0 {
                mercenary.add_charge(change as i8);
                self.unit_replace(position, unit);
            }
        }
    }

    pub fn mercenary_power_start(&mut self, position: Point) {
        let mut unit  = self.get_map().get_unit(position).expect(&format!("Missing unit at {:?}", position)).clone();
        if let UnitType::Normal(NormalUnit { data: UnitData { mercenary: MaybeMercenary::Some { mercenary, .. }, .. }, .. }) = &mut unit {
            if let Some(power_active) = mercenary.power_active_mut() {
                if !*power_active {
                    *power_active = true;
                    self.unit_replace(position, unit);
                }
            }
        }
    }

    pub fn money_income(&mut self, owner: Owner, change: i32) {
        if change != 0 {
            // TODO: add effect depending on change < 0
            self.add_event(Event::MoneyChange(owner, change.into()));
        }
    }

    pub fn money_bonus(&mut self, owner: Owner, change: i32) {
        if change != 0 {
            // TODO: add effect depending on change < 0
            self.add_event(Event::MoneyChange(owner, change.into()));
        }
    }

    pub fn money_buy(&mut self, owner: Owner, cost: u32) {
        if cost > 0 {
            self.add_event(Event::MoneyChange(owner, (-(cost as i32)).into()));
        }
    }

    pub fn terrain_replace(&mut self, position: Point, terrain: Terrain<D>) {
        let old_terrain = self.get_map().get_terrain(position).expect(&format!("Missing terrain at {:?}", position));
        self.add_event(Event::TerrainChange(position, old_terrain.clone(), terrain));
    }

    pub fn terrain_capture_progress(&mut self, position: Point, progress: CaptureProgress) {
        let terrain = self.get_map().get_terrain(position).expect(&format!("Missing terrain at {:?}", position));
        if let Terrain::Realty(_, _, old_progress) = terrain {
            if *old_progress != progress {
                self.add_event(Event::CaptureProgress(position, *old_progress, progress));
            }
        }
    }

    pub fn terrain_built_this_turn(&mut self, position: Point, built_this_turn: BuiltThisTurn) {
        let terrain = self.get_map().get_terrain(position).expect(&format!("Missing terrain at {:?}", position));
        if let Terrain::Realty(realty, _, _) = terrain {
            match realty {
                Realty::Port(old) |
                Realty::Factory(old) |
                Realty::Airport(old) => {
                    if *old != built_this_turn {
                        self.add_event(Event::UpdateBuiltThisTurn(position, *old, built_this_turn));
                    }
                }
                _ => ()
            }
        }
    }

    pub fn unit_creation(&mut self, position: Point, unit: UnitType<D>) {
        if let ClientPerspective::Team(team) = unit.get_team(self.get_game()) {
            if self.get_game().is_foggy() && self.get_game().is_team_alive(&team.into()) {
                let unit_vision = unit.get_vision(self.get_game(), position);
                let perspective = to_client_perspective(&Some(team.into()));
                let vision_changes: Vec<(Point, U<2>)> = unit_vision.iter()
                .filter_map(|(p, v)| {
                    fog_change_index(self.get_game().get_vision(perspective, *p), Some(*v))
                        .and_then(|i| Some((*p, i)))
                })
                .collect();
                if vision_changes.len() > 0 {
                    self.change_fog(Some(team.into()), vision_changes.try_into().unwrap());
                }
            }
        }
        self.add_event(Event::UnitCreation(position, unit));
    }

    pub fn unit_path(&mut self, unload_index: Option<UnloadIndex>, path: &Path<D>, board_at_the_end: bool, involuntarily: bool) {
        let unit = self.get_map().get_unit(path.start).expect(&format!("Missing unit at {:?}", path.start)).clone();
        let unit_team = unit.get_team(self.get_game());
        let path_end = path.end(self.get_map()).unwrap();
        let mut was_hover_path = false;
        if !involuntarily {
            match &unit {
                UnitType::Normal(u) => {
                    let movement_type = u.get_movement(self.get_map().get_terrain(path.start).unwrap()).0;
                    match movement_type {
                        MovementType::Hover(hover_mode) => {
                            let steps = path.hover_steps(self.get_map(), hover_mode);
                            self.add_event(Event::HoverPath(Some(unload_index), path.start, steps, Some(board_at_the_end), unit.clone()));
                            was_hover_path = true;
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }
        if !was_hover_path {
            self.add_event(Event::UnitPath(Some(unload_index), path.clone(), Some(board_at_the_end), unit.clone()));
        }
        // update vision
        let player_team = self.get_game().current_player().team;
        if self.get_game().is_foggy() {
            if ClientPerspective::Team(*player_team as u8) == unit_team {
                let perspective = ClientPerspective::Team(*player_team as u8);
                let mut vision_changes = HashMap::new();
                let points = if unit.has_vision_from_path_intermediates() {
                    path.points(self.get_map()).unwrap().into_iter().skip(1).collect()
                } else {
                    vec![path_end]
                };
                for p in points {
                    for (p, vision) in unit.get_vision(self.get_game(), p) {
                        if vision == Vision::TrueSight && !self.get_game().has_true_sight_at(perspective, p)
                        || !self.get_game().has_vision_at(perspective, p) && !vision_changes.contains_key(&p) {
                            vision_changes.insert(p, vision);
                        }
                    }
                }
                let vision_changes: Vec<(Point, U<2>)> = vision_changes.into_iter()
                .filter_map(|(p, v)| 
                     fog_change_index(self.get_game().get_vision(perspective, p), Some(v))
                     .and_then(|vi| Some((p, vi))))
                .collect();
                if vision_changes.len() > 0 {
                    self.change_fog(Some(player_team), vision_changes.try_into().unwrap());
                }
            } else if let ClientPerspective::Team(team) = unit_team {
                self.recalculate_fog_for(team.into());
            }
        }
        // remove details the destroyed by the unit moving over them
        for p in path.points(self.get_map()).unwrap() {
            let old_details = self.get_map().get_details(p);
            let details: Vec<Detail> = old_details.clone().into_iter().filter(|detail| {
                match detail {
                    Detail::Coins1 => {
                        if let Some(owner) = unit.get_owner() {
                            if let Some(player) = self.get_game().get_owning_player(owner) {
                                self.money_bonus(owner, *player.income / 2);
                            }
                        }
                        false
                    }
                    Detail::Coins2 => {
                        if let Some(owner) = unit.get_owner() {
                            if let Some(player) = self.get_game().get_owning_player(owner) {
                                self.money_bonus(owner, *player.income);
                            }
                        }
                        false
                    }
                    Detail::Coins4 => {
                        if let Some(owner) = unit.get_owner() {
                            if let Some(player) = self.get_game().get_owning_player(owner) {
                                self.money_bonus(owner, *player.income * 2);
                            }
                        }
                        false
                    }
                    Detail::AirportBubble(owner) |
                    Detail::PortBubble(owner) |
                    Detail::FactoryBubble(owner) => {
                        Some(*owner) == unit.get_owner()
                    }
                    Detail::Skull(owner, _) => {
                        Some(*owner) == unit.get_owner()
                    }
                }
            }).collect();
            if details != old_details {
                self.add_event(Event::ReplaceDetail(p, old_details.try_into().unwrap(), details.try_into().unwrap()));
            }
        }
    }

    pub fn unit_moved_this_game(&mut self, position: Point) {
        let _ = self.get_map().get_unit(position).expect(&format!("Missing unit at {:?}", position));
        self.add_event(Event::UnitMovedThisGame(position));
    }

    pub fn unit_en_passant_opportunity(&mut self, position: Point) {
        //let _ = self.get_map().get_unit(position).expect(&format!("Missing unit at {:?}", position));
        self.add_event(Event::EnPassantOpportunity(position));
    }

    pub fn unit_direction(&mut self, position: Point, direction: D) {
        let unit = self.get_map().get_unit(position).expect(&format!("Missing unit at {:?}", position));
        match unit {
            UnitType::Chess(ChessUnit { typ: ChessUnits::Pawn(starting_dir, _), .. }) => {
                if *starting_dir != direction {
                    self.add_event(Event::UnitDirection(position, *starting_dir, direction));
                }
            }
            _ => panic!("unit at {position:?} doesn't have direction attribute"),
        }
    }

    pub fn unit_status(&mut self, position: Point, status: UnitActionStatus) {
        let unit = self.get_map().get_unit(position);
        match unit {
            Some(UnitType::Normal(unit)) => {
                if status != unit.action_status {
                    self.add_event(Event::UnitActionStatus(position, unit.action_status, status));
                }
            }
            None => panic!("Missing unit at {position:?}"),
            _ => panic!("unit at {position:?} can't have action status"),
        }
    }

    pub fn unit_build_drone(&mut self, position: Point, drone: TransportableDrones) {
        //let unit = self.get_map().get_unit(position).expect(&format!("Missing unit at {:?}", position));
        self.add_event(Event::BuildDrone(position, drone));
    }

    pub fn unit_exhaust(&mut self, position: Point) {
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
        if unit.get_boarded().len() < *index as usize || !unit.get_boarded()[*index as usize].data.exhausted {
            self.add_event(Event::UnitExhaustBoarded(position, index));
        } else {
            panic!("Can't exhaust unit at {position:?}, boarded as {index}");
        }
    }

    pub fn unit_unexhaust_boarded(&mut self, position: Point, index: UnloadIndex) {
        let unit = self.get_map().get_unit(position).expect(&format!("Missing unit at {:?}", position));
        if unit.get_boarded().len() < *index as usize || unit.get_boarded()[*index as usize].data.exhausted {
            self.add_event(Event::UnitExhaustBoarded(position, index));
        } else {
            panic!("Can't unexhaust unit at {position:?}, boarded as {index}");
        }
    }

    pub fn unit_damage(&mut self, position: Point, damage: u16) {
        let unit = self.get_map().get_unit(position).expect(&format!("Missing unit at {:?}", position));
        self.add_event(Event::UnitHpChange(position, (-(damage.min(unit.get_hp() as u16) as i8)).into(), (-(damage as i32)).max(-999).into()));
    }

    pub fn unit_mass_damage(&mut self, amounts: HashMap<Point, u16>) {
        //let mut list = Vec::new();
        for (position, damage) in amounts {
            /*let unit = self.get_map().get_unit(position).expect(&format!("Missing unit at {:?}", position));
            let damage = -(damage as i32);
            list.push((position, damage.max(-(unit.get_hp() as i32)).into(), damage.max(-999).into()));*/
            self.unit_damage(position, damage);
        }
        //self.add_event(Event::UnitMassHpChange(list.try_into().unwrap()));
    }

    pub fn unit_repair(&mut self, position: Point, heal: u8) {
        let hp = self.get_map().get_unit(position).expect(&format!("Missing unit at {:?}", position)).get_hp();
        self.effect_repair(position);
        self.add_event(Event::UnitHpChange(position, heal.min(100 - hp).into(), heal.into()));
    }

    pub fn unit_heal(&mut self, position: Point, heal: u8) {
        let hp = self.get_map().get_unit(position).expect(&format!("Missing unit at {:?}", position)).get_hp();
        self.effect_heal(position);
        self.add_event(Event::UnitHpChange(position, heal.min(100 - hp).into(), heal.into()));
    }

    pub fn unit_heal_boarded(&mut self, position: Point, index: usize, heal: u8) {
        let unit = self.get_map().get_unit(position).expect(&format!("Missing unit at {:?}", position));
        if unit.get_boarded().len() < index {
            let hp = unit.get_boarded()[index].get_hp();
            if hp < 100 {
                self.add_event(Event::UnitHpChangeBoarded(position, index.into(), heal.min(100 - hp).into()));
            }
        } else {
            panic!("Can't unexhaust unit at {position:?}, boarded as {index}");
        }
    }

    pub fn unit_death(&mut self, position: Point) {
        let unit = self.get_map().get_unit(position).expect(&format!("Missing unit at {:?}", position));
        self.add_event(Event::UnitDeath(position, unit.clone()));
    }

    pub fn unit_mass_death(&mut self, positions: HashSet<Point>) {
        // TODO: mass-effect
        for position in positions {
            self.unit_death(position);
        }
    }

    pub fn unit_replace(&mut self, position: Point, new_unit: UnitType<D>) {
        let unit = self.get_map().get_unit(position).expect(&format!("Missing unit at {:?}", position));
        self.add_event(Event::UnitReplacement(position, unit.clone(), new_unit));
    }

    pub fn accept(mut self) -> Events<Game<D>> {
        if self.events.get(&IPerspective::Server) == self.events.get(&IPerspective::Neutral) {
            // if no info is hidden, there's no need to store multiple identical entries
            Events::Public(self.events.remove(&IPerspective::Server).unwrap())
        } else {
            Events::Secrets(self.events)
        }
    }

    pub fn cancel(mut self) {
        while let Some(event) = self.events.get_mut(&IPerspective::Server).unwrap().pop() {
            event.undo(&mut self.game);
        }
    }
}


