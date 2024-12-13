use executor::Executor;
use interfaces::GameInterface;
use rhai::Scope;
use rustc_hash::FxHashSet as HashSet;
use std::fmt;

use num_rational::Rational32;
use semver::Version;
use zipper::*;
use zipper_derive::Zippable;

use crate::config::environment::Environment;
use crate::config::movement_type_config::MovementPattern;
use crate::game::commands::*;
use crate::game::event_handler::*;
use crate::game::game_view::GameView;
use crate::handle::Handle;
use crate::map::direction::Direction;
use crate::map::point::Point;
use crate::game::game::Game;
use crate::script::custom_action::*;
use crate::script::*;
use crate::VERSION;

use super::combat::*;
use super::hero::*;
use super::movement::*;
use super::unit::Unit;

pub const UNIT_REPAIR: u32 = 30;
pub const MAX_CUSTOM_ACTION_STEPS: u32 = 8;

#[derive(Debug, Clone, PartialEq, Zippable)]
#[zippable(bits=5, support_ref = Environment)]
pub enum UnitAction<D: Direction> {
    Wait,
    Take,
    Enter,
    Attack(AttackVector<D>),
    HeroPower(HeroPowerIndex, LVec<CustomActionInput<D>, {MAX_CUSTOM_ACTION_STEPS}>),
    Custom(CustomActionIndex, LVec<CustomActionInput<D>, {MAX_CUSTOM_ACTION_STEPS}>),
}

impl<D: Direction> fmt::Display for UnitAction<D> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Wait => write!(f, "Wait"),
            Self::Take => write!(f, "Take"),
            Self::Enter => write!(f, "Enter"),
            Self::Attack(p) => write!(f, "Attack {:?}", p),
            Self::HeroPower(index, _) => write!(f, "Hero Power {}", index.0),
            Self::Custom(index, _) => write!(f, "Custom {}", index.0),
        }
    }
}

impl<D: Direction> UnitAction<D> {
    pub fn custom(index: usize, custom_action_data: Vec<CustomActionInput<D>>) -> Self {
        Self::Custom(CustomActionIndex(index), custom_action_data.try_into().unwrap())
    }

    pub fn hero_power(index: usize, custom_action_data: Vec<CustomActionInput<D>>) -> Self {
        Self::HeroPower(HeroPowerIndex(index), custom_action_data.try_into().unwrap())
    }

    pub fn build_action_data_if_valid(&self, game: &Handle<Game<D>>, unit: &Unit<D>, path: &Path<D>, _destination: Point, transporter: Option<(&Unit<D>, usize)>, ballast: &[TBallast<D>]) -> Option<Vec<CustomActionData<D>>> {
        let options = unit.options_after_path(game, path, transporter, ballast);
        match self {
            Self::HeroPower(index, data) => {
                if !options.contains(&Self::hero_power(index.0, Vec::new())) {
                    return None;
                }
                let Some(hero) = unit.get_hero() else {
                    return None;
                };
                let environment = game.environment();
                let power = &environment.config.hero_powers(hero.typ())[index.0];
                if let Some((Some(input_script), _)) = power.script {
                    return is_unit_script_input_valid(input_script, game, path, transporter.map(|(_, i)| i), data);
                } else if data.len() == 0 {
                    return Some(Vec::new())
                }
            }
            Self::Custom(index, data) => {
                if !options.contains(&Self::custom(index.0, Vec::new())) {
                    return None;
                }
                let environment = game.environment();
                let custom_action = &environment.config.custom_actions()[index.0];
                if let Some(input_script) = custom_action.script.0 {
                    return is_unit_script_input_valid(input_script, game, path, transporter.map(|(_, i)| i), data);
                } else if data.len() == 0 {
                    return Some(Vec::new())
                }
            }
            _ => {
                if options.contains(self) {
                    return Some(Vec::new())
                }
            }
        }
        // invalid option chosen
        None
    }

    pub fn execute(&self, handler: &mut EventHandler<D>, unit_id: usize, end: Point, path: &Path<D>, transporter: Option<(&Unit<D>, usize)>, ballast: &[TBallast<D>], action_data: Vec<CustomActionData<D>>) {
        let owner_id = handler.get_game().current_owner();
        let needs_to_exhaust = match self {
            Self::Wait => true,
            Self::Enter => true,
            Self::Take => {
                let attacker = handler.get_game().get_unit(end).unwrap();
                if attacker.movement_pattern() == MovementPattern::Pawn {
                    let mut deaths = HashSet::default();
                    for dp in handler.get_game().all_points() {
                        if let Some(u) = handler.get_game().get_unit(dp) {
                            if attacker.could_take(&u, PathStepTakes::Allow) && u.get_en_passant() == Some(end) {
                                deaths.insert(dp);
                            }
                        }
                    }
                    handler.trigger_all_unit_scripts(
                        |game, unit, unit_pos, transporter, heroes| {
                            if deaths.contains(&unit_pos) {
                                unit.on_death(game, unit_pos, transporter, Some((&attacker, end)), heroes, &[])
                            } else {
                                Vec::new()
                            }
                        },
                        |handler| handler.unit_mass_death(&deaths),
                        |handler, scripts, unit_pos, unit, _observation_id| {
                            if scripts.len() > 0 {
                                let mut scope = Scope::new();
                                scope.push_constant(CONST_NAME_POSITION, unit_pos);
                                scope.push_constant(CONST_NAME_UNIT, unit.clone());
                                scope.push_constant(CONST_NAME_OTHER_POSITION, end);
                                scope.push_constant(CONST_NAME_OTHER_UNIT, attacker.clone());
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
                    );
                }
                true
            }
            Self::Attack(attack_vector) => {
                let attacker = handler.get_game().get_unit(end).unwrap();
                let transporter = transporter.map(|(u, _)| (u, path.start));
                attack_vector.execute(
                    handler,
                    end,
                    attacker,
                    Some(unit_id),
                    Some((path, transporter, ballast)),
                    true,
                    true,
                    true,
                    Rational32::from_integer(1),
                    Counter::AllowCounter,
                );
                true
            }
            Self::HeroPower(index, _) => {
                let unit = handler.get_game().get_unit(end).unwrap().clone();
                let hero = unit.get_hero().unwrap();
                let config = handler.environment().config.clone();
                let power = &config.hero_powers(hero.typ())[index.0];
                handler.hero_charge_sub(end, None, power.required_charge.into());
                handler.hero_power(end, index.0);
                let heroes = Hero::hero_influence_at(&*handler.get_game(), end, unit.get_owner_id());
                if let Some((input_script, function_index)) = power.script {
                    let action_data = input_script.map(|_| action_data);
                    execute_unit_script(function_index, handler, &unit, path, end, transporter, &heroes, ballast, action_data);
                }
                false
            }
            Self::Custom(index, _) => {
                let unit = handler.get_game().get_unit(end).unwrap();
                let config = handler.environment().config.clone();
                let custom_action = &config.custom_actions()[index.0];
                let heroes = Hero::hero_influence_at(&*handler.get_game(), end, unit.get_owner_id());
                let action_data = custom_action.script.0.map(|_| action_data);
                execute_unit_script(custom_action.script.1, handler, &unit, path, end, transporter, &heroes, ballast, action_data);
                false
            }
        };
        if needs_to_exhaust {
            let heroes = Hero::hero_influence_at(&*handler.get_game(), end, owner_id);
            handler.on_unit_normal_action(unit_id, path.clone(), false, &heroes, ballast);
        }
    }
}

#[derive(Debug, Clone, PartialEq, Zippable)]
#[zippable(support_ref = Environment)]
pub struct UnitCommand<D: Direction> {
    pub unload_index: Option<UnloadIndex>,
    pub path: Path<D>,
    pub action: UnitAction<D>,
}

impl<D: Direction> UnitCommand<D> {
    pub fn execute(self, handler: &mut EventHandler<D>) -> Result<(), CommandError> {
        let borrowed_game = handler.get_game();
        let client_game;
        let client = if handler.get_game().is_foggy() {
            let player = handler.get_game().current_owner() as u8;
            let data = handler.get_game().export();
            let secret = data.hidden
            .and_then(|mut h| h.teams.remove(&player))
            .map(|h| (player, h));
            let version = Version::parse(VERSION).unwrap();
            client_game = Handle::new(*Game::import_client(data.public, secret, &handler.environment().config, version).unwrap());
            &client_game
        } else {
            &*borrowed_game
        };
        let board_at_the_end = self.action == UnitAction::Enter;
        let start = self.path.start;
        // check whether the player should even be able to send this command
        let action_data = {
            // making sure i don't accidently change anything while testing move validity
            #[allow(unused_variables)]
            let handler = ();
            if !client.with(|game| game.get_map().is_point_valid(start)) {
                return Err(CommandError::InvalidPoint(start));
            }
            let unit = client.get_unit(start).ok_or(CommandError::MissingUnit)?;
            let mut transporter = None;
            let unit = if let Some(index) = self.unload_index {
                transporter = Some((&unit, index.0));
                let boarded = unit.get_transported();
                boarded.get(index.0).ok_or(CommandError::MissingBoardedUnit)?.clone()
            } else {
                unit
            };
            if client.current_owner() != unit.get_owner_id() {
                return Err(CommandError::NotYourUnit);
            }
            if !unit.can_move(client, start) {
                return Err(CommandError::UnitCannotMove);
            }
            let ballast = search_path(client, &unit, &self.path, transporter, |path, p, can_stop_here, _| {
                if *path == self.path && board_at_the_end {
                    if let Some(transporter) = client.get_unit(p) {
                        if p != path.start && transporter.can_transport(&unit) {
                            return PathSearchFeedback::Found;
                        }
                    }
                } else if *path == self.path && !board_at_the_end && can_stop_here {
                    return PathSearchFeedback::Found;
                }
                PathSearchFeedback::Rejected
            }).ok_or(CommandError::InvalidPath)?.1;
            let destination = self.path.end(client).unwrap().0;
            let ballast = if self.path.len() == 0 {
                &[]
            } else {
                ballast.get_entries()
            };
            self.action.build_action_data_if_valid(client, &unit, &self.path, destination, transporter, ballast)
            .ok_or(CommandError::InvalidAction)?
        };
        drop(borrowed_game);

        // now we know that the player entered a valid command
        // check for fog trap
        let mut path_taken = self.path.clone();
        let mut fog_trap = None;
        let unit = handler.get_game().get_unit(start).unwrap();
        let mut transporter = None;
        let unit = if let Some(index) = self.unload_index {
            transporter = Some((&unit, index.0));
            let boarded = unit.get_transported();
            boarded.get(index.0).unwrap().clone()
        } else {
            unit
        };
        let unit_id = handler.observe_unit(start, self.unload_index.map(|ui| ui.0)).0;
        let mut ballast;
        loop {
            ballast = search_path(&*handler.get_game(), &unit, &path_taken, transporter, |path, p, can_stop_here, _| {
                if *path == path_taken && board_at_the_end {
                    if let Some(transporter) = handler.get_game().get_unit(p) {
                        if p != path.start && transporter.can_transport(&unit) {
                            return PathSearchFeedback::Found;
                        }
                    }
                } else if *path == path_taken && !board_at_the_end && can_stop_here {
                    return PathSearchFeedback::Found;
                }
                PathSearchFeedback::Rejected
            });
            if ballast.is_some() || path_taken.len() == 0 {
                break;
            } else {
                fog_trap = Some(path_taken.end(&*handler.get_game()).unwrap().0);
                path_taken.steps.pop();
            }
        }
        let ballast = ballast.expect(&format!("couldn't handle unit command {:?}", self)).1;
        let ballast = if path_taken.steps.len() > 0 {
            handler.unit_path(self.unload_index.map(|i| i.0), &path_taken, board_at_the_end, false);
            ballast.get_entries()
        } else {
            &[]
        };
        let end = path_taken.end(&*handler.get_game()).unwrap().0;
        if let Some(fog_trap) = fog_trap {
            // no event for the path is necessary if the unit is unable to move at all
            if path_taken.steps.len() > 0 {
                handler.unit_path(self.unload_index.map(|i| i.0), &path_taken, false, false);
            }
            // fog trap
            handler.effect_fog_surprise(fog_trap);
            let heroes = Hero::hero_influence_at(&*handler.get_game(), end, unit.get_owner_id());
            handler.on_unit_normal_action(unit_id, path_taken.clone(), true, &heroes, ballast);
        } else {
            // TODO: need to check whether action can really be executed
            // so far the code mainly checks whether it looks correct from the user perspective
            self.action.execute(handler, unit_id, end, &path_taken, transporter, ballast, action_data);
        }
        //exhaust_all_on_chess_board(handler, path_taken.start);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UnloadIndex(pub usize);

impl SupportedZippable<&Environment> for UnloadIndex {
    fn export(&self, zipper: &mut Zipper, support: &Environment) {
        zipper.write_u32(self.0 as u32, bits_needed_for_max_value(support.config.max_player_count() as u32));
    }
    fn import(unzipper: &mut Unzipper, support: &Environment) -> Result<Self, ZipperError> {
        Ok(Self(unzipper.read_u32(bits_needed_for_max_value(support.config.max_player_count() as u32))? as usize))
    }
}

impl From<usize> for UnloadIndex {
    fn from(value: usize) -> Self {
        Self(value)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CustomActionIndex(pub usize);

impl SupportedZippable<&Environment> for CustomActionIndex {
    fn export(&self, zipper: &mut Zipper, support: &Environment) {
        let len = support.config.custom_actions().len() as u32;
        let bits = bits_needed_for_max_value(len.max(1) - 1);
        zipper.write_u32(self.0 as u32, bits);
    }

    fn import(unzipper: &mut Unzipper, support: &Environment) -> Result<Self, ZipperError> {
        let len = support.config.custom_actions().len() as u32;
        let bits = bits_needed_for_max_value(len.max(1) - 1);
        Ok(Self(unzipper.read_u32(bits)? as usize))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct HeroPowerIndex(pub usize);

impl SupportedZippable<&Environment> for HeroPowerIndex {
    fn export(&self, zipper: &mut Zipper, support: &Environment) {
        let max_len = support.config.hero_types().iter()
            .map(|hero| support.config.hero_powers(*hero).len())
            .max()
            .unwrap_or(0) as u32;
        let bits = bits_needed_for_max_value(max_len.max(1) - 1);
        zipper.write_u32(self.0 as u32, bits);
    }

    fn import(unzipper: &mut Unzipper, support: &Environment) -> Result<Self, ZipperError> {
        let max_len = support.config.hero_types().iter()
            .map(|hero| support.config.hero_powers(*hero).len())
            .max()
            .unwrap_or(0) as u32;
        let bits = bits_needed_for_max_value(max_len.max(1) - 1);
        Ok(Self(unzipper.read_u32(bits)? as usize))
    }
}
