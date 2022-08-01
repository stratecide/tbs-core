use std::collections::{HashMap, HashSet};

use crate::map::map::Map;
use crate::map::point::Point;
use crate::player::*;
use crate::terrain::Terrain;
use crate::field_modifiers::FieldModifier;
use crate::units::*;
use crate::map::direction::Direction;
use crate::game::game::*;
use crate::units::mercenary::Mercenaries;
use super::events;

pub enum Command<D: Direction> {
    EndTurn,
    UnitCommand(UnitCommand<D>),
    BuyUnit(Point, u8),
}
impl<D: Direction> Command<D> {
    pub fn convert<R: Fn() -> f32>(self, handler: &mut events::EventHandler<D>, random: R) -> Result<(), CommandError> {
        let owner_id = handler.game.current_player().owner_id;
        match self {
            Self::EndTurn => {
                // un-exhaust units
                for p in handler.get_map().wrapping_logic().pointmap().get_valid_points() {
                    let unit = handler.get_map().get_unit(&p);
                    if let Some(unit) = unit {
                        if unit.get_owner() == Some(&owner_id) {
                            let mut events = vec![];
                            if unit.is_exhausted() {
                                events.push(Event::UnitExhaust(p.clone()));
                            }
                            for (index, u) in unit.get_boarded().iter().enumerate() {
                                if u.is_exhausted() {
                                    events.push(Event::UnitExhaustBoarded(p.clone(), index as u8));
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
                        if handler.get_game().current_turn() >= *offset as u32 {
                            let random_value:f32 = random();
                            if *value && random_value < *to_bright_chance || !*value && random_value < *to_dark_chance {
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
                        for player in &handler.get_game().players {
                            events.push(Event::RevealFunds(player.owner_id, player.funds));
                        }
                    } else {
                        for player in &handler.get_game().players {
                            events.push(Event::HideFunds(player.owner_id, player.funds));
                        }
                    }
                    for event in events {
                        handler.add_event(event);
                    }
                }
                // end merc powers
                for p in handler.get_map().wrapping_logic().pointmap().get_valid_points() {
                    match handler.get_map().get_unit(&p) {
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

                handler.start_turn();

                Ok(())
            }
            Self::UnitCommand(command) => command.convert(handler),
            Self::BuyUnit(pos, index) => {
                let team = Some(handler.get_game().current_player().team);
                if !handler.get_game().has_vision_at(team, &pos) {
                    Err(CommandError::NoVision)
                } else if let Some(_) = handler.get_map().get_unit(&pos) {
                    Err(CommandError::Blocked(pos))
                } else {
                    let mut bubble_data = None;
                    let fms = handler.get_map().get_field_modifiers(&pos);
                    for (index, fm) in fms.into_iter().enumerate() {
                        match fm {
                            FieldModifier::FactoryBubble(owner) => {
                                if owner != owner_id {
                                    return Err(CommandError::NotYourBubble);
                                }
                                bubble_data = Some((
                                    crate::terrain::build_options_factory(handler.get_game(), owner_id, 0),
                                    Event::RemoveFieldModifier(pos.clone(), index as u8, fm.clone())
                                ));
                            }
                            _ => {}
                        }
                    }
                    if let Some((options, event)) = bubble_data {
                        if let Some((unit, cost)) = options.get(index as usize) {
                            if *cost as i32 <= handler.get_game().current_player().funds {
                                handler.add_event(Event::MoneyChange(owner_id, -(*cost as i16)));
                                let u = unit.clone();
                                let vision_changes: HashSet<Point> = unit.get_vision(handler.get_game(), &pos).into_iter().filter(|p| !handler.get_game().has_vision_at(team, &p)).collect();
                                handler.add_event(Event::UnitCreation(pos, u)); 
                                if vision_changes.len() > 0 {
                                    handler.add_event(Event::PureFogChange(team, vision_changes));
                                }
                                handler.add_event(event);
                                Ok(())
                            } else {
                                Err(CommandError::NotEnoughMoney)
                            }
                        } else {
                            Err(CommandError::InvalidIndex)
                        }
                    } else if let Some(Terrain::Realty(realty, owner)) = handler.get_map().get_terrain(&pos) {
                        if owner == &Some(owner_id) {
                            let options = realty.buildable_units(handler.get_game(), owner_id);
                            if let Some((unit, cost)) = options.get(index as usize) {
                                if *cost as i32 <= handler.get_game().current_player().funds {
                                    handler.add_event(Event::MoneyChange(owner_id, -(*cost as i16)));
                                    let mut u = unit.clone();
                                    u.set_exhausted(true);
                                    let vision_changes: HashSet<Point> = unit.get_vision(handler.get_game(), &pos).into_iter().filter(|p| !handler.get_game().has_vision_at(team, &p)).collect();
                                    handler.add_event(Event::UnitCreation(pos, u)); 
                                    if vision_changes.len() > 0 {
                                        handler.add_event(Event::PureFogChange(team, vision_changes));
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
    UnitTypeWrong,
    InvalidPath,
    InvalidPoint(Point),
    InvalidTarget,
    InvalidIndex,
    PowerNotUsable,
    Blocked(Point),
    NotEnoughMoney,
    NotYourRealty,
    NotYourBubble,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Event<D:Direction> {
    NextTurn,
    FogFlipRandom,
    PureFogChange(Perspective, HashSet<Point>),
    FogChange(Perspective, HashMap<Point, (Terrain<D>, Option<UnitType<D>>)>),
    UnitPath(Option<u8>, Vec<Option<Point>>, UnitType<D>),
    UnitPathInto(Option<u8>, Vec<Option<Point>>, UnitType<D>),
    UnitExhaust(Point),
    UnitExhaustBoarded(Point, u8),
    UnitHpChange(Point, i8, i16),
    UnitCreation(Point, UnitType<D>),
    UnitDeath(Point, UnitType<D>),
    MercenaryCharge(Point, i8),
    MercenaryPowerSimple(Point),
    TerrainChange(Point, Terrain<D>, Terrain<D>),
    MoneyChange(Owner, i16),
    HideFunds(Owner, i32), // when fog starts
    RevealFunds(Owner, i32), // when fog ends
    RemoveFieldModifier(Point, u8, FieldModifier),
    ReplaceFieldModifiers(Point, Vec<FieldModifier>, Vec<FieldModifier>),
}
impl<D: Direction> Event<D> {
    pub fn apply(&self, game: &mut Game<D>) {
        match self {
            Self::PureFogChange(team, points) => {
                flip_fog(game, team, points);
            }
            Self::FogFlipRandom => {
                game.flip_fog_state();
            }
            Self::FogChange(team, changes) => {
                flip_fog(game, team, &changes.keys().map(|p| p.clone()).collect());
                for (pos, (terrain, unit)) in changes.iter() {
                    apply_vision_changes(game, pos, team, terrain, unit);
                }
            }
            Self::NextTurn => game.current_turn += 1,
            Self::UnitPath(unload_index, path, unit) => {
                if let Some(p) = path.first().unwrap() {
                    if let Some(index) = unload_index {
                        if let Some(unit) = game.get_map_mut().get_unit_mut(p) {
                            unit.unboard(*index);
                        }
                    } else {
                        game.get_map_mut().set_unit(p.clone(), None);
                    }
                }
                if let Some(p) = path.last().unwrap() {
                    game.get_map_mut().set_unit(p.clone(), Some(unit.clone()));
                }
            }
            Self::UnitPathInto(unload_index, path, unit) => {
                if let Some(p) = path.first().unwrap() {
                    if let Some(index) = unload_index {
                        if let Some(unit) = game.get_map_mut().get_unit_mut(p) {
                            unit.unboard(*index);
                        }
                    } else {
                        game.get_map_mut().set_unit(p.clone(), None);
                    }
                }
                let transporter = game.get_map_mut().get_unit_mut(&path.last().unwrap().unwrap()).unwrap();
                transporter.board(transporter.get_boarded().len() as u8, unit.clone().as_transportable().unwrap());
            }
            Self::UnitExhaust(pos) => {
                let unit = game.get_map_mut().get_unit_mut(pos).expect(&format!("expected a unit at {:?} to (un)exhaust!", pos));
                match unit {
                    UnitType::Normal(unit) => unit.exhausted = !unit.exhausted,
                    UnitType::Mercenary(unit) => unit.unit.exhausted = !unit.unit.exhausted,
                    UnitType::Chess(unit) => unit.exhausted = !unit.exhausted,
                    UnitType::Structure(unit) => unit.exhausted = !unit.exhausted,
                }
            }
            Self::UnitExhaustBoarded(pos, index) => {
                let transporter = game.get_map_mut().get_unit_mut(pos).expect(&format!("expected a unit at {:?} to (un)exhaust!", pos));
                let mut transported = transporter.get_boarded_mut();
                match transported.get_mut(*index as usize) {
                    Some(TransportableTypes::Normal(u)) => u.exhausted = !u.exhausted,
                    Some(TransportableTypes::Mercenary(m)) => m.unit.exhausted = !m.unit.exhausted,
                    None => {}
                }
            }
            Self::UnitHpChange(pos, hp_change, _) => {
                let unit = game.get_map_mut().get_unit_mut(pos).expect(&format!("expected a unit at {:?} to change hp by {}!", pos, hp_change));
                let hp = unit.get_hp_mut();
                *hp = (*hp as i8 + hp_change) as u8;
            }
            Self::UnitCreation(pos, unit) => {
                game.get_map_mut().set_unit(pos.clone(), Some(unit.clone()));
            }
            Self::UnitDeath(pos, _) => {
                game.get_map_mut().set_unit(pos.clone(), None).expect(&format!("expected a unit at {:?} to die!", pos));
            }
            Self::MercenaryCharge(pos, change) => {
                if let Some(UnitType::Mercenary(merc)) = game.get_map_mut().get_unit_mut(pos) {
                    merc.charge = (merc.charge as i8 + change).max(0).min(merc.typ.max_charge() as i8) as u8;
                }
            }
            Self::MercenaryPowerSimple(pos) => {
                if let Some(UnitType::Mercenary(merc)) = game.get_map_mut().get_unit_mut(pos) {
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
                    player.funds += *change as i32;
                }
            }
            Self::HideFunds(_owner, _) => {}
            Self::RevealFunds(owner, value) => {
                if let Some(player) = game.get_owning_player_mut(owner) {
                    player.funds = *value;
                }
            }
            Self::RemoveFieldModifier(p, index, _) => {
                game.get_map_mut().remove_field_modifier(p, *index as usize);
            }
            Self::ReplaceFieldModifiers(p, _, list) => {
                game.get_map_mut().set_field_modifiers(p.clone(), list.clone());
            }
        }
    }
    pub fn undo(&self, game: &mut Game<D>) {
        match self {
            Self::PureFogChange(team, points) => {
                flip_fog(game, team, points);
            }
            Self::FogFlipRandom => {
                game.flip_fog_state();
            }
            Self::FogChange(team, changes) => {
                flip_fog(game, team, &changes.keys().map(|p| p.clone()).collect());
                for (pos, (terrain, unit)) in changes.iter() {
                    apply_vision_changes(game, pos, team, terrain, unit);
                }
            }
            Self::NextTurn => game.current_turn -= 1,
            Self::UnitPath(unload_index, path, unit) => {
                if let Some(p) = path.last().unwrap() {
                    game.get_map_mut().set_unit(p.clone(), None);
                }
                if let Some(p) = path.first().unwrap() {
                    if let Some(index) = unload_index {
                        if let (Some(u), Some(b)) = (game.get_map_mut().get_unit_mut(p), unit.clone().as_transportable()) {
                            u.board(*index, b);
                        }
                    } else {
                        game.get_map_mut().set_unit(p.clone(), Some(unit.clone()));
                    }
                }
            }
            Self::UnitPathInto(unload_index, path, unit) => {
                let transporter = game.get_map_mut().get_unit_mut(&path.last().unwrap().unwrap()).unwrap();
                transporter.unboard(transporter.get_boarded().len() as u8 - 1);
                if let Some(p) = path.first().unwrap() {
                    if let Some(index) = unload_index {
                        if let (Some(u), Some(b)) = (game.get_map_mut().get_unit_mut(p), unit.clone().as_transportable()) {
                            u.board(*index, b);
                        }
                    } else {
                        game.get_map_mut().set_unit(p.clone(), Some(unit.clone()));
                    }
                }
            }
            Self::UnitExhaust(pos) => {
                let unit = game.get_map_mut().get_unit_mut(pos).expect(&format!("expected a unit at {:?} to (un)exhaust!", pos));
                match unit {
                    UnitType::Normal(unit) => unit.exhausted = !unit.exhausted,
                    UnitType::Mercenary(unit) => unit.unit.exhausted = !unit.unit.exhausted,
                    UnitType::Chess(unit) => unit.exhausted = !unit.exhausted,
                    UnitType::Structure(unit) => unit.exhausted = !unit.exhausted,
                }
            }
            Self::UnitExhaustBoarded(pos, index) => {
                let transporter = game.get_map_mut().get_unit_mut(pos).expect(&format!("expected a unit at {:?} to (un)exhaust!", pos));
                let mut transported = transporter.get_boarded_mut();
                match transported.get_mut(*index as usize) {
                    Some(TransportableTypes::Normal(u)) => u.exhausted = !u.exhausted,
                    Some(TransportableTypes::Mercenary(m)) => m.unit.exhausted = !m.unit.exhausted,
                    None => {}
                }
            }
            Self::UnitHpChange(pos, hp_change, _) => {
                let unit = game.get_map_mut().get_unit_mut(pos).expect(&format!("expected a unit at {:?} to change hp by {}!", pos, -hp_change));
                let hp = unit.get_hp_mut();
                *hp = (*hp as i8 - hp_change) as u8;
            }
            Self::UnitCreation(pos, _) => {
                game.get_map_mut().set_unit(pos.clone(), None).expect(&format!("expected a unit at {:?} to die!", pos));
            }
            Self::UnitDeath(pos, unit) => {
                game.get_map_mut().set_unit(pos.clone(), Some(unit.clone()));
            }
            Self::MercenaryCharge(pos, change) => {
                if let Some(UnitType::Mercenary(merc)) = game.get_map_mut().get_unit_mut(pos) {
                    merc.charge = (merc.charge as i8 - change).max(0).min(merc.typ.max_charge() as i8) as u8;
                }
            }
            Self::MercenaryPowerSimple(pos) => {
                if let Some(UnitType::Mercenary(merc)) = game.get_map_mut().get_unit_mut(pos) {
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
                    player.funds -= *change as i32;
                }
            }
            Self::HideFunds(owner, value) => {
                if let Some(player) = game.get_owning_player_mut(owner) {
                    player.funds = *value;
                }
            }
            Self::RevealFunds(_owner, _) => {}
            Self::RemoveFieldModifier(p, index, fm) => {
                game.get_map_mut().insert_field_modifier(p.clone(), *index as usize, fm.clone());
            }
            Self::ReplaceFieldModifiers(p, list, _) => {
                game.get_map_mut().set_field_modifiers(p.clone(), list.clone());
            }
        }
    }
    fn fog_replacement(&self, game: &Game<D>, team: &Perspective) -> Option<Event<D>> {
        match self {
            Self::PureFogChange(t, points) => {
                if t == team {
                    let mut changes = HashMap::new();
                    for p in points.clone() {
                        changes.insert(p, (game.get_map().get_terrain(&p).unwrap().clone(), game.get_map().get_unit(&p).and_then(|u| Some(u.clone()))));
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
            Self::UnitPath(unload_index, path, unit) => {
                let visible_path: Vec<Option<Point>> = if unit.get_team(game) == *team {
                    path.clone()
                } else {
                    build_visible_path(game, path, team)
                };
                if visible_path.len() > 0 {
                    let unload_index = if visible_path[0].is_some() {
                        *unload_index
                    } else {
                        None
                    };
                    Some(Self::UnitPath(unload_index, visible_path, unit.clone()))
                } else {
                    None
                }
            }
            Self::UnitPathInto(unload_index, path, unit) => {
                let visible_path: Vec<Option<Point>> = if unit.get_team(game) == *team {
                    path.clone()
                } else {
                    build_visible_path(game, path, team)
                };
                if visible_path.len() > 0 {
                    let unload_index = if visible_path[0].is_some() {
                        *unload_index
                    } else {
                        None
                    };
                    if game.has_vision_at(*team, &path.last().unwrap().unwrap()) {
                        Some(Self::UnitPathInto(unload_index, visible_path, unit.clone()))
                    } else {
                        Some(Self::UnitPath(unload_index, visible_path, unit.clone()))
                    }
                } else {
                    None
                }
            }
            Self::UnitExhaust(pos) => {
                if game.has_vision_at(*team, pos) {
                    Some(self.clone())
                } else {
                    None
                }
            }
            Self::UnitExhaustBoarded(pos, _) => {
                if game.has_vision_at(*team, pos) {
                    Some(self.clone())
                } else {
                    None
                }
            }
            Self::UnitHpChange(pos, _, _) => {
                if game.has_vision_at(*team, pos) {
                    Some(self.clone())
                } else {
                    None
                }
            }
            Self::UnitCreation(pos, _) => {
                if game.has_vision_at(*team, pos) {
                    Some(self.clone())
                } else {
                    None
                }
            }
            Self::UnitDeath(pos, _) => {
                if game.has_vision_at(*team, pos) {
                    Some(self.clone())
                } else {
                    None
                }
            }
            Self::MercenaryCharge(pos, _) => {
                if game.has_vision_at(*team, pos) {
                    Some(self.clone())
                } else {
                    None
                }
            }
            Self::MercenaryPowerSimple(pos) => {
                if game.has_vision_at(*team, pos) {
                    Some(self.clone())
                } else {
                    None
                }
            }
            Self::TerrainChange(pos, before, after) => {
                if game.has_vision_at(*team, pos) {
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
            Self::HideFunds(owner, _) => {
                if *team != game.get_owning_player(owner).and_then(|p| Some(p.team)) {
                    Some(self.clone())
                } else {
                    None
                }
            }
            Self::RevealFunds(owner, _) => {
                if *team != game.get_owning_player(owner).and_then(|p| Some(p.team)) {
                    Some(self.clone())
                } else {
                    None
                }
            }
            Self::RemoveFieldModifier(p, index, fm) => {
                if game.has_vision_at(*team, p) {
                    Some(self.clone())
                } else if let Some(fm) = fm.fog_replacement() {
                    let mut new_index = 0;
                    for (i, fm) in game.get_map().get_field_modifiers(p).into_iter().enumerate() {
                        if i == *index as usize {
                            break;
                        }
                        if fm.fog_replacement().is_some() {
                            new_index += 1;
                        }
                    }
                    Some(Self::RemoveFieldModifier(p.clone(), new_index, fm))
                } else {
                    None
                }
            }
            Self::ReplaceFieldModifiers(p, old, new) => {
                if game.has_vision_at(*team, p) {
                    Some(self.clone())
                } else {
                    let old: Vec<FieldModifier> = old.iter().filter_map(|fm| {
                        fm.fog_replacement()
                    }).collect();
                    let new: Vec<FieldModifier> = new.iter().filter_map(|fm| {
                        fm.fog_replacement()
                    }).collect();
                    if old != new {
                        Some(Self::ReplaceFieldModifiers(p.clone(), old, new))
                    } else {
                        None
                    }
                }
            }
        }
    }
}

fn flip_fog<D: Direction>(game: &mut Game<D>, team: &Perspective, positions: &HashSet<Point>) {
    let fog = game.get_fog_mut().get_mut(team).unwrap();
    for pos in positions {
        if fog.contains(pos) {
            fog.remove(pos);
        } else {
            fog.insert(pos.clone());
        }
    }
}
fn apply_vision_changes<D: Direction>(game: &mut Game<D>, pos: &Point, team: &Perspective, terrain: &Terrain<D>, unit: &Option<UnitType<D>>) {
    if game.has_vision_at(*team, pos) {
        game.get_map_mut().set_terrain(pos.clone(), terrain.clone());
        game.get_map_mut().set_unit(pos.clone(), unit.clone());
    } else {
        game.get_map_mut().set_terrain(pos.clone(), terrain.fog_replacement());
        game.get_map_mut().set_unit(pos.clone(), None);
    }
}
fn build_visible_path<D: Direction>(game: &Game<D>, path: &Vec<Option<Point>>, team: &Perspective) -> Vec<Option<Point>> {
    let mut visible_path = vec![];
    for (i, p) in path.iter().enumerate() {
        // since this is only called on events that haven't been replaced with a fog version, all points in the path are non-null
        if game.has_vision_at(*team, &p.unwrap()) {
            visible_path.push(p.clone());
        } else if i > 0 && game.has_vision_at(*team, &path[i - 1].unwrap()) {
            visible_path.push(p.clone());
            visible_path.push(None);
        } else if i < path.len() - 1 && game.has_vision_at(*team, &path[i + 1].unwrap()) {
            if i == 0 { // otherwise the previous case (i > 0 && ...) already added a None
                visible_path.push(None);
            }
            visible_path.push(p.clone());
        }
    }
    visible_path
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
        for p in self.get_map().wrapping_logic().pointmap().get_valid_points() {
            match self.get_map().get_terrain(&p) {
                Some(Terrain::Realty(realty, owner)) => {
                    if *owner == Some(self.game.current_player().owner_id) {
                        income_factor += realty.income_factor();
                    }
                }
                _ => {}
            }
        }
        self.add_event(Event::MoneyChange(self.game.current_player().owner_id, self.game.current_player().income * income_factor));
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
                self.add_event(Event::PureFogChange(team, changes));
            }
        }
    }
}
