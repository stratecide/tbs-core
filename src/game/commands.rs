use interfaces::game_interface::{CommandInterface, GameInterface};
use zipper::*;
use zipper::zipper_derive::*;

use crate::commanders::CommanderPower;
use crate::map::point::Point;
use crate::units::normal_units::{NormalUnits, NormalUnit, UnitActionStatus};
use crate::units::structures::{Structure, Structures};
use crate::player::*;
use crate::terrain::{Terrain, Realty, CaptureProgress};
use crate::details::Detail;
use crate::units::*;
use crate::map::direction::Direction;
use crate::units::commands::UnitCommand;
use super::event_handler::EventHandler;
use super::fog::FogIntensity;

#[derive(Debug, Zippable)]
#[zippable(bits = 8)]
pub enum Command<D: Direction> {
    EndTurn,
    UnitCommand(UnitCommand<D>),
    BuyUnit(Point, U<255>),
    CommanderPowerSimple(CommanderPower),
}

impl<D: Direction> CommandInterface for Command<D> {
    fn end_turn() -> Self {
        Self::EndTurn
    }
}

impl<D: Direction> Command<D> {
    pub fn convert(self, handler: &mut EventHandler<D>) -> Result<(), CommandError> {
        let owner_id = handler.get_game().current_player().owner_id;
        match self {
            Self::EndTurn => {
                // un-exhaust units
                for p in handler.get_map().all_points() {
                    if let Some(unit) = handler.get_map().get_unit(p).cloned() {
                        if unit.get_owner() == Some(owner_id) {
                            match unit {
                                // Structures get un-exhausted during start_turn
                                UnitType::Structure(_) => (),
                                _ => {
                                    if unit.is_exhausted() {
                                        handler.unit_unexhaust(p);
                                    }
                                },
                            }
                            for (index, u) in unit.get_boarded().iter().enumerate() {
                                if u.data.exhausted {
                                    handler.unit_unexhaust_boarded(p, index.into());
                                } else {
                                    match &u.typ {
                                        NormalUnits::LightDrone(_) |
                                        NormalUnits::HeavyDrone(_) => {
                                            if u.get_hp() < 100 {
                                                handler.unit_heal_boarded(p, index.into(), 30);
                                            }
                                        }
                                        _ => (),
                                    }
                                }
                            }
                        }
                    }
                }

                // reset built_this_turn-counter for realties
                for p in handler.get_map().all_points() {
                    match handler.get_map().get_terrain(p) {
                        Some(Terrain::Realty(realty, _, _)) => {
                            match realty {
                                Realty::Factory(_) |
                                Realty::Airport(_) |
                                Realty::Port(_) => {
                                    handler.terrain_built_this_turn(p, 0.into());
                                }
                                _ => (),
                            }
                        }
                        _ => (),
                    }
                }

                let fog_before = if handler.get_game().is_foggy() {
                    let next_player = handler.get_game().players.get((handler.get_game().current_turn() + 1) % handler.get_game().players.len()).unwrap();
                    Some(handler.get_game().recalculate_fog(Some(next_player.team)))
                } else {
                    None
                };

                handler.next_turn();

                // reset capture-progress / finish capturing
                let current_player_owner = handler.get_game().current_player().owner_id;
                for p in handler.get_map().all_points() {
                    match handler.get_map().get_terrain(p) {
                        Some(Terrain::Realty(realty, owner, CaptureProgress::Capturing(new_owner, progress))) => {
                            if let Some(unit) = handler.get_map().get_unit(p).filter(|u| u.get_owner() != *owner && u.get_owner() == Some(*new_owner) && u.can_capture()) {
                                if current_player_owner == *new_owner && unit.is_capturing() {
                                    let progress = **progress + (unit.get_hp() as f32 / 10.).ceil() as i32;
                                    if progress < 10 {
                                        handler.terrain_capture_progress(p, CaptureProgress::Capturing(*new_owner, progress.into()));
                                    } else {
                                        // captured
                                        handler.terrain_replace(p, Terrain::Realty(realty.clone(), Some(*new_owner), CaptureProgress::None));
                                    }
                                }
                                // keep progress otherwise
                            } else {
                                handler.terrain_capture_progress(p, CaptureProgress::None);
                            }
                        }
                        _ => (),
                    }
                    if let Some(UnitType::Normal(unit)) = handler.get_map().get_unit(p) {
                        if unit.get_owner() == current_player_owner && unit.action_status != UnitActionStatus::Normal {
                            handler.unit_status(p, UnitActionStatus::Normal);
                        }
                    }
                }

                // end merc powers
                for p in handler.get_map().all_points() {
                    match handler.get_map().get_unit(p) {
                        Some(UnitType::Normal(unit)) => {
                            if unit.owner == owner_id && unit.data.mercenary.power_active() {
                                handler.mercenary_power_start(p);
                            }
                        }
                        _ => {}
                    }
                }
                
                handler.get_game().current_player().commander.clone().start_turn(handler, handler.get_game().current_player().owner_id);

                handler.start_turn(fog_before);

                Ok(())
            }
            Self::UnitCommand(command) => command.convert(handler),
            Self::BuyUnit(pos, index) => {
                let team = Some(handler.get_game().current_player().team);
                if handler.get_game().get_fog_at(to_client_perspective(&team), pos) != FogIntensity::TrueSight {
                    // factories and bubbles provide true-sight
                    Err(CommandError::NoVision)
                } else if let Some(_) = handler.get_map().get_unit(pos) {
                    Err(CommandError::Blocked(pos))
                } else {
                    let mut bubble_data = None;
                    let details = handler.get_map().get_details(pos);
                    for (index, detail) in details.into_iter().enumerate() {
                        match detail {
                            Detail::AirportBubble(owner) => {
                                bubble_data = Some((
                                    index,
                                    owner,
                                    crate::terrain::build_options_airport(handler.get_game(), owner_id, 0),
                                ));
                            }
                            Detail::FactoryBubble(owner) => {
                                bubble_data = Some((
                                    index,
                                    owner,
                                    crate::terrain::build_options_factory(handler.get_game(), owner_id, 0),
                                ));
                            }
                            Detail::PortBubble(owner) => {
                                bubble_data = Some((
                                    index,
                                    owner,
                                    crate::terrain::build_options_port(handler.get_game(), owner_id, 0),
                                ));
                            }
                            _ => {}
                        }
                    }
                    if let Some((bubble_index, owner, options)) = bubble_data {
                        if owner != owner_id {
                            return Err(CommandError::NotYourBubble);
                        }
                        if let Some((unit, cost)) = options.get(*index as usize) {
                            if *cost as i32 <= *handler.get_game().current_player().funds {
                                buy_unit(handler, *cost as u32, unit.clone(), pos);
                                handler.detail_remove(pos, bubble_index);
                                Ok(())
                            } else {
                                Err(CommandError::NotEnoughMoney)
                            }
                        } else {
                            Err(CommandError::InvalidIndex)
                        }
                    } else if let Some(Terrain::Realty(realty, owner, _)) = handler.get_map().get_terrain(pos) {
                        if owner == &Some(owner_id) {
                            let options = realty.buildable_units(handler.get_game(), owner_id);
                            if let Some((unit, cost)) = options.get(*index as usize) {
                                if *cost as i32 <= *handler.get_game().current_player().funds {
                                    let realty = realty.clone();
                                    let mut unit = unit.clone();
                                    unit.set_exhausted(true);
                                    buy_unit(handler, *cost as u32, unit, pos);
                                    // increment counter for that realty
                                    realty.after_buying(pos, handler);
                                    Ok(())
                                } else {
                                    Err(CommandError::NotEnoughMoney)
                                }
                            } else {
                                Err(CommandError::InvalidIndex)
                            }
                        } else {
                            Err(CommandError::NotYourRealty)
                        }
                    } else {
                        Err(CommandError::NotYourRealty)
                    }
                }
            }
            Self::CommanderPowerSimple(power) => {
                if !power.is_simple() || !handler.get_game().current_player().commander.powers().into_iter().any(|p| p == power) {
                    return Err(CommandError::InvalidCommanderPower);
                }
                if *handler.get_game().current_player().commander.charge() < power.charge_cost() as i32 {
                    return Err(CommandError::NotEnoughCharge);
                }
                /*if !handler.get_game().current_player().commander.can_activate(&power) {
                    return Err(CommandError::PowerNotUsable);
                }*/
                if handler.get_game().current_player().commander.power_active() {
                    return Err(CommandError::PowerNotUsable);
                }
                Ok(power.execute(handler, handler.get_game().current_player().owner_id))
            }
        }
    }
}

fn buy_unit<D: Direction>(handler: &mut EventHandler<D>, cost: u32, mut unit: UnitType<D>, pos: Point) {
    let owner_id = handler.get_game().current_player().owner_id;
    handler.money_buy(owner_id, cost);
    match &mut unit {
        UnitType::Normal(NormalUnit {typ: NormalUnits::DroneBoat(_, drone_id), ..}) => {
            *drone_id = handler.get_map().new_drone_id(handler.rng());
        }
        UnitType::Normal(NormalUnit {typ: NormalUnits::DroneShip(_, drone_id), ..}) => {
            *drone_id = handler.get_map().new_drone_id(handler.rng());
        }
        UnitType::Structure(Structure {typ: Structures::DroneTower(_, _, drone_id), ..}) => {
            *drone_id = handler.get_map().new_drone_id(handler.rng());
        }
        _ => (),
    }
    handler.unit_creation(pos, unit); 
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
    InvalidIndex,
    PowerNotUsable,
    Blocked(Point),
    NotEnoughMoney,
    NotYourRealty,
    CannotCaptureHere,
    NotYourBubble,
    InvalidCommanderPower,
    NotEnoughCharge,
    CannotRepairHere,
}

