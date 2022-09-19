use std::collections::{HashMap, HashSet};

use zipper::*;
use zipper::zipper_derive::*;

use crate::commanders::{Charge, MAX_CHARGE, CommanderPower};
use crate::map::map::{Map, FieldData};
use crate::map::point::Point;
use crate::map::point_map;
use crate::{player::*, details};
use crate::terrain::Terrain;
use crate::details::Detail;
use crate::units::*;
use crate::map::direction::Direction;
use crate::game::game::*;
use crate::units::mercenary::Mercenaries;
use crate::units::chess::*;
use crate::units::commands::{UnitCommand, UnloadIndex};
use crate::units::transportable::TransportableTypes;
use crate::units::movement::Path;

#[derive(Debug, Zippable)]
#[zippable(bits = 8)]
pub enum Command<D: Direction> {
    EndTurn,
    UnitCommand(UnitCommand::<D>),
    BuyUnit(Point, U8::<255>),
    CommanderPowerSimple(CommanderPower),
}
impl<D: Direction> Command<D> {
    pub fn convert<R: Fn() -> f32>(self, handler: &mut EventHandler<D>, random: R) -> Result<(), CommandError> {
        let owner_id = handler.game.current_player().owner_id;
        match self {
            Self::EndTurn => {
                // un-exhaust units
                for p in handler.get_map().all_points() {
                    let unit = handler.get_map().get_unit(p);
                    if let Some(unit) = unit {
                        if unit.get_owner() == Some(&owner_id) {
                            let mut events = vec![];
                            if unit.is_exhausted() {
                                events.push(Event::UnitExhaust(p.clone()));
                            }
                            for (index, u) in unit.get_boarded().iter().enumerate() {
                                if u.is_exhausted() {
                                    events.push(Event::UnitExhaustBoarded(p.clone(), (index as u8).try_into().unwrap()));
                                }
                            }
                            for event in events {
                                handler.add_event(event);
                            }
                        }
                    }
                }
                let was_foggy = handler.get_game().is_foggy();

                handler.add_event(Event::NextTurn);
                
                // update fog manually if it's random
                match handler.get_game().get_fog_mode() {
                    FogMode::Random(value, offset, to_bright_chance, to_dark_chance) => {
                        if handler.get_game().current_turn() >= **offset as u32 {
                            let random_value= (random() * 180.) as u8;
                            if *value && random_value < **to_bright_chance || !*value && random_value < **to_dark_chance {
                                handler.add_event(Event::FogFlipRandom);
                            }
                        }
                    }
                    _ => {}
                }
                // hide / reveal player funds if fog started / ended
                if was_foggy != handler.get_game().is_foggy() {
                    // usually events have to be added immediately, but this list of events can't influence each other
                    let mut events: Vec<Event<D>> = vec![];
                    if was_foggy {
                        for player in handler.get_game().players.iter() {
                            events.push(Event::PureRevealFunds(player.owner_id));
                        }
                    } else {
                        for player in handler.get_game().players.iter() {
                            events.push(Event::PureHideFunds(player.owner_id));
                        }
                    }
                    for event in events {
                        handler.add_event(event);
                    }
                }

                // end merc powers
                for p in handler.get_map().all_points() {
                    match handler.get_map().get_unit(p) {
                        Some(UnitType::Mercenary(merc)) => {
                            if merc.unit.owner == owner_id {
                                match &merc.typ {
                                    Mercenaries::EarlGrey(true) => handler.add_event(Event::MercenaryPowerSimple(p)),
                                    _ => {}
                                }
                            }
                        }
                        _ => {}
                    }
                }
                
                handler.get_game().current_player().commander.clone().start_turn(handler, handler.get_game().current_player().owner_id);

                handler.start_turn();

                Ok(())
            }
            Self::UnitCommand(command) => command.convert(handler),
            Self::BuyUnit(pos, index) => {
                let team = Some(handler.get_game().current_player().team);
                if !handler.get_game().has_vision_at(team, pos) {
                    Err(CommandError::NoVision)
                } else if let Some(_) = handler.get_map().get_unit(pos) {
                    Err(CommandError::Blocked(pos))
                } else {
                    let mut bubble_data = None;
                    let details = handler.get_map().get_details(pos);
                    for (index, detail) in details.into_iter().enumerate() {
                        match detail {
                            Detail::FactoryBubble(owner) => {
                                if owner != owner_id {
                                    return Err(CommandError::NotYourBubble);
                                }
                                bubble_data = Some((
                                    crate::terrain::build_options_factory(handler.get_game(), owner_id, 0),
                                    Event::RemoveDetail(pos.clone(), (index as u8).try_into().unwrap(), detail.clone())
                                ));
                            }
                            _ => {}
                        }
                    }
                    if let Some((options, event)) = bubble_data {
                        if let Some((unit, cost)) = options.get(*index as usize) {
                            if *cost as i32 <= *handler.get_game().current_player().funds {
                                handler.add_event(Event::MoneyChange(owner_id, (-(*cost as i32)).try_into().unwrap()));
                                let u = unit.clone();
                                let vision_changes: Vec<Point> = unit.get_vision(handler.get_game(), pos).into_iter().filter(|p| !handler.get_game().has_vision_at(team, *p)).collect();
                                handler.add_event(Event::UnitCreation(pos, u)); 
                                if vision_changes.len() > 0 {
                                    handler.add_event(Event::PureFogChange(team, vision_changes.try_into().unwrap()));
                                }
                                handler.add_event(event);
                                Ok(())
                            } else {
                                Err(CommandError::NotEnoughMoney)
                            }
                        } else {
                            Err(CommandError::InvalidIndex)
                        }
                    } else if let Some(Terrain::Realty(realty, owner)) = handler.get_map().get_terrain(pos) {
                        if owner == &Some(owner_id) {
                            let options = realty.buildable_units(handler.get_game(), owner_id);
                            if let Some((unit, cost)) = options.get(*index as usize) {
                                if *cost as i32 <= *handler.get_game().current_player().funds {
                                    handler.add_event(Event::MoneyChange(owner_id, (-(*cost as i32)).try_into().unwrap()));
                                    let mut u = unit.clone();
                                    u.set_exhausted(true);
                                    let vision_changes: Vec<Point> = unit.get_vision(handler.get_game(), pos).into_iter().filter(|p| !handler.get_game().has_vision_at(team, *p)).collect();
                                    handler.add_event(Event::UnitCreation(pos, u)); 
                                    if vision_changes.len() > 0 {
                                        handler.add_event(Event::PureFogChange(team, vision_changes.try_into().unwrap()));
                                    }
                                    // TODO: increment counter for that realty
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
                if *handler.get_game().current_player().commander.charge() < *power.charge_cost() {
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
}

#[derive(Debug, Clone, PartialEq, Zippable)]
#[zippable(bits = 8)]
pub enum Event<D:Direction> {
    NextTurn,
    FogFlipRandom,
    PureFogChange(Perspective, LVec::<Point, {point_map::MAX_AREA}>),
    FogChange(Perspective, LVec::<(Point, FieldData::<D>), {point_map::MAX_AREA}>),
    UnitPath(Option::<Option::<UnloadIndex>>, Path::<D>, bool, UnitType::<D>),
    UnitPathInto(Option::<Option::<UnloadIndex>>, Path::<D>, UnitType::<D>),
    UnitExhaust(Point),
    UnitExhaustBoarded(Point, UnloadIndex),
    UnitHpChange(Point, I8::<-100, 99>, I16::<-999, 999>),
    UnitCreation(Point, UnitType::<D>),
    UnitDeath(Point, UnitType::<D>),
    MercenaryCharge(Point, I8::<{-(mercenary::MAX_CHARGE as i8)}, {mercenary::MAX_CHARGE as i8}>),
    MercenaryPowerSimple(Point),
    TerrainChange(Point, Terrain::<D>, Terrain::<D>),
    MoneyChange(Owner, Funds),
    PureHideFunds(Owner),
    HideFunds(Owner, Funds), // when fog starts
    PureRevealFunds(Owner),
    RevealFunds(Owner, Funds), // when fog ends
    RemoveDetail(Point, U8::<{details::MAX_STACK_SIZE as u8 - 1}>, Detail),
    ReplaceDetail(Point, LVec::<Detail, {details::MAX_STACK_SIZE}>, LVec::<Detail, {details::MAX_STACK_SIZE}>),
    Effect(Effect),
    CommanderCharge(Owner, I32::<{-(MAX_CHARGE as i32)}, {MAX_CHARGE as i32}>),
    CommanderFlipActiveSimple(Owner),
    UnitMovedThisGame(Point),
    EnPassantOpportunity(Point, Point),
    EnPassantOpportunityGone(Point, Point),
    UnitDirection(Point, D, D),
}
impl<D: Direction> Event<D> {
    pub fn apply(&self, game: &mut Game<D>) {
        match self {
            Self::PureFogChange(team, points) => {
                flip_fog(game, team, points.iter());
            }
            Self::FogFlipRandom => {
                game.flip_fog_state();
            }
            Self::FogChange(team, changes) => {
                flip_fog(game, team, changes.iter().map(|change| &change.0));
                for (pos, change) in changes.iter() {
                    apply_vision_changes(game, team, *pos, change.clone());
                }
            }
            Self::NextTurn => game.current_turn += 1,
            Self::UnitPath(unload_index, path, end_visible, unit) => {
                if let Some(unload_index) = unload_index {
                    if let Some(index) = unload_index {
                        if let Some(unit) = game.get_map_mut().get_unit_mut(path.start) {
                            unit.unboard(**index);
                        }
                    } else {
                        game.get_map_mut().set_unit(path.start.clone(), None);
                    }
                }
                if *end_visible {
                    let end = path.end(game.get_map()).unwrap();
                    game.get_map_mut().set_unit(end, Some(unit.clone()));
                }
            }
            Self::UnitPathInto(unload_index, path, unit) => {
                if let Some(unload_index) = unload_index {
                    if let Some(index) = unload_index {
                        if let Some(unit) = game.get_map_mut().get_unit_mut(path.start) {
                            unit.unboard(**index);
                        }
                    } else {
                        game.get_map_mut().set_unit(path.start.clone(), None);
                    }
                }
                let end = path.end(game.get_map()).unwrap();
                let transporter = game.get_map_mut().get_unit_mut(end).unwrap();
                transporter.board(transporter.get_boarded().len() as u8, unit.clone().as_transportable().unwrap());
            }
            Self::UnitExhaust(pos) => {
                let unit = game.get_map_mut().get_unit_mut(*pos).expect(&format!("expected a unit at {:?} to (un)exhaust!", pos));
                match unit {
                    UnitType::Normal(unit) => unit.exhausted = !unit.exhausted,
                    UnitType::Mercenary(unit) => unit.unit.exhausted = !unit.unit.exhausted,
                    UnitType::Chess(unit) => unit.exhausted = !unit.exhausted,
                    UnitType::Structure(unit) => unit.exhausted = !unit.exhausted,
                }
            }
            Self::UnitExhaustBoarded(pos, index) => {
                let transporter = game.get_map_mut().get_unit_mut(*pos).expect(&format!("expected a unit at {:?} to (un)exhaust!", pos));
                let mut transported = transporter.get_boarded_mut();
                match transported.get_mut(**index as usize) {
                    Some(TransportableTypes::Normal(u)) => u.exhausted = !u.exhausted,
                    Some(TransportableTypes::Mercenary(m)) => m.unit.exhausted = !m.unit.exhausted,
                    None => {}
                }
            }
            Self::UnitHpChange(pos, hp_change, _) => {
                let unit = game.get_map_mut().get_unit_mut(*pos).expect(&format!("expected a unit at {:?} to change hp by {}!", pos, hp_change));
                let hp = unit.get_hp();
                unit.set_hp((hp as i8 + **hp_change) as u8);
            }
            Self::UnitCreation(pos, unit) => {
                game.get_map_mut().set_unit(pos.clone(), Some(unit.clone()));
            }
            Self::UnitDeath(pos, _) => {
                game.get_map_mut().set_unit(pos.clone(), None).expect(&format!("expected a unit at {:?} to die!", pos));
            }
            Self::MercenaryCharge(pos, change) => {
                if let Some(UnitType::Mercenary(merc)) = game.get_map_mut().get_unit_mut(*pos) {
                    merc.charge = ((*merc.charge as i8 + **change).max(0).min(merc.typ.max_charge() as i8) as u8).try_into().unwrap();
                }
            }
            Self::MercenaryPowerSimple(pos) => {
                if let Some(UnitType::Mercenary(merc)) = game.get_map_mut().get_unit_mut(*pos) {
                    match &mut merc.typ {
                        Mercenaries::EarlGrey(power_active) => {
                            *power_active = !*power_active;
                        }
                    }
                }
            }
            Self::TerrainChange(pos, _, terrain) => {
                game.get_map_mut().set_terrain(pos.clone(), terrain.clone());
            }
            Self::MoneyChange(owner, change) => {
                if let Some(player) = game.get_owning_player_mut(owner) {
                    player.funds = (*player.funds + **change).try_into().unwrap();
                }
            }
            Self::PureHideFunds(_) => {}
            Self::HideFunds(owner, _) => {
                if let Some(player) = game.get_owning_player_mut(owner) {
                    player.funds = Funds::new(0);
                }
            }
            Self::PureRevealFunds(_) => {}
            Self::RevealFunds(owner, value) => {
                if let Some(player) = game.get_owning_player_mut(owner) {
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
                game.get_owning_player_mut(owner).unwrap().commander.add_charge(**delta);
            }
            Self::CommanderFlipActiveSimple(owner) => {
                game.get_owning_player_mut(owner).unwrap().commander.flip_active();
            }
            Self::UnitMovedThisGame(p) => {
                if let Some(UnitType::Chess(unit)) = game.get_map_mut().get_unit_mut(*p) {
                    unit.typ.flip_moved_this_game();
                }
            }
            Self::EnPassantOpportunity(p, passed_over) => {
                if let Some(UnitType::Chess(unit)) = game.get_map_mut().get_unit_mut(*p) {
                    match &mut unit.typ {
                        ChessUnits::Pawn(_, _, p) => {
                            *p = Some(*passed_over);
                        }
                        _ => {}
                    }
                }
            }
            Self::EnPassantOpportunityGone(p, _) => {
                if let Some(UnitType::Chess(unit)) = game.get_map_mut().get_unit_mut(*p) {
                    match &mut unit.typ {
                        ChessUnits::Pawn(_, _, p) => {
                            *p = None;
                        }
                        _ => {}
                    }
                }
            }
            Self::UnitDirection(p, new_dir, _) => {
                if let Some(UnitType::Chess(unit)) = game.get_map_mut().get_unit_mut(*p) {
                    match &mut unit.typ {
                        ChessUnits::Pawn(d, _, _) => {
                            *d = *new_dir;
                        }
                        _ => {}
                    }
                }
            }
        }
    }
    pub fn undo(&self, game: &mut Game<D>) {
        match self {
            Self::PureFogChange(team, points) => {
                flip_fog(game, team, points.iter());
            }
            Self::FogFlipRandom => {
                game.flip_fog_state();
            }
            Self::FogChange(team, changes) => {
                flip_fog(game, team, changes.iter().map(|change| &change.0));
                for (pos, change) in changes.iter() {
                    apply_vision_changes(game, team, *pos, change.clone());
                }
            }
            Self::NextTurn => game.current_turn -= 1,
            Self::UnitPath(unload_index, path, end_visible, unit) => {
                if *end_visible {
                    let end = path.end(game.get_map()).unwrap();
                    game.get_map_mut().set_unit(end, None);
                }
                if let Some(unload_index) = unload_index {
                    if let Some(index) = unload_index {
                        if let (Some(u), Some(b)) = (game.get_map_mut().get_unit_mut(path.start), unit.clone().as_transportable()) {
                            u.board(**index, b);
                        }
                    } else {
                        game.get_map_mut().set_unit(path.start.clone(), Some(unit.clone()));
                    }
                }
            }
            Self::UnitPathInto(unload_index, path, unit) => {
                let end = path.end(game.get_map()).unwrap();
                let transporter = game.get_map_mut().get_unit_mut(end).unwrap();
                transporter.unboard(transporter.get_boarded().len() as u8 - 1);
                if let Some(unload_index) = unload_index {
                    if let Some(index) = unload_index {
                        if let (Some(u), Some(b)) = (game.get_map_mut().get_unit_mut(path.start), unit.clone().as_transportable()) {
                            u.board(**index, b);
                        }
                    } else {
                        game.get_map_mut().set_unit(path.start.clone(), Some(unit.clone()));
                    }
                }
            }
            Self::UnitExhaust(pos) => {
                let unit = game.get_map_mut().get_unit_mut(*pos).expect(&format!("expected a unit at {:?} to (un)exhaust!", pos));
                match unit {
                    UnitType::Normal(unit) => unit.exhausted = !unit.exhausted,
                    UnitType::Mercenary(unit) => unit.unit.exhausted = !unit.unit.exhausted,
                    UnitType::Chess(unit) => unit.exhausted = !unit.exhausted,
                    UnitType::Structure(unit) => unit.exhausted = !unit.exhausted,
                }
            }
            Self::UnitExhaustBoarded(pos, index) => {
                let transporter = game.get_map_mut().get_unit_mut(*pos).expect(&format!("expected a unit at {:?} to (un)exhaust!", pos));
                let mut transported = transporter.get_boarded_mut();
                match transported.get_mut(**index as usize) {
                    Some(TransportableTypes::Normal(u)) => u.exhausted = !u.exhausted,
                    Some(TransportableTypes::Mercenary(m)) => m.unit.exhausted = !m.unit.exhausted,
                    None => {}
                }
            }
            Self::UnitHpChange(pos, hp_change, _) => {
                let unit = game.get_map_mut().get_unit_mut(*pos).expect(&format!("expected a unit at {:?} to change hp by {}!", pos, -**hp_change));
                let hp = unit.get_hp();
                unit.set_hp((hp as i8 - **hp_change) as u8);
            }
            Self::UnitCreation(pos, _) => {
                game.get_map_mut().set_unit(pos.clone(), None).expect(&format!("expected a unit at {:?} to die!", pos));
            }
            Self::UnitDeath(pos, unit) => {
                game.get_map_mut().set_unit(pos.clone(), Some(unit.clone()));
            }
            Self::MercenaryCharge(pos, change) => {
                if let Some(UnitType::Mercenary(merc)) = game.get_map_mut().get_unit_mut(*pos) {
                    merc.charge = ((*merc.charge as i8 - **change).max(0).min(merc.typ.max_charge() as i8) as u8).try_into().unwrap();
                }
            }
            Self::MercenaryPowerSimple(pos) => {
                if let Some(UnitType::Mercenary(merc)) = game.get_map_mut().get_unit_mut(*pos) {
                    match &mut merc.typ {
                        Mercenaries::EarlGrey(power_active) => {
                            *power_active = !*power_active;
                        }
                    }
                }
            }
            Self::TerrainChange(pos, terrain, _) => {
                game.get_map_mut().set_terrain(pos.clone(), terrain.clone());
            }
            Self::MoneyChange(owner, change) => {
                if let Some(player) = game.get_owning_player_mut(owner) {
                    player.funds = (*player.funds - **change).try_into().unwrap();
                }
            }
            Self::PureHideFunds(_) => {}
            Self::HideFunds(owner, value) => {
                if let Some(player) = game.get_owning_player_mut(owner) {
                    player.funds = *value;
                }
            }
            Self::PureRevealFunds(_) => {}
            Self::RevealFunds(owner, _) => {
                if let Some(player) = game.get_owning_player_mut(owner) {
                    player.funds = Funds::new(0);
                }
            }
            Self::RemoveDetail(p, index, detail) => {
                game.get_map_mut().insert_detail(p.clone(), **index as usize, detail.clone());
            }
            Self::ReplaceDetail(p, list, _) => {
                game.get_map_mut().set_details(p.clone(), list.iter().cloned().collect());
            }
            Self::Effect(_) => {}
            Self::CommanderCharge(owner, delta) => {
                game.get_owning_player_mut(owner).unwrap().commander.add_charge(-**delta);
            }
            Self::CommanderFlipActiveSimple(owner) => {
                game.get_owning_player_mut(owner).unwrap().commander.flip_active();
            }
            Self::UnitMovedThisGame(p) => {
                if let Some(UnitType::Chess(unit)) = game.get_map_mut().get_unit_mut(*p) {
                    unit.typ.flip_moved_this_game();
                }
            }
            Self::EnPassantOpportunity(p, _) => {
                if let Some(UnitType::Chess(unit)) = game.get_map_mut().get_unit_mut(*p) {
                    match &mut unit.typ {
                        ChessUnits::Pawn(_, _, p) => {
                            *p = None;
                        }
                        _ => {}
                    }
                }
            }
            Self::EnPassantOpportunityGone(p, passed_over) => {
                if let Some(UnitType::Chess(unit)) = game.get_map_mut().get_unit_mut(*p) {
                    match &mut unit.typ {
                        ChessUnits::Pawn(_, _, p) => {
                            *p = Some(*passed_over);
                        }
                        _ => {}
                    }
                }
            }
            Self::UnitDirection(p, _, old_dir) => {
                if let Some(UnitType::Chess(unit)) = game.get_map_mut().get_unit_mut(*p) {
                    match &mut unit.typ {
                        ChessUnits::Pawn(d, _, _) => {
                            *d = *old_dir;
                        }
                        _ => {}
                    }
                }
            }
        }
    }
    fn fog_replacement(&self, game: &Game<D>, team: &Perspective) -> Option<Event<D>> {
        match self {
            Self::PureFogChange(t, points) => {
                if t == team {
                    let mut changes = LVec::new();
                    for p in points.iter() {
                        changes.push((p.clone(), game.get_map().get_field_data(*p))).unwrap();
                    }
                    Some(Self::FogChange(t.clone(), changes))
                } else {
                    None
                }
            }
            Self::FogFlipRandom => {
                Some(Self::FogFlipRandom)
            }
            Self::FogChange(_, _) => {
                panic!("FogChange should only ever be created as replacement for PureFogChange. It shouldn't be replaced itself!");
            }
            Self::NextTurn => Some(Self::NextTurn),
            Self::UnitPath(unload_index, path, _, unit) => {
                let visible_path: Option<Path<D>> = if unit.get_team(game) != *team {
                    path.fog_replacement(game, *team)
                } else {
                    Some(path.clone())
                };
                if let Some(visible_path) = visible_path {
                    let unload_index = if game.has_vision_at(*team, path.start) {
                        Some(unload_index.unwrap_or(None))
                    } else {
                        None
                    };
                    Some(Self::UnitPath(unload_index, visible_path, game.has_vision_at(*team, path.end(game.get_map()).unwrap()), unit.clone()))
                } else {
                    None
                }
            }
            Self::UnitPathInto(unload_index, path, unit) => {
                let visible_path: Option<Path<D>> = if unit.get_team(game) != *team {
                    path.fog_replacement(game, *team)
                } else {
                    Some(path.clone())
                };
                if let Some(visible_path) = visible_path {
                    let unload_index = if game.has_vision_at(*team, path.start) {
                        Some(unload_index.unwrap_or(None))
                    } else {
                        None
                    };
                    if game.has_vision_at(*team, path.end(game.get_map()).unwrap()) {
                        Some(Self::UnitPathInto(unload_index, visible_path.try_into().unwrap(), unit.clone()))
                    } else {
                        Some(Self::UnitPath(unload_index, visible_path.try_into().unwrap(), false, unit.clone()))
                    }
                } else {
                    None
                }
            }
            Self::UnitExhaust(pos) => {
                if game.has_vision_at(*team, *pos) {
                    Some(self.clone())
                } else {
                    None
                }
            }
            Self::UnitExhaustBoarded(pos, _) => {
                if game.has_vision_at(*team, *pos) {
                    Some(self.clone())
                } else {
                    None
                }
            }
            Self::UnitHpChange(pos, _, _) => {
                if game.has_vision_at(*team, *pos) {
                    Some(self.clone())
                } else {
                    None
                }
            }
            Self::UnitCreation(pos, _) => {
                if game.has_vision_at(*team, *pos) {
                    Some(self.clone())
                } else {
                    None
                }
            }
            Self::UnitDeath(pos, _) => {
                if game.has_vision_at(*team, *pos) {
                    Some(self.clone())
                } else {
                    None
                }
            }
            Self::MercenaryCharge(pos, _) => {
                if game.has_vision_at(*team, *pos) {
                    Some(self.clone())
                } else {
                    None
                }
            }
            Self::MercenaryPowerSimple(pos) => {
                if game.has_vision_at(*team, *pos) {
                    Some(self.clone())
                } else {
                    None
                }
            }
            Self::TerrainChange(pos, before, after) => {
                if game.has_vision_at(*team, *pos) {
                    Some(self.clone())
                } else {
                    let before = before.fog_replacement();
                    let after = after.fog_replacement();
                    if before != after {
                        Some(Self::TerrainChange(pos.clone(), before, after))
                    } else {
                        None
                    }
                }
            }
            Self::MoneyChange(owner, _) => {
                if !game.is_foggy() || *team == game.get_owning_player(owner).and_then(|p| Some(p.team)) {
                    Some(self.clone())
                } else {
                    None
                }
            }
            Self::PureHideFunds(owner) => {
                if *team != game.get_owning_player(owner).and_then(|p| Some(p.team)) {
                    Some(Self::HideFunds(owner.clone(), game.get_owning_player(owner).unwrap().funds))
                } else {
                    None
                }
            }
            Self::HideFunds(_, _) => {
                panic!("HideFunds should only ever be created as replacement for PureHideFunds. It shouldn't be replaced itself!");
            }
            Self::PureRevealFunds(owner) => {
                if *team != game.get_owning_player(owner).and_then(|p| Some(p.team)) {
                    Some(Self::RevealFunds(owner.clone(), game.get_owning_player(owner).unwrap().funds))
                } else {
                    None
                }
            }
            Self::RevealFunds(_, _) => {
                panic!("RevealFunds should only ever be created as replacement for PureRevealFunds. It shouldn't be replaced itself!");
            }
            Self::RemoveDetail(p, index, detail) => {
                if game.has_vision_at(*team, *p) {
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
                    Some(Self::RemoveDetail(p.clone(), new_index.try_into().unwrap(), detail))
                } else {
                    None
                }
            }
            Self::ReplaceDetail(p, old, new) => {
                if game.has_vision_at(*team, *p) {
                    Some(self.clone())
                } else {
                    let old: Vec<Detail> = old.iter().filter_map(|detail| {
                        detail.fog_replacement()
                    }).collect();
                    let new: Vec<Detail> = new.iter().filter_map(|detail| {
                        detail.fog_replacement()
                    }).collect();
                    if old != new {
                        Some(Self::ReplaceDetail(p.clone(), old.try_into().unwrap(), new.try_into().unwrap()))
                    } else {
                        None
                    }
                }
            }
            Self::Effect(effect) => {
                if !game.is_foggy() {
                    Some(self.clone())
                } else if let Some(effect) = effect.fog_replacement(game, *team) {
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
            Self::UnitMovedThisGame(p) => {
                if game.has_vision_at(*team, *p) {
                    Some(self.clone())
                } else {
                    None
                }
            }
            Self::EnPassantOpportunity(p, _) | Self::EnPassantOpportunityGone(p, _) => {
                if game.has_vision_at(*team, *p) {
                    Some(self.clone())
                } else {
                    None
                }
            }
            Self::UnitDirection(p, _, _) => {
                if game.has_vision_at(*team, *p) {
                    Some(self.clone())
                } else {
                    None
                }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Zippable)]
#[zippable(bits = 8)]
pub enum Effect {
    Flame(Point),
    GunFire(Point),
    ShellFire(Point),
}
impl Effect {
    pub fn fog_replacement<D: Direction>(&self, game: &Game<D>, team: Option<Team>) -> Option<Self> {
        match self {
            Self::Flame(p) |
            Self::GunFire(p) |
            Self::ShellFire(p) => {
                if game.has_vision_at(team, *p) {
                    Some(self.clone())
                } else {
                    None
                }
            }
        }
    }
}

fn flip_fog<'a, D: Direction, I: Iterator<Item = &'a Point>>(game: &mut Game<D>, team: &Perspective, positions: I) {
    let fog = game.get_fog_mut().get_mut(team).unwrap();
    for pos in positions {
        if fog.contains(pos) {
            fog.remove(pos);
        } else {
            fog.insert(pos.clone());
        }
    }
}

fn apply_vision_changes<D: Direction>(game: &mut Game<D>, team: &Perspective, pos: Point, mut change: FieldData<D>) {
    if !game.has_vision_at(*team, pos) {
        change = change.fog_replacement();
    }
    let FieldData {
        terrain,
        details,
        unit,
    } = change;
    game.get_map_mut().set_terrain(pos.clone(), terrain);
    game.get_map_mut().set_details(pos.clone(), details.to_vec());
    game.get_map_mut().set_unit(pos.clone(), unit);
}

pub struct EventHandler<'a, D: Direction> {
    game: &'a mut Game<D>,
    events: HashMap<Option<Perspective>, Vec<Event<D>>>,
}
impl<'a, D: Direction> EventHandler<'a, D> {
    pub fn new(game: &'a mut Game<D>) -> Self {
        let mut events = HashMap::new();
        events.insert(None, vec![]);
        events.insert(Some(None), vec![]);
        for team in game.get_teams() {
            events.insert(Some(Some(team)), vec![]);
        }
        EventHandler {
            game,
            events,
        }
    }
    pub fn get_game(&self) -> &Game<D> {
        &self.game
    }
    pub fn get_map(&self) -> &Map<D> {
        self.game.get_map()
    }
    pub fn add_event(&mut self, event: Event<D>) {
        event.apply(&mut self.game);
        for (key, events) in self.events.iter_mut() {
            if let Some(perspective) = key {
                if let Some(event) = event.fog_replacement(self.game, perspective) {
                    events.push(event);
                }
            }
        }
        self.events.get_mut(&None).unwrap().push(event);
    }
    pub fn accept(self) -> HashMap<Option<Perspective>, Vec<Event<D>>> {
        self.events
    }
    pub fn cancel(mut self) {
        while let Some(event) = self.events.get_mut(&None).unwrap().pop() {
            event.undo(&mut self.game);
        }
    }

    pub fn start_turn(&mut self) {
        self.recalculate_fog(false);

        // income from properties
        let mut income_factor = 0;
        for p in self.get_map().all_points() {
            match self.get_map().get_terrain(p) {
                Some(Terrain::Realty(realty, owner)) => {
                    if *owner == Some(self.game.current_player().owner_id) {
                        income_factor += realty.income_factor();
                    }
                }
                _ => {}
            }
        }
        self.add_event(Event::MoneyChange(self.game.current_player().owner_id, ((*self.game.current_player().income * income_factor) as i32).try_into().unwrap()));
    }
    pub fn recalculate_fog(&mut self, keep_current_team: bool) {
        let mut teams:HashSet<Option<Team>> = self.game.get_teams().into_iter().map(|team| Some(team)).collect();
        if keep_current_team {
            teams.remove(&Some(self.game.current_player().team));
        }
        teams.insert(None);
        for team in teams {
            let fog = self.game.recalculate_fog(team);
            let mut changes = HashSet::new();
            for p in fog.difference(self.game.get_fog().get(&team).unwrap()) {
                changes.insert(p.clone());
            }
            for p in self.game.get_fog().get(&team).unwrap().difference(&fog) {
                changes.insert(p.clone());
            }
            if changes.len() > 0 {
                let changes: Vec<Point> = changes.into_iter().collect();
                self.add_event(Event::PureFogChange(team, changes.try_into().unwrap()));
            }
        }
    }
}
