use interfaces::game_interface::{EventInterface, ClientPerspective};
use zipper::*;
use zipper::zipper_derive::*;

use crate::commanders::MAX_CHARGE;
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
use crate::game::fog::*;
use crate::units::mercenary::MaybeMercenary;
use crate::units::chess::*;
use crate::units::commands::UnloadIndex;
use crate::units::movement::PathStep;

#[derive(Debug, Clone, PartialEq, Zippable)]
#[zippable(bits = 8)]
pub enum Event<D:Direction> {
    NextTurn,
    MoneyChange(Owner, Funds),
    Effect(Effect<D>),
    UnitPath(Option<UnitType<D>>, LVec<UnitStep<D>, {point_map::MAX_AREA}>),
    // fog events
    PureFogChange(Perspective, LVec<(Point, FogIntensity, FogIntensity), {point_map::MAX_AREA}>),
    FogChange(Perspective, LVec<(Point, FogIntensity, FogIntensity, FieldData<D>), {point_map::MAX_AREA}>),
    PureHideFunds(Owner),
    HideFunds(Owner, Funds),
    PureRevealFunds(Owner),
    RevealFunds(Owner, Funds),
    // unit events
    UnitAdd(Point, UnitType<D>),
    UnitRemove(Point, UnitType<D>),
    UnitAddBoarded(Point, NormalUnit),
    UnitRemoveBoarded(Point, UnloadIndex, NormalUnit),
    UnitExhaust(Point),
    UnitExhaustBoarded(Point, UnloadIndex),
    UnitHpChange(Point, I<-100, 99>, I<-999, 999>),
    UnitHpChangeBoarded(Point, UnloadIndex, I<-100, 99>),
    //UnitPath(Option<Option<UnloadIndex>>, Path<D>, Option<bool>, UnitType<D>),
    //HoverPath(Option<Option<UnloadIndex>>, Point, LVec<(bool, PathStep<D>), {point_map::MAX_AREA}>, Option<bool>, UnitType<D>),
    UnitActionStatus(Point, UnitActionStatus, UnitActionStatus),
    UnitMovedThisGame(Point),
    EnPassantOpportunity(Point),
    UnitDirection(Point, D, D),
    BuildDrone(Point, TransportableDrones),
    // terrain events
    TerrainChange(Point, Terrain<D>, Terrain<D>),
    CaptureProgress(Point, CaptureProgress, CaptureProgress),
    UpdateBuiltThisTurn(Point, BuiltThisTurn, BuiltThisTurn),
    // detail events
    RemoveDetail(Point, U<{details::MAX_STACK_SIZE as i32 - 1}>, Detail),
    ReplaceDetail(Point, LVec<Detail, {details::MAX_STACK_SIZE}>, LVec<Detail, {details::MAX_STACK_SIZE}>),
    // commander events
    CommanderCharge(Owner, I<{-(MAX_CHARGE as i32)}, {MAX_CHARGE as i32}>),
    CommanderFlipActiveSimple(Owner),
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
                let team = to_client_perspective(&team);
                for (pos, _, intensity) in vision_changes {
                    game.set_fog(team, *pos, *intensity);
                }
            }
            Self::FogChange(team, changes) => {
                let team = to_client_perspective(&team);
                for (pos, intensity_before, intensity, fd) in changes.iter() {
                    apply_vision_changes(game, team, *pos, *intensity_before, *intensity, fd);
                }
            }
            Self::NextTurn => game.current_turn += 1,
            /*Self::UnitPath(unload_index, path, end_visible, unit) => {
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
            }*/
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
                    UnitType::Unknown => panic!("Unknown unit to (un)exhaust at {pos:?}"),
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
            Self::UnitAdd(pos, unit) => {
                game.get_map_mut().set_unit(*pos, Some(unit.clone()));
            }
            Self::UnitRemove(pos, _) => {
                game.get_map_mut().set_unit(*pos, None).expect(&format!("expected a unit at {:?} to die!", pos));
            }
            Self::UnitAddBoarded(pos, unit) => {
                if let Some(transporter) = game.get_map_mut().get_unit_mut(*pos) {
                    transporter.board(unit.get_boarded().len() as u8, unit.clone());
                }
            }
            Self::UnitRemoveBoarded(pos, index, _) => {
                if let Some(transporter) = game.get_map_mut().get_unit_mut(*pos) {
                    transporter.unboard(**index as u8);
                }
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
            Self::UnitPath(_, _) => {}
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
            Self::PureFogChange(team, vision_changes) => {
                let team = to_client_perspective(&team);
                for (pos, intensity, _) in vision_changes {
                    game.set_fog(team, *pos, *intensity);
                }
            }
            Self::FogChange(team, changes) => {
                let team = to_client_perspective(&team);
                for (pos, intensity_before, intensity, fd) in changes.iter() {
                    apply_vision_changes(game, team, *pos, *intensity, *intensity_before, fd);
                }
            }
            Self::NextTurn => game.current_turn -= 1,
            /*Self::UnitPath(unload_index, path, end_visible, unit) => {
                undo_unit_path(game, *unload_index, path, *end_visible, unit);
            }
            Self::HoverPath(unload_index, start, steps, end_visible, unit) => {
                let mut path = Path::new(*start);
                for (_, step) in steps {
                    path.steps.push(step.clone());
                }
                undo_unit_path(game, *unload_index, &path, *end_visible, unit);
            }*/
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
                    UnitType::Unknown => panic!("Unknown unit to (un)exhaust at {pos:?}"),
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
            Self::UnitAdd(pos, _) => {
                game.get_map_mut().set_unit(*pos, None).expect(&format!("expected a unit at {:?} to die!", pos));
            }
            Self::UnitRemove(pos, unit) => {
                game.get_map_mut().set_unit(*pos, Some(unit.clone()));
            }
            Self::UnitAddBoarded(pos, _) => {
                if let Some(transporter) = game.get_map_mut().get_unit_mut(*pos) {
                    transporter.unboard(transporter.get_boarded().len() as u8 - 1);
                }
            }
            Self::UnitRemoveBoarded(pos, index, unit) => {
                if let Some(transporter) = game.get_map_mut().get_unit_mut(*pos) {
                    transporter.board(**index as u8, unit.clone());
                }
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
            Self::UnitPath(_, _) => {}
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
                    for (p, intensity_before, intensity) in points.iter() {
                        let change = game.get_map().get_field_data(*p).fog_replacement(*intensity_before.min(intensity));
                        changes.push((*p, *intensity_before, *intensity, change));
                    }
                    Some(Self::FogChange(t.clone(), changes))
                } else {
                    None
                }
            }
            Self::FogChange(_, _) => {
                panic!("FogChange should only ever be created as replacement for PureFogChange. It shouldn't be replaced itself!");
            }
            Self::NextTurn => Some(Self::NextTurn),
            /*Self::UnitPath(unload_index, path, into, unit) => {
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
            }*/
            Self::UnitActionStatus(pos, _, _) => {
                if game.can_see_unit_at(team, *pos, game.get_map().get_unit(*pos).unwrap(), false) {
                    Some(self.clone())
                } else {
                    None
                }
            }
            Self::UnitExhaust(pos) => {
                if game.can_see_unit_at(team, *pos, game.get_map().get_unit(*pos).unwrap(), false) {
                    Some(self.clone())
                } else {
                    None
                }
            }
            Self::UnitHpChange(pos, _, _) => {
                if game.can_see_unit_at(team, *pos, game.get_map().get_unit(*pos).unwrap(), false) {
                    Some(self.clone())
                } else {
                    None
                }
            }
            Self::UnitAdd(pos, unit) => {
                if let Some(unit) = unit.fog_replacement(game.get_map().get_terrain(*pos).unwrap(), game.get_fog_at(team, *pos)) {
                    Some(Self::UnitAdd(*pos, unit))
                } else {
                    None
                }
            }
            Self::UnitRemove(pos, unit) => {
                if let Some(unit) = unit.fog_replacement(game.get_map().get_terrain(*pos).unwrap(), game.get_fog_at(team, *pos)) {
                    Some(Self::UnitRemove(*pos, unit))
                } else {
                    None
                }
            }
            Self::UnitHpChangeBoarded(pos, _, _) |
            Self::UnitExhaustBoarded(pos, _) |
            Self::UnitAddBoarded(pos, _) |
            Self::UnitRemoveBoarded(pos, _, _) => {
                if game.get_fog_at(team, *pos) <= FogIntensity::NormalVision && game.can_see_unit_at(team, *pos, game.get_map().get_unit(*pos).unwrap(), false) {
                    Some(self.clone())
                } else {
                    None
                }
            }
            Self::TerrainChange(pos, before, after) => {
                let fog_intensity = game.get_fog_at(team, *pos);
                let before = before.fog_replacement(fog_intensity);
                let after = after.fog_replacement(fog_intensity);
                if before != after {
                    Some(Self::TerrainChange(*pos, before, after))
                } else {
                    None
                }
            }
            Self::CaptureProgress(pos, _, _) => {
                match game.get_fog_at(team, *pos) {
                    FogIntensity::TrueSight |
                    FogIntensity::NormalVision => {
                        Some(self.clone())
                    }
                    _ => {
                        None
                    }
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
                let fog_intensity = game.get_fog_at(team, *p);
                if fog_intensity == FogIntensity::TrueSight {
                    Some(self.clone())
                } else if let Some(detail) = detail.fog_replacement(fog_intensity) {
                    let mut new_index = 0;
                    for (i, detail) in game.get_map().get_details(*p).into_iter().enumerate() {
                        if i == **index as usize {
                            break;
                        }
                        if detail.fog_replacement(fog_intensity).is_some() {
                            new_index += 1;
                        }
                    }
                    Some(Self::RemoveDetail(*p, new_index.into(), detail))
                } else {
                    None
                }
            }
            Self::ReplaceDetail(p, old, new) => {
                let fog_intensity = game.get_fog_at(team, *p);
                if fog_intensity == FogIntensity::TrueSight {
                    Some(self.clone())
                } else {
                    let old: Vec<Detail> = old.iter().filter_map(|detail| {
                        detail.fog_replacement(fog_intensity)
                    }).collect();
                    let new: Vec<Detail> = new.iter().filter_map(|detail| {
                        detail.fog_replacement(fog_intensity)
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
            Self::UnitPath(unit, steps) => {
                let mut unit = unit.clone().expect("UnitPath needs to have a unit before fog_replacement");
                if unit.get_team(game) == team {
                    return Some(self.clone());
                }
                let mut result = Vec::new();
                for step in steps {
                    if let Some(step) = step.replace_unit_path(game, team, &mut unit) {
                        result.push(step);
                    }
                }
                if result.len() == 0 {
                    None
                } else {
                    let start = result[0].get_start();
                    let unit = unit.fog_replacement(game.get_map().get_terrain(start).unwrap(), game.get_fog_at(team, start));
                    Some(Self::UnitPath(unit, result.try_into().unwrap()))
                }
            }
            Self::CommanderCharge(_, _) => {
                Some(self.clone())
            }
            Self::CommanderFlipActiveSimple(_) => {
                Some(self.clone())
            }
            Self::UnitMovedThisGame(pos) => {
                if game.can_see_unit_at(team, *pos, game.get_map().get_unit(*pos).unwrap(), false) {
                    Some(self.clone())
                } else {
                    None
                }
            }
            Self::EnPassantOpportunity(pos) => {
                if game.can_see_unit_at(team, *pos, game.get_map().get_unit(*pos).unwrap(), false) {
                    Some(self.clone())
                } else {
                    None
                }
            }
            Self::UnitDirection(pos, _, _) => {
                if game.can_see_unit_at(team, *pos, game.get_map().get_unit(*pos).unwrap(), false) {
                    Some(self.clone())
                } else {
                    None
                }
            }
            Self::UpdateBuiltThisTurn(p, _, _) => {
                match game.get_fog_at(team, *p) {
                    FogIntensity::TrueSight |
                    FogIntensity::NormalVision => Some(self.clone()),
                    _ => None,
                }
            }
            Self::BuildDrone(pos, _) => {
                if game.can_see_unit_at(team, *pos, game.get_map().get_unit(*pos).unwrap(), false) {
                    Some(self.clone())
                } else {
                    None
                }
            }
        }
    }
}

/*fn apply_unit_path<D: Direction>(game: &mut Game<D>, unload_index: Option<Option<UnloadIndex>>, path: &Path<D>, end_visible: Option<bool>, unit: &UnitType<D>) {
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
    let unload_index = if game.can_see_unit_at(team, start, &unit, true) {
        Some(unload_index.unwrap_or(None))
    } else {
        None
    };
    let mut path = Path::new(start);
    for step in steps {
        path.steps.push(step.step().clone());
    }
    // TODO: doesn't work if the transporter has stealth
    let into = if game.can_see_unit_at(team, path.end(game.get_map()).unwrap(), &unit, true) {
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
    if game.can_see_unit_at(team, current, &unit, true) {
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
        let visible = game.can_see_unit_at(team, current, &unit, true);
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
}*/

#[derive(Debug, Clone, PartialEq, Zippable)]
#[zippable(bits = 1)]
pub enum UnitStep<D: Direction> {
    Simple(Point, PathStep<D>),
    Transform(Point, PathStep<D>, Option<UnitType<D>>),
}

impl<D: Direction> UnitStep<D> {
    pub fn get_start(&self) -> Point {
        match self {
            Self::Simple(p, _) => *p,
            Self::Transform(p, _, _) => *p,
        }
    }

    pub fn get_step(&self) -> PathStep<D> {
        match self {
            Self::Simple(_, step) => *step,
            Self::Transform(_, step, _) => *step,
        }
    }

    fn replace_unit_path(&self, game: &Game<D>, team: ClientPerspective, unit: &mut UnitType<D>) -> Option<Self> {
        let (p, step, unit2) = match self {
            Self::Simple(p, step) => (*p, *step, Some(unit.clone())),
            Self::Transform(p, step, unit2) => (*p, *step, unit2.clone()),
        };
        let p2 = step.progress(game.get_map(), p).unwrap();
        let unit1 = unit.fog_replacement(game.get_map().get_terrain(p).unwrap(), game.get_fog_at(team, p));
        let unit2 = unit2.and_then(|unit| unit.fog_replacement(game.get_map().get_terrain(p2).unwrap(), game.get_fog_at(team, p2)));
        if let Some(unit2) = unit2.clone() {
            *unit = unit2;
        }
        if unit1 == None && unit2 == None {
            None
        } else if unit1 == unit2 {
            Some(Self::Simple(p, step))
        } else {
            Some(Self::Transform(p, step, unit2))
        }
    }

}

#[derive(Debug, Clone, PartialEq, Zippable)]
#[zippable(bits = 8)]
pub enum Effect<D: Direction> {
    Laser(LVec<(Point, D), {LASER_CANNON_RANGE}>),
    Lightning(LVec<Point, {MAX_AREA}>),
    // unit effects - only visible if the affected unit is visible
    Flame(Point),
    GunFire(Point),
    ShellFire(Point),
    Repair(Point),
    Explode(Point),
}
impl<D: Direction> Effect<D> {
    pub fn fog_replacement(&self, game: &Game<D>, team: ClientPerspective) -> Option<Self> {
        match self {
            Self::Flame(p) |
            Self::GunFire(p) |
            Self::Repair(p) |
            Self::Explode(p) |
            Self::ShellFire(p) => {
                let fog_intensity = game.get_fog_at(team, *p);
                if fog_intensity <= FogIntensity::NormalVision {
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

fn apply_vision_changes<D: Direction>(game: &mut Game<D>, team: ClientPerspective, pos: Point, intensity_before: FogIntensity, intensity: FogIntensity, change: &FieldData<D>) {
    game.set_fog(team, pos, intensity);
    let change = if intensity < intensity_before {
        change.clone()
    } else {
        change.clone().fog_replacement(intensity)
    };
    game.get_map_mut().set_terrain(pos, change.terrain);
    game.get_map_mut().set_details(pos, change.details.to_vec());
    game.get_map_mut().set_unit(pos, change.unit);
}

