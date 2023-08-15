use interfaces::game_interface::{CommandInterface, EventInterface, ClientPerspective};
use zipper::*;
use zipper::zipper_derive::*;

use crate::commanders::{MAX_CHARGE, CommanderPower};
use crate::map::map::FieldData;
use crate::map::point::Point;
use crate::map::point_map::{self, MAX_AREA};
use crate::units::normal_units::{NormalUnits, NormalUnit, TransportableDrones, TransportedUnit, UnitData, UnitActionStatus};
use crate::units::structures::{LASER_CANNON_RANGE, Structure, Structures};
use crate::{player::*, details};
use crate::terrain::{Terrain, BuiltThisTurn, Realty, CaptureProgress};
use crate::details::Detail;
use crate::units::*;
use crate::map::direction::Direction;
use crate::game::game::*;
use crate::units::mercenary::MaybeMercenary;
use crate::units::chess::*;
use crate::units::commands::{UnitCommand, UnloadIndex};
use crate::units::movement::{Path, PathStep, PathStepExt};
use super::event_handler::EventHandler;

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

                let was_foggy = handler.get_game().is_foggy();

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

                handler.start_turn(was_foggy);

                Ok(())
            }
            Self::UnitCommand(command) => command.convert(handler),
            Self::BuyUnit(pos, index) => {
                let team = Some(handler.get_game().current_player().team);
                if !handler.get_game().has_vision_at(to_client_perspective(&team), pos) {
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
                    if let Some((index, owner, options)) = bubble_data {
                        if owner != owner_id {
                            return Err(CommandError::NotYourBubble);
                        }
                        if let Some((unit, cost)) = options.get(index) {
                            if *cost as i32 <= *handler.get_game().current_player().funds {
                                buy_unit(handler, *cost as u32, unit.clone(), pos);
                                handler.detail_remove(pos, index);
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
        UnitType::Structure(Structure {typ: Structures::DroneTower(Some((_, _, drone_id))), ..}) => {
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

#[derive(Debug, Clone, PartialEq, Zippable)]
#[zippable(bits = 2)]
pub enum FogChange<D: Direction> {
    NoneToSome(FieldData<D>),
    NoneToTrue(FieldData<D>),
    SomeToTrue(Option<UnitType<D>>),
}
impl<D: Direction> FogChange<D> {
    pub fn index(&self) -> U<2> {
        match self {
            Self::NoneToSome(_) => 0,
            Self::NoneToTrue(_) => 1,
            Self::SomeToTrue(_) => 2,
        }.into()
    }
}
pub fn fog_change_index(before: Option<Vision>, after: Option<Vision>) -> Option<U<2>> {
    match (before, after) {
        (None, None) => None,
        (Some(Vision::Normal), Some(Vision::Normal)) => None,
        (Some(Vision::TrueSight), Some(Vision::TrueSight)) => None,
        (None, Some(Vision::Normal)) => Some(0.into()),
        (Some(Vision::Normal), None) => Some(0.into()),
        (None, Some(Vision::TrueSight)) => Some(1.into()),
        (Some(Vision::TrueSight), None) => Some(1.into()),
        (Some(Vision::Normal), Some(Vision::TrueSight)) => Some(2.into()),
        (Some(Vision::TrueSight), Some(Vision::Normal)) => Some(2.into()),
    }
}

#[derive(Debug, Clone, PartialEq, Zippable)]
#[zippable(bits = 8)]
pub enum Event<D:Direction> {
    NextTurn,
    RandomFogNextTurn(bool),
    RandomFogForecast(bool, U<255>),
    PureFogChange(Perspective, LVec<(Point, U<2>), {point_map::MAX_AREA}>),
    FogChange(Perspective, LVec<(Point, FogChange<D>), {point_map::MAX_AREA}>),
    UnitPath(Option<Option<UnloadIndex>>, Path<D>, Option<bool>, UnitType<D>),
    HoverPath(Option<Option<UnloadIndex>>, Point, LVec<(bool, PathStep<D>), {point_map::MAX_AREA}>, Option<bool>, UnitType<D>),
    UnitActionStatus(Point, UnitActionStatus, UnitActionStatus),
    UnitExhaust(Point),
    UnitExhaustBoarded(Point, UnloadIndex),
    UnitHpChange(Point, I<-100, 99>, I<-999, 999>),
    //UnitMassHpChange(LVec<(Point, I<-100, 99>, I<-999, 999>), {point_map::MAX_AREA}>),
    UnitHpChangeBoarded(Point, UnloadIndex, I<-100, 99>),
    UnitCreation(Point, UnitType<D>),
    UnitDeath(Point, UnitType<D>),
    UnitReplacement(Point, UnitType<D>, UnitType<D>),
    //MercenaryCharge(Point, I<{-(mercenary::MAX_CHARGE as i32)}, {mercenary::MAX_CHARGE as i32}>),
    //MercenaryPowerSimple(Point),
    TerrainChange(Point, Terrain<D>, Terrain<D>),
    CaptureProgress(Point, CaptureProgress, CaptureProgress),
    MoneyChange(Owner, Funds),
    PureHideFunds(Owner),
    HideFunds(Owner, Funds), // when fog starts
    PureRevealFunds(Owner),
    RevealFunds(Owner, Funds), // when fog ends
    RemoveDetail(Point, U<{details::MAX_STACK_SIZE as i32 - 1}>, Detail),
    ReplaceDetail(Point, LVec<Detail, {details::MAX_STACK_SIZE}>, LVec<Detail, {details::MAX_STACK_SIZE}>),
    Effect(Effect<D>),
    CommanderCharge(Owner, I<{-(MAX_CHARGE as i32)}, {MAX_CHARGE as i32}>),
    CommanderFlipActiveSimple(Owner),
    UnitMovedThisGame(Point),
    EnPassantOpportunity(Point),
    UnitDirection(Point, D, D),
    UpdateBuiltThisTurn(Point, BuiltThisTurn, BuiltThisTurn),
    BuildDrone(Point, TransportableDrones),
}
impl<D: Direction> EventInterface for Event<D> {
    fn export_list(list: &Vec<Self>) -> Vec<u8> {
        let mut zipper = Zipper::new();
        for e in list {
            e.export(&mut zipper);
        }
        zipper.finish()
    }
    fn import_list(list: Vec<u8>) -> Vec<Self> {
        let mut unzipper = Unzipper::new(list);
        let mut result = vec![];
        loop {
            match Self::import(&mut unzipper) {
                Ok(e) => result.push(e),
                Err(ZipperError::NotEnoughBits) => break,
                _ => break, // TODO: should probably be handled somehow. Maybe return a Result instead?
            }
        }
        result
    }
}
impl<D: Direction> Event<D> {
    pub fn apply(&self, game: &mut Game<D>) {
        match self {
            Self::PureFogChange(team, vision_changes) => {
                flip_fog(game, to_client_perspective(&team), vision_changes.iter().cloned());
            }
            Self::RandomFogNextTurn(_) => {
                match game.get_fog_mode_mut() {
                    FogMode::Random(_, _, _, forecast) => {
                        forecast.remove(0).expect("Forecast for random fog is empty");
                    }
                    _ => panic!("Received RandomFogNextTurn event but fog isn't random"),
                }
            }
            Self::RandomFogForecast(new_value, repetitions) => {
                match game.get_fog_mode_mut() {
                    FogMode::Random(_, _, _, forecast) => {
                        for _ in 0..**repetitions {
                            forecast.push(*new_value);
                        }
                    }
                    _ => panic!("Received FogUpdateRandom event but fog isn't random"),
                }
            }
            Self::FogChange(team, changes) => {
                let team = to_client_perspective(&team);
                flip_fog(game, team, changes.iter().map(|change| (change.0, change.1.index())));
                for (pos, change) in changes.iter() {
                    apply_vision_changes(game, team, *pos, change.clone());
                }
            }
            Self::NextTurn => game.current_turn += 1,
            Self::UnitPath(unload_index, path, end_visible, unit) => {
                apply_unit_path(game, *unload_index, path, *end_visible, unit);
            }
            Self::HoverPath(unload_index, start, steps, end_visible, unit) => {
                let mut unit = unit.clone();
                if let Some((on_sea, _)) = steps.iter().last() {
                    match &mut unit {
                        UnitType::Normal(unit) => {
                            match &mut unit.typ {
                                NormalUnits::Hovercraft(os) => *os = *on_sea,
                                _ => {}
                            }
                        }
                        _ => {}
                    }
                }
                let mut path = Path::new(*start);
                for (_, step) in steps {
                    path.steps.push(step.clone());
                }
                apply_unit_path(game, *unload_index, &path, *end_visible, &unit);
            }
            Self::UnitActionStatus(pos, _, action_status) => {
                match game.get_map_mut().get_unit_mut(*pos) {
                    Some(UnitType::Normal(unit)) => {
                        unit.action_status = *action_status;
                    },
                    _ => (),
                }
            }
            Self::UnitExhaust(pos) => {
                let unit = game.get_map_mut().get_unit_mut(*pos).expect(&format!("expected a unit at {:?} to (un)exhaust!", pos));
                match unit {
                    UnitType::Normal(unit) => unit.data.exhausted = !unit.data.exhausted,
                    UnitType::Chess(unit) => unit.exhausted = !unit.exhausted,
                    UnitType::Structure(unit) => unit.exhausted = !unit.exhausted,
                }
            }
            Self::UnitExhaustBoarded(pos, index) => {
                let transporter = game.get_map_mut().get_unit_mut(*pos).expect(&format!("expected a transport at {:?} to (un)exhaust!", pos));
                let mut transported = transporter.get_boarded_mut();
                if let Some(boarded) = transported.get_mut(**index as usize) {
                    boarded.exhausted = !boarded.exhausted;
                }
            }
            Self::UnitHpChange(pos, hp_change, _) => {
                let unit = game.get_map_mut().get_unit_mut(*pos).expect(&format!("expected a unit at {:?} to change hp by {}!", pos, hp_change));
                let hp = unit.get_hp();
                unit.set_hp((hp as i32 + **hp_change) as u8);
            }
            Self::UnitHpChangeBoarded(pos, index, hp_change) => {
                let transporter = game.get_map_mut().get_unit_mut(*pos).expect(&format!("expected a transport at {:?} to change hp!", pos));
                let mut transported = transporter.get_boarded_mut();
                if let Some(boarded) = transported.get_mut(**index as usize) {
                    boarded.hp = (*boarded.hp + **hp_change).into();
                }
            }
            Self::UnitCreation(pos, unit) => {
                game.get_map_mut().set_unit(*pos, Some(unit.clone()));
            }
            Self::UnitDeath(pos, _) => {
                game.get_map_mut().set_unit(*pos, None).expect(&format!("expected a unit at {:?} to die!", pos));
            }
            Self::UnitReplacement(pos, _, unit) => {
                game.get_map_mut().set_unit(*pos, Some(unit.clone()));
            }
            Self::TerrainChange(pos, _, terrain) => {
                game.get_map_mut().set_terrain(*pos, terrain.clone());
            }
            Self::CaptureProgress(pos, _, new_progress) => {
                match game.get_map_mut().get_terrain_mut(*pos) {
                    Some(Terrain::Realty(_, _, progress)) => {
                        *progress = *new_progress;
                    }
                    _ => (), // shouldn't happen
                }
            }
            Self::MoneyChange(owner, change) => {
                if let Some(player) = game.get_owning_player_mut(*owner) {
                    player.funds += **change;
                }
            }
            Self::PureHideFunds(_) => {}
            Self::HideFunds(owner, _) => {
                if let Some(player) = game.get_owning_player_mut(*owner) {
                    player.funds = 0.into();
                }
            }
            Self::PureRevealFunds(_) => {}
            Self::RevealFunds(owner, value) => {
                if let Some(player) = game.get_owning_player_mut(*owner) {
                    player.funds = *value;
                }
            }
            Self::RemoveDetail(p, index, _) => {
                game.get_map_mut().remove_detail(*p, **index as usize);
            }
            Self::ReplaceDetail(p, _, list) => {
                game.get_map_mut().set_details(*p, list.iter().cloned().collect());
            }
            Self::Effect(_) => {}
            Self::CommanderCharge(owner, delta) => {
                game.get_owning_player_mut(*owner).unwrap().commander.add_charge(**delta);
            }
            Self::CommanderFlipActiveSimple(owner) => {
                game.get_owning_player_mut(*owner).unwrap().commander.flip_active();
            }
            Self::UnitMovedThisGame(p) => {
                if let Some(UnitType::Chess(unit)) = game.get_map_mut().get_unit_mut(*p) {
                    unit.typ.flip_moved_this_game();
                }
            }
            Self::EnPassantOpportunity(p) => {
                if let Some(UnitType::Chess(unit)) = game.get_map_mut().get_unit_mut(*p) {
                    match &mut unit.typ {
                        ChessUnits::Pawn(_, en_passant) => {
                            *en_passant = !*en_passant;
                        }
                        _ => {}
                    }
                }
            }
            Self::UnitDirection(p, _, new_dir) => {
                if let Some(UnitType::Chess(unit)) = game.get_map_mut().get_unit_mut(*p) {
                    match &mut unit.typ {
                        ChessUnits::Pawn(d, _) => {
                            *d = *new_dir;
                        }
                        _ => {}
                    }
                }
            }
            Self::UpdateBuiltThisTurn(p, _, val) => {
                match game.get_map_mut().get_terrain_mut(*p) {
                    Some(Terrain::Realty(Realty::Factory(built_this_turn), _, _)) |
                    Some(Terrain::Realty(Realty::Airport(built_this_turn), _, _)) |
                    Some(Terrain::Realty(Realty::Port(built_this_turn), _, _)) => {
                        *built_this_turn = *val;
                    }
                    _ => {}
                }
            }
            Self::BuildDrone(p, drone) => {
                match game.get_map_mut().get_unit_mut(*p) {
                    Some(UnitType::Normal(NormalUnit {typ: NormalUnits::DroneBoat(drones, _), ..})) => {
                        let unit = TransportedUnit {
                            typ: drone.clone(),
                            data: UnitData {
                                exhausted: true,
                                hp: 100.into(),
                                mercenary: MaybeMercenary::None,
                                zombie: false,
                            },
                        };
                        drones.push(unit);
                    }
                    Some(UnitType::Structure(Structure {typ: Structures::DroneTower(Some((_, drones, _))), ..})) => {
                        let unit = TransportedUnit {
                            typ: drone.clone(),
                            data: UnitData {
                                exhausted: true,
                                hp: 100.into(),
                                mercenary: MaybeMercenary::None,
                                zombie: false,
                            },
                        };
                        drones.push(unit);
                    }
                    _ => (),
                }
            }
        }
    }
    pub fn undo(&self, game: &mut Game<D>) {
        match self {
            Self::PureFogChange(team, points) => {
                flip_fog(game, to_client_perspective(&team), points.iter().cloned());
            }
            Self::RandomFogNextTurn(old_value) => {
                match game.get_fog_mode_mut() {
                    FogMode::Random(_, _, _, forecast) => {
                        forecast.insert(0, *old_value).unwrap();
                    }
                    _ => panic!("Received RandomFogNextTurn event but fog isn't random"),
                }
            }
            Self::RandomFogForecast(_, repetitions) => {
                match game.get_fog_mode_mut() {
                    FogMode::Random(_, _, _, forecast) => {
                        for _ in 0..**repetitions {
                            forecast.pop().expect("Forecast for random fog is empty");
                        }
                    }
                    _ => panic!("Received FogUpdateRandom event but fog isn't random"),
                }
            }
            Self::FogChange(team, changes) => {
                let team = to_client_perspective(&team);
                flip_fog(game, team, changes.iter().map(|change| (change.0, change.1.index())));
                for (pos, change) in changes.iter() {
                    apply_vision_changes(game, team, *pos, change.clone());
                }
            }
            Self::NextTurn => game.current_turn -= 1,
            Self::UnitPath(unload_index, path, end_visible, unit) => {
                undo_unit_path(game, *unload_index, path, *end_visible, unit);
            }
            Self::HoverPath(unload_index, start, steps, end_visible, unit) => {
                let mut path = Path::new(*start);
                for (_, step) in steps {
                    path.steps.push(step.clone());
                }
                undo_unit_path(game, *unload_index, &path, *end_visible, unit);
            }
            Self::UnitActionStatus(pos, action_status, _) => {
                match game.get_map_mut().get_unit_mut(*pos) {
                    Some(UnitType::Normal(unit)) => {
                        unit.action_status = *action_status;
                    },
                    _ => (),
                }
            }
            Self::UnitExhaust(pos) => {
                let unit = game.get_map_mut().get_unit_mut(*pos).expect(&format!("expected a unit at {:?} to (un)exhaust!", pos));
                match unit {
                    UnitType::Normal(unit) => unit.data.exhausted = !unit.data.exhausted,
                    UnitType::Chess(unit) => unit.exhausted = !unit.exhausted,
                    UnitType::Structure(unit) => unit.exhausted = !unit.exhausted,
                }
            }
            Self::UnitExhaustBoarded(pos, index) => {
                let transporter = game.get_map_mut().get_unit_mut(*pos).expect(&format!("expected a unit at {:?} to (un)exhaust!", pos));
                let mut transported = transporter.get_boarded_mut();
                if let Some(boarded) = transported.get_mut(**index as usize) {
                    boarded.exhausted = !boarded.exhausted;
                }
            }
            Self::UnitHpChange(pos, hp_change, _) => {
                let unit = game.get_map_mut().get_unit_mut(*pos).expect(&format!("expected a unit at {:?} to change hp by {}!", pos, -**hp_change));
                let hp = unit.get_hp();
                unit.set_hp((hp as i32 - **hp_change) as u8);
            }
            Self::UnitHpChangeBoarded(pos, index, hp_change) => {
                let transporter = game.get_map_mut().get_unit_mut(*pos).expect(&format!("expected a transport at {:?} to change hp!", pos));
                let mut transported = transporter.get_boarded_mut();
                if let Some(boarded) = transported.get_mut(**index as usize) {
                    boarded.hp = (*boarded.hp - **hp_change).into();
                }
            }
            Self::UnitCreation(pos, _) => {
                game.get_map_mut().set_unit(*pos, None).expect(&format!("expected a unit at {:?} to die!", pos));
            }
            Self::UnitDeath(pos, unit) => {
                game.get_map_mut().set_unit(*pos, Some(unit.clone()));
            }
            Self::UnitReplacement(pos, unit, _) => {
                game.get_map_mut().set_unit(*pos, Some(unit.clone()));
            }
            Self::TerrainChange(pos, terrain, _) => {
                game.get_map_mut().set_terrain(*pos, terrain.clone());
            }
            Self::CaptureProgress(pos, old_progress, _) => {
                match game.get_map_mut().get_terrain_mut(*pos) {
                    Some(Terrain::Realty(_, _, progress)) => {
                        *progress = *old_progress;
                    }
                    _ => (), // shouldn't happen
                }
            }
            Self::MoneyChange(owner, change) => {
                if let Some(player) = game.get_owning_player_mut(*owner) {
                    player.funds -= **change;
                }
            }
            Self::PureHideFunds(_) => {}
            Self::HideFunds(owner, value) => {
                if let Some(player) = game.get_owning_player_mut(*owner) {
                    player.funds = *value;
                }
            }
            Self::PureRevealFunds(_) => {}
            Self::RevealFunds(owner, _) => {
                if let Some(player) = game.get_owning_player_mut(*owner) {
                    player.funds = 0.into();
                }
            }
            Self::RemoveDetail(p, index, detail) => {
                game.get_map_mut().insert_detail(*p, **index as usize, detail.clone());
            }
            Self::ReplaceDetail(p, list, _) => {
                game.get_map_mut().set_details(*p, list.iter().cloned().collect());
            }
            Self::Effect(_) => {}
            Self::CommanderCharge(owner, delta) => {
                game.get_owning_player_mut(*owner).unwrap().commander.add_charge(-**delta);
            }
            Self::CommanderFlipActiveSimple(owner) => {
                game.get_owning_player_mut(*owner).unwrap().commander.flip_active();
            }
            Self::UnitMovedThisGame(p) => {
                if let Some(UnitType::Chess(unit)) = game.get_map_mut().get_unit_mut(*p) {
                    unit.typ.flip_moved_this_game();
                }
            }
            Self::EnPassantOpportunity(p) => {
                if let Some(UnitType::Chess(unit)) = game.get_map_mut().get_unit_mut(*p) {
                    match &mut unit.typ {
                        ChessUnits::Pawn(_, en_passant) => {
                            *en_passant = !*en_passant;
                        }
                        _ => {}
                    }
                }
            }
            Self::UnitDirection(p, old_dir, _) => {
                if let Some(UnitType::Chess(unit)) = game.get_map_mut().get_unit_mut(*p) {
                    match &mut unit.typ {
                        ChessUnits::Pawn(d, _) => {
                            *d = *old_dir;
                        }
                        _ => {}
                    }
                }
            }
            Self::UpdateBuiltThisTurn(p, val, _) => {
                match game.get_map_mut().get_terrain_mut(*p) {
                    Some(Terrain::Realty(Realty::Factory(built_this_turn), _, _)) |
                    Some(Terrain::Realty(Realty::Airport(built_this_turn), _, _)) |
                    Some(Terrain::Realty(Realty::Port(built_this_turn), _, _)) => {
                        *built_this_turn = *val;
                    }
                    _ => {}
                }
            }
            Self::BuildDrone(p, _) => {
                match game.get_map_mut().get_unit_mut(*p) {
                    Some(UnitType::Normal(NormalUnit {typ: NormalUnits::DroneBoat(drones, _), ..})) => {
                        drones.pop();
                    }
                    _ => (),
                }
            }
        }
    }
    pub fn fog_replacement(&self, game: &Game<D>, team: ClientPerspective) -> Option<Event<D>> {
        match self {
            Self::PureFogChange(t, points) => {
                if to_client_perspective(t) == team {
                    let mut changes = LVec::new();
                    for (p, index) in points.iter() {
                        let change = match **index {
                            0 => FogChange::NoneToSome(game.get_map().get_field_data(*p).stealth_replacement()),
                            1 => FogChange::NoneToTrue(game.get_map().get_field_data(*p)),
                            2 => FogChange::SomeToTrue(game.get_map().get_unit(*p).cloned()),
                            _ => panic!("U<2> contains a value > 2"),
                        };
                        changes.push((*p, change));
                    }
                    Some(Self::FogChange(t.clone(), changes))
                } else {
                    None
                }
            }
            Self::RandomFogNextTurn(_) |
            Self::RandomFogForecast(_, _) => {
                Some(self.clone())
            }
            Self::FogChange(_, _) => {
                panic!("FogChange should only ever be created as replacement for PureFogChange. It shouldn't be replaced itself!");
            }
            Self::NextTurn => Some(Self::NextTurn),
            Self::UnitPath(unload_index, path, into, unit) => {
                if let Some((unload_index, start, steps, into, unit)) = fog_replacement_unit_path(game, team, *unload_index, path.start, &path.steps, *into, unit.clone()) {
                    let mut path = Path::new(start);
                    path.steps = steps;
                    Some(Self::UnitPath(unload_index, path, into, unit))
                } else {
                    None
                }
            }
            Self::HoverPath(unload_index, start, steps, into, unit) => {
                if let Some((unload_index, start, steps, into, unit)) = fog_replacement_unit_path(game, team, *unload_index, *start, steps, *into, unit.clone()) {
                    Some(Self::HoverPath(unload_index, start, steps, into, unit))
                } else {
                    None
                }
            }
            Self::UnitActionStatus(pos, _, _) => {
                if game.can_see_unit_at(team, *pos, game.get_map().get_unit(*pos).unwrap()) {
                    Some(self.clone())
                } else {
                    None
                }
            }
            Self::UnitExhaust(pos) => {
                if game.can_see_unit_at(team, *pos, game.get_map().get_unit(*pos).unwrap()) {
                    Some(self.clone())
                } else {
                    None
                }
            }
            Self::UnitExhaustBoarded(pos, _) => {
                if game.can_see_unit_at(team, *pos, game.get_map().get_unit(*pos).unwrap()) {
                    Some(self.clone())
                } else {
                    None
                }
            }
            Self::UnitHpChange(pos, _, _) => {
                if game.can_see_unit_at(team, *pos, game.get_map().get_unit(*pos).unwrap()) {
                    Some(self.clone())
                } else {
                    None
                }
            }
            Self::UnitHpChangeBoarded(pos, _, _) => {
                if game.can_see_unit_at(team, *pos, game.get_map().get_unit(*pos).unwrap()) {
                    Some(self.clone())
                } else {
                    None
                }
            }
            Self::UnitCreation(pos, unit) => {
                if game.can_see_unit_at(team, *pos, unit) {
                    Some(self.clone())
                } else {
                    None
                }
            }
            Self::UnitDeath(pos, unit) => {
                if game.can_see_unit_at(team, *pos, unit) {
                    Some(self.clone())
                } else {
                    None
                }
            }
            Self::UnitReplacement(pos, before, after) => {
                match (game.can_see_unit_at(team, *pos, before), game.can_see_unit_at(team, *pos, after)) {
                    (true, true) => Some(self.clone()),
                    (false, false) => None,
                    (true, false) => Some(Self::UnitDeath(*pos, before.clone())),
                    (false, true) => Some(Self::UnitCreation(*pos, after.clone())),
                }
            }
            Self::TerrainChange(pos, before, after) => {
                if game.has_vision_at(team, *pos) {
                    Some(self.clone())
                } else {
                    let before = before.fog_replacement();
                    let after = after.fog_replacement();
                    if before != after {
                        Some(Self::TerrainChange(*pos, before, after))
                    } else {
                        None
                    }
                }
            }
            Self::CaptureProgress(pos, _, _) => {
                if game.has_vision_at(team, *pos) {
                    Some(self.clone())
                } else {
                    None
                }
            }
            Self::MoneyChange(owner, _) => {
                if !game.is_foggy() || team == to_client_perspective(&game.get_owning_player(*owner).and_then(|p| Some(p.team))) {
                    Some(self.clone())
                } else {
                    None
                }
            }
            Self::PureHideFunds(owner) => {
                if team != to_client_perspective(&game.get_owning_player(*owner).and_then(|p| Some(p.team))) {
                    Some(Self::HideFunds(owner.clone(), game.get_owning_player(*owner).unwrap().funds))
                } else {
                    None
                }
            }
            Self::HideFunds(_, _) => {
                panic!("HideFunds should only ever be created as replacement for PureHideFunds. It shouldn't be replaced itself!");
            }
            Self::PureRevealFunds(owner) => {
                if team != to_client_perspective(&game.get_owning_player(*owner).and_then(|p| Some(p.team))) {
                    Some(Self::RevealFunds(owner.clone(), game.get_owning_player(*owner).unwrap().funds))
                } else {
                    None
                }
            }
            Self::RevealFunds(_, _) => {
                panic!("RevealFunds should only ever be created as replacement for PureRevealFunds. It shouldn't be replaced itself!");
            }
            Self::RemoveDetail(p, index, detail) => {
                if game.has_vision_at(team, *p) {
                    Some(self.clone())
                } else if let Some(detail) = detail.fog_replacement() {
                    let mut new_index = 0;
                    for (i, detail) in game.get_map().get_details(*p).into_iter().enumerate() {
                        if i == **index as usize {
                            break;
                        }
                        if detail.fog_replacement().is_some() {
                            new_index += 1;
                        }
                    }
                    Some(Self::RemoveDetail(*p, new_index.into(), detail))
                } else {
                    None
                }
            }
            Self::ReplaceDetail(p, old, new) => {
                if game.has_vision_at(team, *p) {
                    Some(self.clone())
                } else {
                    let old: Vec<Detail> = old.iter().filter_map(|detail| {
                        detail.fog_replacement()
                    }).collect();
                    let new: Vec<Detail> = new.iter().filter_map(|detail| {
                        detail.fog_replacement()
                    }).collect();
                    if old != new {
                        Some(Self::ReplaceDetail(*p, old.try_into().unwrap(), new.try_into().unwrap()))
                    } else {
                        None
                    }
                }
            }
            Self::Effect(effect) => {
                if !game.is_foggy() {
                    Some(self.clone())
                } else if let Some(effect) = effect.fog_replacement(game, team) {
                    Some(Self::Effect(effect))
                } else {
                    None
                }
            }
            Self::CommanderCharge(_, _) => {
                Some(self.clone())
            }
            Self::CommanderFlipActiveSimple(_) => {
                Some(self.clone())
            }
            Self::UnitMovedThisGame(pos) => {
                if game.can_see_unit_at(team, *pos, game.get_map().get_unit(*pos).unwrap()) {
                    Some(self.clone())
                } else {
                    None
                }
            }
            Self::EnPassantOpportunity(p) => {
                if game.has_vision_at(team, *p) {
                    Some(self.clone())
                } else {
                    None
                }
            }
            Self::UnitDirection(pos, _, _) => {
                if game.can_see_unit_at(team, *pos, game.get_map().get_unit(*pos).unwrap()) {
                    Some(self.clone())
                } else {
                    None
                }
            }
            Self::UpdateBuiltThisTurn(p, _, _) => {
                if game.has_vision_at(team, *p) {
                    Some(self.clone())
                } else {
                    None
                }
            }
            Self::BuildDrone(pos, _) => {
                if game.can_see_unit_at(team, *pos, game.get_map().get_unit(*pos).unwrap()) {
                    Some(self.clone())
                } else {
                    None
                }
            }
        }
    }
}

fn apply_unit_path<D: Direction>(game: &mut Game<D>, unload_index: Option<Option<UnloadIndex>>, path: &Path<D>, end_visible: Option<bool>, unit: &UnitType<D>) {
    if let Some(unload_index) = unload_index {
        if let Some(index) = unload_index {
            if let Some(unit) = game.get_map_mut().get_unit_mut(path.start) {
                unit.unboard(*index as u8);
            }
        } else {
            game.get_map_mut().set_unit(path.start, None);
        }
    }
    if let Some(into) = end_visible {
        let end = path.end(game.get_map()).unwrap();
        if let (true, UnitType::Normal(unit)) = (into, unit) {
            let transporter = game.get_map_mut().get_unit_mut(end).unwrap();
            transporter.board(transporter.get_boarded().len() as u8, unit.clone());
        } else {
            game.get_map_mut().set_unit(end, Some(unit.clone()));
        }
    }
}

fn undo_unit_path<D: Direction>(game: &mut Game<D>, unload_index: Option<Option<UnloadIndex>>, path: &Path<D>, end_visible: Option<bool>, unit: &UnitType<D>) {
    if let Some(into) = end_visible {
        let end = path.end(game.get_map()).unwrap();
        if into {
            let transporter = game.get_map_mut().get_unit_mut(end).unwrap();
            transporter.unboard(transporter.get_boarded().len() as u8 - 1);
        } else {
            game.get_map_mut().set_unit(end, None);
        }
    }
    if let Some(unload_index) = unload_index {
        if let Some(index) = unload_index {
            if let (Some(u), UnitType::Normal(b)) = (game.get_map_mut().get_unit_mut(path.start), unit.clone()) {
                u.board(*index as u8, b);
            }
        } else {
            game.get_map_mut().set_unit(path.start, Some(unit.clone()));
        }
    }
}

fn fog_replacement_unit_path<D: Direction, S: PathStepExt<D>>(game: &Game<D>, team: ClientPerspective, unload_index: Option<Option<UnloadIndex>>, start: Point, steps: &LVec<S, {point_map::MAX_AREA}>, end_visible: Option<bool>, unit: UnitType<D>) -> Option<(Option<Option<UnloadIndex>>, Point, LVec<S, {point_map::MAX_AREA}>, Option<bool>, UnitType<D>)> {
    // TODO: doesn't work if the transporter has stealth
    let unload_index = if game.can_see_unit_at(team, start, &unit) {
        Some(unload_index.unwrap_or(None))
    } else {
        None
    };
    let mut path = Path::new(start);
    for step in steps {
        path.steps.push(step.step().clone());
    }
    // TODO: doesn't work if the transporter has stealth
    let into = if game.can_see_unit_at(team, path.end(game.get_map()).unwrap(), &unit) {
        end_visible
    } else {
        None
    };
    let visible_path = if unit.get_team(game) != team {
        unit_path_fog_replacement(game, team, unit, start, steps)
    } else {
        Some((unit, start, steps.clone()))
    };
    if let Some((unit, start, steps)) = visible_path {
        Some((unload_index, start, steps, into, unit))
    } else {
        None
    }
}

fn unit_path_fog_replacement<D: Direction, S: PathStepExt<D>>(game: &Game<D>, team: ClientPerspective, mut unit: UnitType<D>, start: Point, steps: &LVec<S, {point_map::MAX_AREA}>) -> Option<(UnitType<D>, Point, LVec<S, {point_map::MAX_AREA}>)> {
    let mut result = None;
    let mut current = start;
    let mut previous_visible = false;
    let mut last_visible = None;
    if game.can_see_unit_at(team, current, &unit) {
        result = Some((start, LVec::new()));
        previous_visible = true;
        last_visible = Some(start);
    }
    for step in steps.iter() {
        if result.is_none() {
            step.update_unit(&mut unit);
        }
        let previous = current;
        current = step.step().progress(game.get_map(), current).expect(&format!("unable to find next point after {:?}", current));
        let visible = game.can_see_unit_at(team, current, &unit);
        if visible && !previous_visible {
            // either the unit appears out of fog or this is the first step
            if let Some(result) = &mut result {
                // not necessary to skip ahead if the unit reappears in the same field where it last vanished
                if last_visible != Some(previous) {
                    result.1.push(step.skip_to(previous));
                }
            } else {
                result = Some((previous, LVec::new()));
            }
        }
        if visible || previous_visible {
            // if the previous step was visible, this one should be too
            // CAUTION: should not be visible if teleporting into fog
            last_visible = Some(current);
            result.as_mut().unwrap().1.push(step.clone());
        }
        previous_visible = visible;
    }
    result.and_then(|(start, steps)| Some((unit, start, steps)))
}

#[derive(Debug, Clone, PartialEq, Zippable)]
#[zippable(bits = 8)]
pub enum Effect<D: Direction> {
    Flame(Point),
    GunFire(Point),
    ShellFire(Point),
    Repair(Point),
    Laser(LVec<(Point, D), {LASER_CANNON_RANGE}>),
    Lightning(LVec<Point, {MAX_AREA}>),
}
impl<D: Direction> Effect<D> {
    pub fn fog_replacement(&self, game: &Game<D>, team: ClientPerspective) -> Option<Self> {
        match self {
            Self::Flame(p) |
            Self::GunFire(p) |
            Self::Repair(p) |
            Self::ShellFire(p) => {
                if game.has_vision_at(team, *p) {
                    Some(self.clone())
                } else {
                    None
                }
            }
            Self::Lightning(_) |
            Self::Laser(_) => Some(self.clone()),
        }
    }
}

fn flip_fog<D: Direction, I: Iterator<Item = (Point, U<2>)>>(game: &mut Game<D>, team: ClientPerspective, vision_changes: I) {
    for (pos, change_index) in vision_changes {
        let vision = match (*change_index, game.get_vision(team, pos)) {
            (0, None) => Some(Vision::Normal),
            (0, Some(Vision::Normal)) => None,
            (1, None) => Some(Vision::TrueSight),
            (1, Some(Vision::TrueSight)) => None,
            (2, Some(Vision::Normal)) => Some(Vision::TrueSight),
            (2, Some(Vision::TrueSight)) => Some(Vision::Normal),
            _ => panic!("pattern not covered at {:?}: {}", pos, *change_index),
        };
        game.set_vision(team, pos, vision);
    }
}

fn apply_vision_changes<D: Direction>(game: &mut Game<D>, team: ClientPerspective, pos: Point, change: FogChange<D>) {
    match change {
        FogChange::NoneToSome(mut change) |
        FogChange::NoneToTrue(mut change) => {
            if !game.has_vision_at(team, pos) {
                change = change.fog_replacement();
            }
            let FieldData {
                terrain,
                details,
                unit,
            } = change;
            game.get_map_mut().set_terrain(pos, terrain);
            game.get_map_mut().set_details(pos, details.to_vec());
            game.get_map_mut().set_unit(pos, unit);
        }
        FogChange::SomeToTrue(mut change) => {
            if !game.has_true_sight_at(team, pos) {
                change = if let Some(unit) = change {
                    if unit.fog_replacement().is_none() && game.get_map().get_terrain(pos).unwrap().hides_unit(&unit) {
                        None
                    } else {
                        unit.stealth_replacement()
                    }
                } else {
                    None
                };
            }
            game.get_map_mut().set_unit(pos, change);
        }
    }
}

