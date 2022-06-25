use std::collections::{HashMap, HashSet};
use rand;

use crate::map::map::Map;
use crate::map::point::Point;
use crate::player::*;
use crate::terrain::Terrain;
use crate::units::*;
use crate::map::direction::Direction;
use crate::game::game::*;
use crate::units::mercenary::Mercenaries;
use super::events;

pub enum Command<D: Direction> {
    EndTurn,
    UnitCommand(UnitCommand<D>),
}
impl<D: Direction> Command<D> {
    pub fn convert(self, handler: &mut events::EventHandler<D>) -> Result<(), CommandError> {
        match self {
            Self::EndTurn => {
                for p in handler.get_map().wrapping_logic().pointmap().get_valid_points() {
                    match handler.get_map().get_unit(&p) {
                        Some(UnitType::Normal(unit)) => {
                            if unit.exhausted && unit.owner == handler.game.current_player().owner_id {
                                handler.add_event(Event::UnitExhaust(p));
                            }
                        }
                        Some(UnitType::Mercenary(merc)) => {
                            if merc.unit.exhausted && merc.unit.owner == handler.game.current_player().owner_id {
                                handler.add_event(Event::UnitExhaust(p));
                            }
                        }
                        Some(UnitType::Chess(unit)) => {
                            if unit.exhausted && unit.owner == handler.game.current_player().owner_id {
                                handler.add_event(Event::UnitExhaust(p));
                            }
                        }
                        Some(UnitType::Structure(_)) => {}
                        None => {}
                    }
                }
                handler.add_event(Event::NextTurn);
                for p in handler.get_map().wrapping_logic().pointmap().get_valid_points() {
                    match handler.get_map().get_unit(&p) {
                        Some(UnitType::Mercenary(merc)) => {
                            if merc.unit.owner == handler.game.current_player().owner_id {
                                match &merc.typ {
                                    Mercenaries::EarlGrey(true) => handler.add_event(Event::MercenaryPowerSimple(p)),
                                    _ => {}
                                }
                            }
                        }
                        _ => {}
                    }
                }
                match handler.get_game().get_fog_mode() {
                    FogMode::Random(value, offset, to_bright_chance, to_dark_chance) => {
                        if handler.get_game().current_turn() >= *offset as u32 {
                            let random_value:f32 = rand::random();
                            if *value && random_value < *to_bright_chance || !*value && random_value < *to_dark_chance {
                                handler.add_event(Event::FogFlipRandom);
                            }
                        }
                    }
                    _ => {}
                }
                handler.recalculate_fog(false);
                Ok(())
            }
            Self::UnitCommand(command) => command.convert(handler)
        }
    }
}

#[derive(Debug, Clone)]
pub enum CommandError {
    NoVision,
    MissingUnit,
    NotYourUnit,
    UnitCannotMove,
    UnitCannotCapture,
    UnitTypeWrong,
    InvalidPath,
    InvalidPoint(Point),
    InvalidTarget,
    PowerNotUsable,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Event<D:Direction> {
    NextTurn,
    FogFlipRandom,
    PureFogChange(Perspective, HashSet<Point>),
    FogChange(Perspective, HashMap<Point, (Terrain<D>, Option<UnitType<D>>)>),
    UnitPath(Vec<Option<Point>>, UnitType<D>),
    UnitExhaust(Point),
    UnitHpChange(Point, i8, i16),
    UnitDeath(Point, UnitType<D>),
    MercenaryCharge(Point, i8),
    MercenaryPowerSimple(Point),
    TerrainChange(Point, Terrain<D>, Terrain<D>),
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
            Self::UnitPath(path, unit) => {
                if let Some(p) = path.first().unwrap() {
                    game.get_map_mut().set_unit(p.clone(), None);
                }
                if let Some(p) = path.last().unwrap() {
                    game.get_map_mut().set_unit(p.clone(), Some(unit.clone()));
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
            Self::UnitHpChange(pos, hp_change, _) => {
                let unit = game.get_map_mut().get_unit_mut(pos).expect(&format!("expected a unit at {:?} to change hp by {}!", pos, hp_change));
                let hp = unit.get_hp_mut();
                *hp = (*hp as i8 + hp_change) as u8;
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
            Self::UnitPath(path, unit) => {
                if let Some(p) = path.last().unwrap() {
                    game.get_map_mut().set_unit(p.clone(), None);
                }
                if let Some(p) = path.first().unwrap() {
                    game.get_map_mut().set_unit(p.clone(), Some(unit.clone()));
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
            Self::UnitHpChange(pos, hp_change, _) => {
                let unit = game.get_map_mut().get_unit_mut(pos).expect(&format!("expected a unit at {:?} to change hp by {}!", pos, -hp_change));
                let hp = unit.get_hp_mut();
                *hp = (*hp as i8 - hp_change) as u8;
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
            Self::UnitPath(path, unit) => {
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
                if visible_path.len() > 0 {
                    Some(Self::UnitPath(visible_path, unit.clone()))
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
            Self::UnitHpChange(pos, _, _) => {
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

    pub fn recalculate_fog(&mut self, keep_current_team: bool) {
        let mut teams:HashSet<Option<Team>> = self.game.get_teams().into_iter().map(|team| Some(team)).collect();
        if keep_current_team {
            teams.remove(&Some(self.game.current_player().team()));
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
