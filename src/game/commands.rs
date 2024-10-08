use std::error::Error;
use std::fmt::Display;

use interfaces::GameInterface;
use rhai::packages::Package;
use rhai::Scope;
use zipper_derive::Zippable;
use zipper::*;

use crate::config::environment::Environment;
use crate::map::point::Point;
use crate::details::Detail;
use crate::script::custom_action::{execute_commander_script, is_commander_script_input_valid, CustomActionData};
use crate::script::executor::Executor;
use crate::script::{CONST_NAME_EVENT_HANDLER, CONST_NAME_POSITION, CONST_NAME_TERRAIN};
use crate::terrain::attributes::TerrainAttributeKey;
use crate::terrain::terrain::TerrainBuilder;
use crate::map::direction::Direction;
use crate::units::attributes::{ActionStatus, AttributeKey};
use crate::units::commands::{UnitCommand, MAX_CUSTOM_ACTION_STEPS};
use crate::units::hero::Hero;
use crate::units::unit_types::UnitType;
use super::event_handler::EventHandler;
use super::fog::FogIntensity;
use super::game_view::GameView;
use super::rhai_event_handler::EventHandlerPackage;

#[derive(Debug, Clone, Zippable)]
#[zippable(bits = 4, support_ref = Environment)]
pub enum Command<D: Direction> {
    EndTurn,
    UnitCommand(UnitCommand<D>),
    BuyUnit(Point, UnitType, D),
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
            Self::BuyUnit(pos, unit_type, d) => {
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
                for (index, det) in handler.get_game().get_details(pos).into_iter().enumerate() {
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
                    handler.detail_remove(pos, bubble_index);
                } else if terrain.has_attribute(TerrainAttributeKey::BuiltThisTurn) {
                    handler.terrain_built_this_turn(pos, built_this_turn + 1);
                }
                let function_indices = terrain.on_build(&*handler.get_game(), pos, bubble_index.is_some());
                if function_indices.len() > 0 {
                    let mut scope = Scope::new();
                    scope.push_constant(CONST_NAME_POSITION, pos);
                    scope.push_constant(CONST_NAME_TERRAIN, terrain);
                    handler.get_game().add_self_to_scope(&mut scope);
                    scope.push_constant(CONST_NAME_EVENT_HANDLER, handler.clone());
                    let environment = handler.get_game().environment();
                    let mut engine = environment.get_engine();
                    EventHandlerPackage::new().register_into_engine(&mut engine);
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
            }
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
