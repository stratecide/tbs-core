use std::error::Error;
use std::fmt::Display;

use interfaces::GameInterface;
use rustc_hash::FxHashSet;
use semver::Version;
use zipper_derive::Zippable;
use zipper::*;
use rhai::*;

use crate::config::environment::Environment;
use crate::handle::Handle;
use crate::map::point::Point;
use crate::script::custom_action::*;
use crate::script::*;
use crate::map::direction::Direction;
use crate::script::executor::Executor;
use crate::units::commands::{UnitCommand, MAX_CUSTOM_ACTION_STEPS};
use crate::units::hero::Hero;
use crate::VERSION;
use super::event_handler::EventHandler;
use super::fog::FogIntensity;
use super::game::Game;
use super::game_view::GameView;

#[derive(Debug, Clone, PartialEq, Zippable)]
#[zippable(bits = 4, support_ref = Environment)]
pub enum Command<D: Direction> {
    EndTurn,
    UnitCommand(UnitCommand<D>),
    TerrainAction(Point, LVec<CustomActionInput<D>, {MAX_CUSTOM_ACTION_STEPS}>),
    TokenAction(Point, LVec<CustomActionInput<D>, {MAX_CUSTOM_ACTION_STEPS}>),
    CommanderPower(CommanderPowerIndex, LVec<CustomActionInput<D>, {MAX_CUSTOM_ACTION_STEPS}>),
}

impl<D: Direction> Command<D> {
    pub fn commander_power(index: usize, custom_action_data: Vec<CustomActionInput<D>>) -> Self {
        Self::CommanderPower(CommanderPowerIndex(index), custom_action_data.try_into().unwrap())
    }

    pub fn execute(self, handler: &mut EventHandler<D>) -> Result<(), CommandError> {
        match self {
            Self::EndTurn => {
                handler.end_turn();
                Ok(())
            }
            Self::UnitCommand(command) => command.execute(handler),
            Self::TerrainAction(pos, data) => {
                let team = handler.get_game().current_team();
                if let Some(err) = handler.with_game(|game| {
                    if !game.get_map().is_point_valid(pos) {
                        return Some(CommandError::InvalidPoint(pos));
                    }
                    if game.get_fog_at(team, pos) != FogIntensity::TrueSight {
                        // without TrueSight, a stealthed unit could block this field
                        return Some(CommandError::NoVision);
                    }
                    if game.get_map().get_unit(pos).is_some() {
                        return Some(CommandError::Blocked(pos));
                    }
                    None
                }) {
                    return Err(err);
                }
                let terrain = handler.get_game().get_terrain(pos).unwrap();
                if terrain.get_owner_id() != handler.get_game().current_owner() {
                    return Err(CommandError::NotYourProperty);
                }
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
                // check whether the player should even be able to send this command
                let (script, data) = {
                    // making sure i don't accidently change anything while testing move validity
                    #[allow(unused_variables)]
                    let handler = ();
                    let heroes = Hero::hero_influence_at(client, pos, client.current_owner());
                    let (input_script, script) = client.with(|game| {
                        for token in game.get_map().get_tokens(pos) {
                            if game.environment().config.token_action_script(token.typ()).is_some() {
                                return Err(CommandError::Blocked(pos));
                            }
                        }
                        game.environment().config.terrain_action_script(client, pos, &terrain, &heroes)
                        .ok_or(CommandError::InvalidAction)
                    })?;
                    let Some(data) = is_terrain_script_input_valid(input_script, client, pos, terrain.clone(), &data) else {
                        return Err(CommandError::InvalidAction);
                    };
                    (script, data)
                };
                drop(borrowed_game);
                execute_terrain_script(script, handler, pos, terrain, data);
                Ok(())
            }
            Self::TokenAction(pos, data) => {
                let team = handler.get_game().current_team();
                if let Some(err) = handler.with_game(|game| {
                    if !game.get_map().is_point_valid(pos) {
                        return Some(CommandError::InvalidPoint(pos));
                    }
                    if game.get_fog_at(team, pos) != FogIntensity::TrueSight {
                        // without TrueSight, a stealthed unit could block this field
                        return Some(CommandError::NoVision);
                    }
                    if game.get_map().get_unit(pos).is_some() {
                        return Some(CommandError::Blocked(pos));
                    }
                    None
                }) {
                    return Err(err);
                }
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
                // check whether the player should even be able to send this command
                let (script, token, data) = {
                    // making sure i don't accidently change anything while testing move validity
                    #[allow(unused_variables)]
                    let handler = ();
                    client.with(|game| {
                        let mut script_data = None;
                        // look from top to bottom
                        for token in game.get_map().get_tokens(pos).iter()
                        .rev() {
                            if let Some((input_script, script)) = game.environment().config.token_action_script(token.typ()) {
                                if token.get_owner_id() != game.current_player().get_owner_id() {
                                    return Err(CommandError::Blocked(pos));
                                }
                                script_data = is_token_script_input_valid(input_script, client, pos, token.clone(), &data)
                                .map(|data| (script, token.clone(), data));
                                break;
                            }
                        }
                        script_data.ok_or(CommandError::InvalidAction)
                    })?
                };
                drop(borrowed_game);
                execute_token_script(script, handler, pos, token, data);
                Ok(())
            }
            Self::CommanderPower(index, data) => {
                let owner_id = handler.get_game().current_owner();
                let player = handler.get_game().get_owning_player(owner_id).unwrap();
                let commander = player.commander;
                if !commander.can_activate_power(index.0, false) {
                    return Err(CommandError::PowerNotUsable);
                }
                let script = commander.power_activation_script(index.0);
                let data = if let Some((Some(input_script), _)) = script {
                    is_commander_script_input_valid(input_script, &*handler.get_game(), &data)
                    .ok_or(CommandError::PowerNotUsable)?
                } else if data.len() == 0 {
                    Vec::new()
                } else {
                    return Err(CommandError::PowerNotUsable);
                };
                handler.add_commander_charge(owner_id, -(commander.power_cost(index.0) as i32));
                handler.commander_power(owner_id, index.0);
                if let Some((input_script, function_index)) = script {
                    let data = input_script.map(|_| data);
                    execute_commander_script(function_index, handler, data);
                }
                Ok(())
            }
        }?;
        cleanup_dead_material(handler, true);
        Ok(())
    }
}

pub fn cleanup_dead_material<D: Direction>(handler: &mut EventHandler<D>, execute_scripts: bool) {
    // destroy units that are now dead
    let all_points = handler.with_map(|map| map.all_points());
    let environment = handler.get_game().environment();
    let is_unit_dead_rhai = environment.is_unit_dead_rhai();
    let engine = environment.get_engine::<D>();
    let executor = Executor::new(engine, Scope::new(), environment);
    for _ in 0..100 {
        let deaths: FxHashSet<Point> = all_points.iter().cloned()
        .filter(|p| {
            handler.get_game().get_unit(*p).map(|u| {
                match executor.run(is_unit_dead_rhai, (u,)) {
                    Ok(result) => result,
                    Err(e) => {
                        // TODO: log error
                        println!("unit is_unit_dead_rhai {is_unit_dead_rhai}: {e:?}");
                        false
                    }
                }
            }).unwrap_or(false)
        })
        .collect();
        if execute_scripts {
            handler.trigger_all_unit_scripts(
                |game, unit, unit_pos, transporter, heroes| {
                    if deaths.contains(&unit_pos) {
                        unit.on_death(game, unit_pos, transporter, None, heroes, &[])
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
                        scope.push_constant(CONST_NAME_OTHER_POSITION, ());
                        scope.push_constant(CONST_NAME_OTHER_UNIT, ());
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
        // check if a player lost
        let viable_player_ids = handler.with_map(|map| map.get_viable_player_ids(&*handler.get_game()));
        let players: Vec<u8> = handler.get_game().players().iter()
        .filter(|p| !p.dead)
        .map(|p| p.color_id)
        .collect();
        let mut no_player_died = true;
        for owner_id in players {
            if !viable_player_ids.contains(&owner_id) {
                no_player_died = false;
                handler.player_dies(owner_id as i8);
            }
        }
        handler.recalculate_fog();
        if deaths.len() == 0 && no_player_died {
            break;
        }
    }
}
#[derive(Debug, Clone, PartialEq)]
pub struct CommanderPowerIndex(pub usize);

impl SupportedZippable<&Environment> for CommanderPowerIndex {
    fn export(&self, zipper: &mut Zipper, support: &Environment) {
        let max_len = support.config.commander_types().iter()
            .map(|co| support.config.commander_powers(*co).len())
            .max()
            .unwrap_or(0) as u32;
        let bits = bits_needed_for_max_value(max_len.max(1) - 1);
        zipper.write_u32(self.0 as u32, bits);
    }

    fn import(unzipper: &mut Unzipper, support: &Environment) -> Result<Self, ZipperError> {
        let max_len = support.config.commander_types().iter()
            .map(|co| support.config.commander_powers(*co).len())
            .max()
            .unwrap_or(0) as u32;
        let bits = bits_needed_for_max_value(max_len.max(1) - 1);
        Ok(Self(unzipper.read_u32(bits)? as usize))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandError {
    NoVision,
    MissingUnit,
    MissingBoardedUnit,
    NotYourUnit,
    UnitCannotMove,
    UnitCannotCapture,
    UnitCannotBeBoarded,
    UnitCannotPull,
    UnitTypeWrong,
    InvalidPath,
    InvalidPoint(Point),
    InvalidTarget,
    InvalidUnitType,
    InvalidAction,
    PowerNotUsable,
    Blocked(Point),
    NotEnoughMoney,
    NotYourProperty,
    BuildLimitReached,
    CannotCaptureHere,
    InvalidCommanderPower,
    NotEnoughCharge,
    CannotRepairHere,
    CannotBuildHere,
}

impl Display for CommandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl Error for CommandError {}
