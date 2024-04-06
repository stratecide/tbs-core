use interfaces::game_interface::CommandInterface;

use crate::map::map_view::MapView;
use crate::map::point::Point;
use crate::details::Detail;
use crate::script::custom_action::CustomActionData;
use crate::terrain::attributes::TerrainAttributeKey;
use crate::terrain::terrain::TerrainBuilder;
use crate::map::direction::Direction;
use crate::units::attributes::{ActionStatus, AttributeKey};
use crate::units::commands::UnitCommand;
use crate::units::hero::Hero;
use crate::units::unit_types::UnitType;
use super::event_handler::EventHandler;
use super::fog::FogIntensity;
use super::game_view::GameView;

#[derive(Debug, Clone)]
pub enum Command<D: Direction> {
    EndTurn,
    UnitCommand(UnitCommand<D>),
    BuyUnit(Point, UnitType, D),
    CommanderPower(usize, Vec<CustomActionData<D>>),
}

impl<D: Direction> CommandInterface for Command<D> {
    fn end_turn() -> Self {
        Self::EndTurn
    }
}

impl<D: Direction> Command<D> {
    pub fn execute(self, handler: &mut EventHandler<D>) -> Result<(), CommandError> {
        match self {
            Self::EndTurn => {
                handler.end_turn();
                Ok(())
            }
            Self::UnitCommand(command) => command.execute(handler),
            Self::BuyUnit(pos, unit_type, d) => {
                let player = handler.get_game().current_player().clone();
                if handler.get_game().get_fog_at(player.get_team(), pos) != FogIntensity::TrueSight {
                    // factories and bubbles provide true-sight
                    return Err(CommandError::NoVision);
                } else if let Some(_) = handler.get_map().get_unit(pos) {
                    return Err(CommandError::Blocked(pos));
                }
                let mut bubble_index = None;
                let mut terrain = handler.get_map().get_terrain(pos).unwrap().clone();
                for (index, det) in handler.get_map().get_details(pos).into_iter().enumerate() {
                    match det {
                        Detail::Bubble(owner, terrain_type) => {
                            bubble_index = Some(index);
                            terrain = TerrainBuilder::new(handler.environment(), *terrain_type)
                            .set_owner_id(owner.0)
                            .build_with_defaults();
                            break;
                        }
                        _ => (),
                    }
                }
                if terrain.get_owner_id() != player.get_owner_id() {
                    return Err(CommandError::NotYourProperty);
                }
                let heroes = Hero::hero_influence_at(handler.get_game(), pos, player.get_owner_id());
                if !terrain.can_build(handler.get_game(), pos, &heroes) {
                    return Err(CommandError::CannotBuildHere);
                }
                // TODO: when checking the config, make sure that terrain-buildable units don't have a drone id
                if !terrain.buildable_units(handler.get_game(), pos, bubble_index.is_some(), &heroes).contains(&unit_type) {
                    return Err(CommandError::InvalidUnitType);
                }
                let built_this_turn = terrain.get_built_this_turn();
                if terrain.has_attribute(TerrainAttributeKey::BuiltThisTurn) && built_this_turn >= terrain.max_built_this_turn() {
                    return Err(CommandError::BuildLimitReached);
                }

                let (mut unit, cost) = terrain.unit_shop_option(handler.get_game(), pos, unit_type, &heroes);
                if cost > *handler.get_game().current_player().funds {
                    return Err(CommandError::NotEnoughMoney)
                }
                let owner_id = handler.get_game().current_player().get_owner_id();
                handler.money_buy(owner_id, cost);
                if bubble_index != None {
                    unit.set_status(ActionStatus::Ready);
                }
                unit.set_direction(d);
                if handler.environment().unit_attributes(unit_type, player.get_owner_id()).any(|a| *a == AttributeKey::DroneStationId) {
                    unit.set_drone_station_id(handler.get_map().new_drone_id(handler.rng()));
                }
                handler.unit_creation(pos, unit);
                if let Some(bubble_index) = bubble_index {
                    handler.detail_remove(pos, bubble_index);
                } else if terrain.has_attribute(TerrainAttributeKey::BuiltThisTurn) {
                    handler.terrain_built_this_turn(pos, built_this_turn + 1);
                }
                for effect in terrain.on_build(handler.get_game(), pos, bubble_index.is_some()) {
                    effect.trigger(handler, pos, &terrain);
                }
                Ok(())
            }
            Self::CommanderPower(index, data) => {
                let commander = &handler.get_game().current_player().commander;
                if !commander.can_activate_power(index, false) {
                    return Err(CommandError::PowerNotUsable);
                }
                let script = commander.power_activation_script(index);
                if !script.is_data_valid(handler.get_game(), &data) {
                    return Err(CommandError::PowerNotUsable);
                }
                Self::activate_power(handler, index, &data);
                Ok(())
            }
        }?;
        let viable_player_ids = handler.get_map().get_viable_player_ids();
        let players: Vec<u8> = handler.get_game().players.iter()
        .filter(|p| !p.dead)
        .map(|p| p.get_owner_id() as u8)
        .collect();
        for owner_id in players {
            if !viable_player_ids.contains(&owner_id) {
                handler.player_dies(owner_id as i8);
            }
        }
        handler.recalculate_fog();
        Ok(())
    }

    pub(crate) fn activate_power(handler: &mut EventHandler<D>, index: usize, data: &[CustomActionData<D>]) {
        let owner_id = handler.get_game().current_player().get_owner_id();
        let commander = &handler.get_game().current_player().commander;
        let script = commander.power_activation_script(index);
        handler.commander_charge_sub(owner_id, commander.power_cost(index));
        handler.commander_power(owner_id, index);
        if script.is_data_valid(handler.get_game(), &data) {
            script.execute(handler, data);
        }
    }
}

#[derive(Debug, Clone)]
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

