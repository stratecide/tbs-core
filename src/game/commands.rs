use std::error::Error;
use std::fmt::Display;

use interfaces::GameInterface;
use semver::Version;
use zipper_derive::Zippable;
use zipper::*;

use crate::config::environment::Environment;
use crate::handle::Handle;
use crate::map::point::Point;
use crate::script::custom_action::*;
use crate::map::direction::Direction;
use crate::units::commands::{UnitCommand, MAX_CUSTOM_ACTION_STEPS};
use crate::units::hero::Hero;
use crate::VERSION;
use super::event_handler::EventHandler;
use super::fog::FogIntensity;
use super::game::Game;
use super::game_view::GameView;

#[derive(Debug, Clone, Zippable)]
#[zippable(bits = 4, support_ref = Environment)]
pub enum Command<D: Direction> {
    EndTurn,
    UnitCommand(UnitCommand<D>),
    TerrainAction(Point, LVec<CustomActionData<D>, {MAX_CUSTOM_ACTION_STEPS}>),
    //BuyUnit(Point, UnitType, D),
    CommanderPower(CommanderPowerIndex, LVec<CustomActionData<D>, {MAX_CUSTOM_ACTION_STEPS}>),
}

impl<D: Direction> Command<D> {
    pub fn commander_power(index: usize, custom_action_data: Vec<CustomActionData<D>>) -> Self {
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
                    // shouldn't need to clone here, actually
                    &*borrowed_game
                };
                // check whether the player should even be able to send this command
                let script = {
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
                    if !is_terrain_script_input_valid(input_script, client, pos, terrain.clone(), &data) {
                        return Err(CommandError::InvalidAction);
                    }
                    script
                };
                drop(borrowed_game);
                execute_terrain_script(script, handler, pos, terrain, &data);
                Ok(())
            }
            /*Self::BuyUnit(pos, unit_type, d) => {
                let owner_id = handler.get_game().current_owner();
                let player = handler.get_game().get_owning_player(owner_id).unwrap();
                if handler.with_game(|game| game.get_fog_at(player.get_team(), pos)) != FogIntensity::TrueSight {
                    // factories and bubbles provide true-sight
                    return Err(CommandError::NoVision);
                } else if handler.get_game().get_unit(pos).is_some() {
                    return Err(CommandError::Blocked(pos));
                }
                let mut bubble_index = None;
                let mut terrain = handler.get_game().get_terrain(pos).unwrap();
                for (index, det) in handler.get_game().get_tokens(pos).into_iter().enumerate() {
                    match det {
                        Detail::Bubble(owner, terrain_type) => {
                            bubble_index = Some(index);
                            terrain = TerrainBuilder::new(&handler.environment(), terrain_type)
                            .set_owner_id(owner.0)
                            .build_with_defaults();
                            break;
                        }
                        _ => (),
                    }
                }
                if terrain.get_owner_id() != owner_id {
                    return Err(CommandError::NotYourProperty);
                }
                let heroes = Hero::hero_influence_at(&*handler.get_game(), pos, owner_id);
                if !terrain.can_build(&*handler.get_game(), pos, &heroes) {
                    return Err(CommandError::CannotBuildHere);
                }
                // TODO: when checking the config, make sure that terrain-buildable units don't have a drone id
                if !terrain.buildable_units(&*handler.get_game(), pos, bubble_index.is_some(), &heroes).contains(&unit_type) {
                    return Err(CommandError::InvalidUnitType);
                }
                let built_this_turn = terrain.get_built_this_turn();
                if terrain.has_attribute(TerrainAttributeKey::BuiltThisTurn) && built_this_turn >= terrain.max_built_this_turn() {
                    return Err(CommandError::BuildLimitReached);
                }

                let (mut unit, cost) = terrain.unit_shop_option(&*handler.get_game(), pos, unit_type, &heroes);
                if cost > *player.funds {
                    return Err(CommandError::NotEnoughMoney)
                }
                handler.money_buy(owner_id, cost);
                if bubble_index != None {
                    unit.set_status(ActionStatus::Ready);
                }
                unit.set_direction(d);
                if handler.environment().unit_attributes(unit_type, owner_id).any(|a| *a == AttributeKey::DroneStationId) {
                    unit.set_drone_station_id(handler.get_game().with(|game| game.get_map().new_drone_id(handler.rng())));
                }
                handler.unit_creation(pos, unit);
                if let Some(bubble_index) = bubble_index {
                    handler.token_remove(pos, bubble_index);
                } else if terrain.has_attribute(TerrainAttributeKey::BuiltThisTurn) {
                    handler.terrain_built_this_turn(pos, built_this_turn + 1);
                }
                let function_indices = terrain.on_build(&*handler.get_game(), pos, bubble_index.is_some());
                if function_indices.len() > 0 {
                    let mut scope = Scope::new();
                    scope.push_constant(CONST_NAME_POSITION, pos);
                    scope.push_constant(CONST_NAME_TERRAIN, terrain);
                    let environment = handler.get_game().environment();
                    let engine = environment.get_engine_handler(handler);
                    let executor = Executor::new(engine, scope, environment);
                    for function_index in function_indices {
                        match executor.run(function_index, ()) {
                            Ok(()) => (),
                            Err(e) => {
                                // TODO: log error
                                println!("BuyUnit script {function_index}: {e:?}");
                            }
                        }
                    }
                }
                Ok(())
            }*/
            Self::CommanderPower(index, data) => {
                let owner_id = handler.get_game().current_owner();
                let player = handler.get_game().get_owning_player(owner_id).unwrap();
                let commander = player.commander;
                if !commander.can_activate_power(index.0, false) {
                    return Err(CommandError::PowerNotUsable);
                }
                let script = commander.power_activation_script(index.0);
                let valid = if let Some((Some(input_script), _)) = script {
                    is_commander_script_input_valid(input_script, &*handler.get_game(), &data)
                } else {
                    data.len() == 0
                };
                if !valid {
                    return Err(CommandError::PowerNotUsable);
                }
                //Self::activate_power(handler, index.0, &data);
                handler.commander_charge_sub(owner_id, commander.power_cost(index.0));
                handler.commander_power(owner_id, index.0);
                if let Some((input_script, function_index)) = script {
                    let data = input_script.map(|_| data.as_slice());
                    execute_commander_script(function_index, handler, data);
                }
                Ok(())
            }
        }?;
        let viable_player_ids = handler.with_map(|map| map.get_viable_player_ids(&*handler.get_game()));
        let players: Vec<u8> = handler.get_game().players().iter()
        .filter(|p| !p.dead)
        .map(|p| p.color_id)
        .collect();
        for owner_id in players {
            if !viable_player_ids.contains(&owner_id) {
                handler.player_dies(owner_id as i8);
            }
        }
        handler.recalculate_fog();
        Ok(())
    }

    /*pub(crate) fn activate_power(handler: &mut EventHandler<D>, index: usize, data: &[CustomActionData<D>]) {
        let owner_id = handler.get_game().current_owner();
        let commander = &handler.get_game().get_owning_player(owner_id).unwrap().commander;
        let script = commander.power_activation_script(index);
        handler.commander_charge_sub(owner_id, commander.power_cost(index));
        handler.commander_power(owner_id, index);
        if script.is_data_valid(handler.get_game(), &data) {
            script.execute(handler, data);
        }
    }*/
}

#[derive(Debug, Clone)]
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
    NotYourBubble,
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
