use interfaces::game_interface::{CommandInterface, GameInterface};

use crate::map::point::Point;
use crate::details::Detail;
use crate::terrain::terrain::TerrainBuilder;
use crate::map::direction::Direction;
use crate::units::attributes::{ActionStatus, AttributeKey};
use crate::units::commands::UnitCommand;
use crate::units::hero::Hero;
use crate::units::unit_types::UnitType;
use super::event_handler::EventHandler;
use super::fog::FogIntensity;

#[derive(Debug)]
pub enum Command<D: Direction> {
    EndTurn,
    UnitCommand(UnitCommand<D>),
    BuyUnit(Point, UnitType, D),
    CommanderPowerSimple(usize),
}

impl<D: Direction> CommandInterface for Command<D> {
    fn end_turn() -> Self {
        Self::EndTurn
    }
}

impl<D: Direction> Command<D> {
    pub fn execute(self, handler: &mut EventHandler<D>) -> Result<(), CommandError> {
        let owner_id = handler.get_game().current_player().get_owner_id();
        match self {
            Self::EndTurn => {
                // un-exhaust units
                for p in handler.get_map().all_points() {
                    if let Some(unit) = handler.get_map().get_unit(p).cloned() {
                        if unit.get_owner_id() == owner_id {
                            match unit.get_status() {
                                ActionStatus::Exhausted => handler.unit_status(p, ActionStatus::Ready),
                                _ => (),
                            }
                            for (index, u) in unit.get_transported().iter().enumerate() {
                                if u.is_exhausted() {
                                    handler.unit_status_boarded(p, index, ActionStatus::Ready);
                                }
                                //if unit.heal_transported() > 0 {
                                //    handler.unit_heal_boarded(p, index, unit.heal_transported() as u8);
                                //} else if unit.heal_transported() < 0 {
                                //    handler.unit_damage_boarded(position, index, -unit.heal_transported() as u8);
                                //    kill units with 0 HP
                                //}
                            }
                        }
                    }
                }

                // unit end turn event
                handler.trigger_all_unit_scripts(
                    |game, unit, unit_pos, transporter, heroes| {
                        unit.on_end_turn(game, unit_pos, transporter, heroes)
                    },
                    |_observation_id| {},
                    |this, script, unit_pos, unit, _observation_id| {
                        script.trigger(this, unit_pos, unit);
                    }
                );

                // reset built_this_turn-counter for realties
                for p in handler.get_map().all_points() {
                    handler.terrain_built_this_turn(p, 0);
                }

                let fog_before = if handler.get_game().is_foggy() {
                    let next_player = handler.get_game().players.get((handler.get_game().current_turn() + 1) % handler.get_game().players.len()).unwrap();
                    Some(handler.get_game().recalculate_fog(next_player.get_team()))
                } else {
                    None
                };

                handler.next_turn();

                // reset capture-progress / finish capturing
                let owner_id = handler.get_game().current_player().get_owner_id();
                for p in handler.get_map().all_points() {
                    let terrain = handler.get_map().get_terrain(p).unwrap();
                    if let Some((new_owner, progress)) = terrain.get_capture_progress() {
                        if new_owner.0 == owner_id {
                            if let Some(unit) = handler.get_map().get_unit(p).filter(|u| u.get_owner_id() == owner_id && u.can_capture()) {
                                if unit.get_status() == ActionStatus::Capturing {
                                    let max_progress = terrain.get_capture_resistance();
                                    let progress = progress as u16 + (unit.get_hp() as f32 / 10.).ceil() as u16;
                                    if progress < max_progress as u16 {
                                        handler.terrain_capture_progress(p, Some((new_owner, (progress as u8).into())));
                                    } else {
                                        // captured
                                        let terrain = TerrainBuilder::new(handler.environment(), terrain.typ())
                                        .copy_from(terrain)
                                        .set_capture_progress(None)
                                        .set_owner_id(new_owner.0)
                                        .build_with_defaults();
                                        handler.terrain_replace(p, terrain);
                                    }
                                    handler.unit_status(p, ActionStatus::Ready);
                                }
                            } else {
                                handler.terrain_capture_progress(p, None);
                            }
                        }
                    }
                }

                let next_power = handler.get_game().current_player().commander.get_next_power();
                if handler.get_game().current_player().commander.can_activate_power(next_power) {
                    Self::activate_power(handler, next_power);
                }

                handler.start_turn(fog_before);

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
                            terrain = TerrainBuilder::new(handler.environment(), terrain_type)
                            .set_owner_id(owner.0)
                            .build_with_defaults();
                            break;
                        }
                        _ => (),
                    }
                }
                if !terrain.can_build() {
                    return Err(CommandError::CannotBuildHere);
                }
                if terrain.get_owner_id() != player.get_owner_id() {
                    return Err(CommandError::NorYourProperty);
                }
                // TODO: when checking the config, make sure that terrain-buildable units don't have a drone id
                if !terrain.buildable_units().contains(&unit_type) {
                    return Err(CommandError::InvalidUnitType);
                }

                let heroes = Hero::hero_influence_at(Some(handler.get_game()), handler.get_map(), pos, player.get_owner_id());
                let heroes: Vec<_> = heroes.iter().collect();
                let (mut unit, cost) = terrain.unit_shop_option(handler.get_game(), pos, unit_type, &heroes);
                if cost > *handler.get_game().current_player().funds {
                    return Err(CommandError::NotEnoughMoney)
                }
                handler.money_buy(owner_id, cost.max(0) as u32);
                if bubble_index == None {
                    unit.set_status(ActionStatus::Exhausted);
                }
                unit.set_direction(d);
                if handler.environment().unit_attributes(unit_type, player.get_owner_id()).any(|a| *a == AttributeKey::DroneStationId) {
                    unit.set_drone_station_id(handler.get_map().new_drone_id(handler.rng()));
                }
                handler.unit_creation(pos, unit);
                if let Some(bubble_index) = bubble_index {
                    handler.detail_remove(pos, bubble_index);
                }
                Ok(())
            }
            Self::CommanderPowerSimple(index) => {
                if !handler.get_game().current_player().commander.can_activate_power(index) {
                    return Err(CommandError::PowerNotUsable);
                }
                Self::activate_power(handler, index);
                Ok(())
            }
        }
    }

    fn activate_power(handler: &mut EventHandler<D>, index: usize) {
        let owner_id = handler.get_game().current_player().get_owner_id();
        handler.commander_charge_sub(owner_id, handler.get_game().current_player().commander.power_cost(index));
        handler.commander_power(owner_id, index);
        for active_effect in handler.get_game().current_player().commander.power_activation_effects(index) {
            active_effect.trigger(handler, owner_id);
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
    NorYourProperty,
    CannotCaptureHere,
    NotYourBubble,
    InvalidCommanderPower,
    NotEnoughCharge,
    CannotRepairHere,
    CannotBuildHere,
}

