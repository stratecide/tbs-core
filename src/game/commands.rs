use std::error::Error;
use std::fmt::Display;

use interfaces::{ClientPerspective, GameInterface};
use rustc_hash::FxHashSet;
use zipper_derive::Zippable;
use zipper::*;
use rhai::*;

use crate::config::environment::Environment;
use crate::map::board::{Board, BoardView};
use crate::map::map::valid_points;
use crate::map::point::Point;
use crate::script::custom_action::*;
use crate::script::*;
use crate::map::direction::Direction;
use crate::script::executor::Executor;
use crate::units::commands::{UnitCommand, MAX_CUSTOM_ACTION_STEPS};
use crate::units::hero::Hero;
use super::event_handler::EventHandler;
use super::fog::FogIntensity;

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
                if !handler.get_game().get_map().is_point_valid(pos) {
                    return Err(CommandError::InvalidPoint(pos));
                }
                if handler.get_game().get_fog_at(team, pos) != FogIntensity::TrueSight {
                    // without TrueSight, a stealthed unit could block this field
                    return Err(CommandError::NoVision);
                }
                if handler.get_game().get_map().get_unit(pos).is_some() {
                    return Err(CommandError::Blocked(pos));
                }
                let terrain = handler.get_game().get_terrain(pos).unwrap().clone();
                if terrain.get_owner_id() != handler.get_game().current_owner() {
                    return Err(CommandError::NotYourProperty);
                }
                let borrowed_game = handler.get_game();
                let client_game;
                let client = if handler.get_game().has_secrets() {
                    client_game = handler.get_game().reimport_as_client(ClientPerspective::Team(handler.get_game().current_owner() as u8));
                    &client_game
                } else {
                    &*borrowed_game
                };
                let board = Board::from(client);
                // check whether the player should even be able to send this command
                let (script, data) = {
                    // making sure i don't accidently change anything while testing move validity
                    #[allow(unused_variables)]
                    let handler = ();
                    let heroes = Hero::hero_influence_at(&board, pos, Some(client.current_owner()));
                    let (input_script, script) = {
                        for token in client.get_tokens(pos) {
                            if client.environment().config.token_action_script(token.typ()).is_some() {
                                return Err(CommandError::Blocked(pos));
                            }
                        }
                        client.environment().config.terrain_action_script(&board, pos, &terrain, &heroes)
                        .ok_or(CommandError::InvalidAction)
                    }?;
                    let Some(data) = is_terrain_script_input_valid(input_script, &board, pos, terrain.clone(), &data) else {
                        return Err(CommandError::InvalidAction);
                    };
                    (script, data)
                };
                execute_terrain_script(script, handler, pos, terrain, data);
                Ok(())
            }
            Self::TokenAction(pos, data) => {
                let team = handler.get_game().current_team();
                if !handler.get_game().get_map().is_point_valid(pos) {
                    return Err(CommandError::InvalidPoint(pos));
                }
                if handler.get_game().get_fog_at(team, pos) != FogIntensity::TrueSight {
                    // without TrueSight, a stealthed unit could block this field
                    return Err(CommandError::NoVision);
                }
                if handler.get_game().get_unit(pos).is_some() {
                    return Err(CommandError::Blocked(pos));
                }
                let borrowed_game = handler.get_game();
                let client_game;
                let client = if handler.get_game().has_secrets() {
                    client_game = handler.get_game().reimport_as_client(ClientPerspective::Team(handler.get_game().current_owner() as u8));
                    &client_game
                } else {
                    &*borrowed_game
                };
                // check whether the player should even be able to send this command
                let (script, token, data) = {
                    // making sure i don't accidently change anything while testing move validity
                    #[allow(unused_variables)]
                    let handler = ();
                    let mut script_data = None;
                    // look from top to bottom
                    for token in client.get_tokens(pos).iter()
                    .rev() {
                        if let Some((input_script, script)) = client.environment().config.token_action_script(token.typ()) {
                            if token.get_owner_id() != client.current_player().get_owner_id() {
                                return Err(CommandError::Blocked(pos));
                            }
                            let board = Board::from(client);
                            script_data = is_token_script_input_valid(input_script, &board, pos, token.clone(), &data)
                            .map(|data| (script, token.clone(), data));
                            break;
                        }
                    }
                    script_data.ok_or(CommandError::InvalidAction)?
                };
                execute_token_script(script, handler, pos, token, data);
                Ok(())
            }
            Self::CommanderPower(index, data) => {
                let owner_id = handler.get_game().current_owner();
                let player = handler.get_game().get_owning_player(owner_id).unwrap();
                let commander = &player.commander;
                if !commander.can_activate_power(index.0, false) {
                    return Err(CommandError::PowerNotUsable);
                }
                let script = commander.power_activation_script(index.0);
                let data = if let Some((Some(input_script), _)) = script {
                    is_commander_script_input_valid(input_script, handler.get_board(), &data)
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
    let all_points = valid_points(handler.get_game());
    let environment = handler.environment().clone();
    let is_unit_dead_rhai = environment.is_unit_dead_rhai();
    let executor = Executor::new(Scope::new(), environment.clone());
    for _ in 0..100 {
        let deaths: FxHashSet<Point> = all_points.iter().cloned()
        .filter(|p| {
            handler.get_game().get_unit(*p).cloned().map(|u| {
                match executor.run::<D, bool>(is_unit_dead_rhai, (u,)) {
                    Ok(result) => result,
                    Err(e) => {
                        environment.log_rhai_error("cleanup_dead_material::is_unit_dead_rhai", environment.get_rhai_function_name(is_unit_dead_rhai), &e);
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
                        let environment = handler.environment().clone();
                        let executor = handler.executor(scope);
                        for function_index in scripts {
                            match executor.run::<D, ()>(function_index, ()) {
                                Ok(()) => (),
                                Err(e) => {
                                    environment.log_rhai_error("cleanup_dead_material::OnDeath", environment.get_rhai_function_name(function_index), &e);
                                }
                            }
                        }
                    }
                }
            );
        }
        // check if a player lost
        let viable_player_ids = handler.get_game().get_map().get_viable_player_ids();
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
