use std::collections::HashMap;
use std::collections::HashSet;
use std::fmt;

use zipper::*;

use crate::config::environment::Environment;
use crate::game::commands::*;
use crate::game::event_handler::*;
use crate::game::fog::FogIntensity;
use crate::map::direction::Direction;
use crate::map::point::Point;
use crate::game::game::Game;
use super::attributes::ActionStatus;

use super::combat::AttackVector;
use super::hero::*;
use super::movement::Path;
use super::movement::PathSearchFeedback;
use super::movement::PathStep;
use super::movement::search_path;
use super::unit::Unit;
use super::unit_types::UnitType;

pub const UNIT_REPAIR: u32 = 30;

#[derive(Debug, Clone, PartialEq)]
pub enum UnitAction<D: Direction> {
    Wait,
    Enter,
    Capture,
    Repair,
    Attack(AttackVector<D>),
    //Pull(D),
    BuyMercenary(HeroType),
    MercenaryPowerSimple,
    //Castle,
    PawnUpgrade(UnitType),
    //BuildDrone(TransportableDrones),
    BuyTransportedUnit(UnitType),
    BuyUnit(UnitType, D),
}
impl<D: Direction> fmt::Display for UnitAction<D> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Wait => write!(f, "Wait"),
            Self::Enter => write!(f, "Enter"),
            Self::Capture => write!(f, "Capture"),
            Self::Repair => write!(f, "Repair"),
            Self::Attack(p) => write!(f, "Attack {:?}", p),
            //Self::Pull(_) => write!(f, "Pull"),
            Self::BuyMercenary(_) => write!(f, "Buy Mercenary"),
            Self::MercenaryPowerSimple => write!(f, "Activate Power"),
            //Self::Castle => write!(f, "Castle"),
            Self::PawnUpgrade(u) => write!(f, "{u:?}"),
            //Self::BuildDrone(o) => write!(f, "Build {}", o.to_normal(Some(0.into())).name()),
            Self::BuyTransportedUnit(unit) => write!(f, "Build {unit:?}"),
            Self::BuyUnit(unit, dir) => write!(f, "Build {unit:?} towards the {dir}"),
        }
    }
}

impl<D: Direction> UnitAction<D> {
    pub fn execute(&self, handler: &mut EventHandler<D>, end: Point, path: &Path<D>) {
        let needs_to_exhaust = match self {
            Self::Wait => true,
            Self::Enter => {
                let transporter = handler.get_map().get_unit(end).unwrap();
                let index = transporter.get_transported().len() - 1;
                handler.unit_status_boarded(end, index, ActionStatus::Exhausted);
                false
            }
            Self::Capture => {
                let terrain = handler.get_map().get_terrain(end).unwrap();
                if let Some(new_progress) = match terrain.get_capture_progress() {
                    Some((capturing_owner, _)) if capturing_owner.0 == handler.get_game().current_player().get_owner_id() => {
                        None
                    }
                    _ => Some((handler.get_game().current_player().get_owner_id().into(), 0.into()))
                } {
                    handler.terrain_capture_progress(end, Some(new_progress));
                }
                handler.unit_status(end, ActionStatus::Capturing);
                false
            }
            Self::Repair => {
                let unit = handler.get_map().get_unit(end).unwrap();
                let heal:u32 = UNIT_REPAIR
                    .min(100 - unit.get_hp() as u32)
                    .min(*handler.get_game().current_player().funds as u32 * 100 / unit.typ().price(handler.environment(), unit.get_owner_id()) as u32);
                if heal > 0 {
                    let cost = unit.typ().price(handler.environment(), unit.get_owner_id()) as u32 * heal / 100;
                    handler.money_buy(unit.get_owner_id(), cost);
                    handler.unit_repair(end, heal as u8);
                    handler.unit_status(end, ActionStatus::Repairing);
                    false
                } else {
                    true
                }
            }
            Self::Attack(attack_vector) => {
                attack_vector.execute(handler, end, Some(path), true, true, true);
                false
            }
            Self::BuyMercenary(hero_type) => {
                let unit = handler.get_map().get_unit(end).unwrap();
                let cost = hero_type.price(handler.environment(), &unit).unwrap();
                handler.money_change(unit.get_owner_id(), cost);
                handler.unit_set_hero(end, Hero::new(handler.environment(), *hero_type, Some(end)));
                true
            }
            Self::MercenaryPowerSimple => {
                let hero = handler.get_map().get_unit(end).unwrap().get_hero();
                let change = hero.get_charge();
                handler.hero_charge_sub(end, change.into());
                handler.hero_power_start(end);
                true
            }
            Self::PawnUpgrade(unit_type) => {
                let old_unit = handler.get_map().get_unit(end).unwrap();
                let new_unit = unit_type.instance(handler.environment())
                .copy_from(old_unit)
                .build_with_defaults();
                handler.unit_replace(end, new_unit);
                true
            }
            Self::BuyTransportedUnit(unit_type) => {
                let owner_id = handler.get_game().current_player().get_owner_id();
                let cost = unit_type.price(handler.environment(), owner_id);
                let unit = unit_type.instance(handler.environment())
                .set_owner_id(owner_id)
                .set_status(ActionStatus::Exhausted)
                .build_with_defaults();
                handler.money_buy(owner_id, cost as u32);
                handler.unit_add_transported(end, unit);
                true
            }
            Self::BuyUnit(unit_type, dir) => {
                let (destination, distortion) = handler.get_map().get_neighbor(end, *dir).unwrap();
                if handler.get_map().get_unit(destination).is_some() {
                    handler.effect_fog_surprise(destination);
                } else {
                    let owner_id = handler.get_game().current_player().get_owner_id();
                    let cost = unit_type.price(handler.environment(), owner_id);
                    let mut unit = unit_type.instance(handler.environment())
                    .set_owner_id(owner_id)
                    .set_status(ActionStatus::Exhausted)
                    .set_direction(distortion.update_direction(*dir));
                    if let Some(drone_id) = handler.get_map().get_unit(end).unwrap().get_drone_station_id() {
                        // TODO: only a drone-station should be able to build drones
                        unit = unit.set_drone_id(drone_id);
                    }
                    let unit = unit.build_with_defaults();
                    let path = Path {
                        start: end,
                        steps: vec![PathStep::Dir(*dir)],
                    };
                    handler.money_buy(owner_id, cost as u32);
                    let unit = handler.animate_unit_path(&unit, &path, false);
                    handler.unit_creation(destination, unit);
                }
                true
            }
        };
        if needs_to_exhaust {
            handler.unit_status(end, ActionStatus::Exhausted);
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct UnitCommand<D: Direction> {
    pub unload_index: Option<usize>,
    pub path: Path<D>,
    pub action: UnitAction<D>,
}

impl<D: Direction> UnitCommand<D> {
    pub fn execute(self, handler: &mut EventHandler<D>) -> Result<(), CommandError> {
        let start = self.path.start;
        let terrain = handler.get_map().get_terrain(start).ok_or(CommandError::InvalidPoint(start))?;
        let team = handler.get_game().current_player().get_team();
        let fog_intensity = handler.get_game().get_fog_at(team, start);
        let unit = handler.get_map().get_unit(start).and_then(|u| u.fog_replacement(handler.get_game(), start, fog_intensity)).ok_or(CommandError::MissingUnit)?;
        let unit = if let Some(index) = self.unload_index {
            let boarded = unit.get_transported();
            boarded.get(index).ok_or(CommandError::MissingBoardedUnit)?.clone()
        } else {
            unit
        };
        if handler.get_game().current_player().get_owner_id() != unit.get_owner_id() {
            return Err(CommandError::NotYourUnit);
        }
        if unit.is_exhausted() {
            return Err(CommandError::UnitCannotMove);
        }
        if self.unload_index.is_some() && self.path.steps.len() == 0 {
            return Err(CommandError::InvalidPath);
        }
        let fog = handler.get_game().get_fog().get(&team);
        let board_at_the_end = self.action == UnitAction::Enter;
        // check whether the path seemed possible for the player (ignores fog traps)
        if !search_path(handler.get_game(), &unit, &self.path, fog, |path, p, can_stop_here| {
            if *path == self.path && board_at_the_end {
                if let Some(transporter) = handler.get_map().get_unit(p) {
                    if p != path.start && transporter.can_transport(&unit) {
                        return PathSearchFeedback::Found;
                    }
                }
            } else if *path == self.path && !board_at_the_end && can_stop_here {
                return PathSearchFeedback::Found;
            }
            PathSearchFeedback::Rejected
        }).is_some() {
            return Err(CommandError::InvalidPath);
        }
        if !unit.options_after_path(handler.get_game(), &self.path).contains(&self.action) {
            return Err(CommandError::InvalidAction);
        }

        // now we know that the player entered a valid command
        // check for fog trap
        let mut path_taken = self.path.clone();
        let mut path_taken_works = !board_at_the_end && self.unload_index.is_none() && path_taken.steps.len() == 0;
        let mut fog_trap = None;
        while !path_taken_works {
            path_taken_works = Self::check_path(handler.get_game(), &unit, &path_taken, None, board_at_the_end);
            if path_taken.steps.len() == 0 {
                // doesn't matter if path_taken_works is true or not at this point
                break
            } else if !path_taken_works {
                fog_trap = Some(path_taken.end(handler.get_map()).unwrap().0);
                path_taken.steps.pop();
            }
        }
        if !path_taken_works {
            // don't know what to do, aaaaaah
            return Err(CommandError::InvalidPath);
        }
        if path_taken != self.path {
            // no event for the path is necessary if the unit is unable to move at all
            if path_taken.steps.len() > 0 {
                handler.unit_path(self.unload_index, &path_taken, false, false);
            }
            // fog trap
            handler.effect_fog_surprise(fog_trap.unwrap());
            // special case of a unit being unable to move that's loaded in a transport
            if path_taken.steps.len() == 0 && self.unload_index.is_some() {
                handler.unit_status_boarded(path_taken.start, self.unload_index.unwrap(), ActionStatus::Exhausted);
            } else {
                handler.unit_status(path_taken.end(handler.get_map())?.0, ActionStatus::Exhausted);
            }
        } else {
            if path_taken.steps.len() > 0 {
                handler.unit_path(self.unload_index, &path_taken, board_at_the_end, false);
            }
            let end = path_taken.end(handler.get_map()).unwrap().0;
            self.action.execute(handler, end, &path_taken);
        }
        exhaust_all_on_chess_board(handler, path_taken.start);
        Ok(())
    }

    pub fn check_path(game: &Game<D>, unit: &Unit<D>, path_taken: &Path<D>, vision: Option<&HashMap<Point, FogIntensity>>, board_at_the_end: bool) -> bool {
        search_path(game, unit, &path_taken, vision, |path, p, can_stop_here| {
            if path == path_taken && board_at_the_end {
                if let Some(transporter) = game.get_map().get_unit(p) {
                    if p != path.start && transporter.can_transport(unit) {
                        return PathSearchFeedback::Found;
                    }
                }
            } else if path == path_taken && !board_at_the_end && can_stop_here {
                return PathSearchFeedback::Found;
            }
            PathSearchFeedback::Rejected
        }).is_some()
    }
}

pub fn exhaust_all_on_chess_board<D: Direction>(handler: &mut EventHandler<D>, pos: Point) {
    if !handler.get_map().get_terrain(pos).and_then(|t| Some(t.is_chess())).unwrap_or(false) {
        return;
    }
    let owner_id = handler.get_game().current_player().get_owner_id();
    let mut to_exhaust = HashSet::new();
    handler.get_map().width_search(pos, |p| {
        let is_chess = handler.get_map().get_terrain(p).and_then(|t| Some(t.is_chess())).unwrap_or(false);
        if let Some(unit) = handler.get_map().get_unit(p) {
            if !unit.is_exhausted() && unit.get_owner_id() == owner_id && unit.can_have_status(ActionStatus::Exhausted) {
                to_exhaust.insert(p);
            }
        }
        is_chess
    });
    // order doesn't matter
    for p in to_exhaust {
        handler.unit_status(p, ActionStatus::Exhausted);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UnloadIndex(pub usize);

impl SupportedZippable<&Environment> for UnloadIndex {
    fn export(&self, zipper: &mut Zipper, support: &Environment) {
        zipper.write_u32(self.0 as u32, bits_needed_for_max_value(support.config.max_player_count() as u32));
    }
    fn import(unzipper: &mut Unzipper, support: &Environment) -> Result<Self, ZipperError> {
        Ok(Self(unzipper.read_u32(bits_needed_for_max_value(support.config.max_player_count() as u32))? as usize))
    }
}

impl From<usize> for UnloadIndex {
    fn from(value: usize) -> Self {
        Self(value)
    }
}

/*#[derive(Debug, Clone, PartialEq, Zippable)]
#[zippable(bits = 2)]
pub enum AttackInfo<D: Direction> {
    Point(Point),
    Direction(D)
}

pub type UnloadIndex = U<7>;

#[derive(Debug, Clone, PartialEq, Zippable)]
pub struct CommonMovement<D: Direction> {
    pub unload_index: Option<UnloadIndex>,
    pub path: Path<D>,
}
impl<D: Direction> CommonMovement<D> {
    pub fn new(unload_index: Option<u8>, path: Path<D>) -> Self {
        Self {
            unload_index: unload_index.and_then(|i| Some(i.into())),
            path,
        }
    }
    
    fn get_unit(&self, map: &Map<D>) -> Result<NormalUnit, CommandError> {
        let unit = map.get_unit(self.path.start).ok_or(CommandError::MissingUnit)?;
        let unit: NormalUnit = if let Some(index) = self.unload_index {
            let mut boarded = unit.get_boarded();
            if boarded.len() <= *index as usize {
                return Err(CommandError::MissingBoardedUnit);
            }
            boarded.remove(*index as usize)
        } else {
            match unit {
                UnitType::Normal(unit) => unit.clone(),
                _ => return Err(CommandError::UnitTypeWrong),
            }
        };
        Ok(unit)
    }

    fn intended_end(&self, map: &Map<D>) -> Result<Point, CommandError> {
        self.path.end(map)
    }

    fn validate_input(&self, game: &Game<D>, board_at_the_end: bool) -> Result<(), CommandError> {
        if !game.get_map().is_point_valid(self.path.start) {
            return Err(CommandError::InvalidPoint(self.path.start));
        }
        check_normal_unit_can_act(game, self.path.start, self.unload_index)?;
        if self.unload_index.is_some() && self.path.steps.len() == 0 {
            return Err(CommandError::InvalidPath);
        }
        let unit = self.get_unit(game.get_map())?;
        let team = unit.get_team(game);
        let fog = game.get_fog().get(&team);
        if Self::check_path(game, &unit, &self.path, fog, board_at_the_end) {
            Ok(())
        } else {
            Err(CommandError::InvalidPath)
        }
    }

    pub fn check_path(game: &Game<D>, unit: &NormalUnit, path_taken: &Path<D>, vision: Option<&HashMap<Point, FogIntensity>>, board_at_the_end: bool) -> bool {
        search_path(game, &unit.as_unit(), &path_taken, vision, |path, p, can_stop_here| {
            if path == path_taken && board_at_the_end {
                if let Some(transporter) = game.get_map().get_unit(p) {
                    if p != path.start && transporter.boardable_by(unit) {
                        return PathSearchFeedback::Found;
                    }
                }
            } else if path == path_taken && !board_at_the_end && can_stop_here {
                return PathSearchFeedback::Found;
            }
            PathSearchFeedback::Rejected
        }).is_some()
    }
    
    // returns the point the unit ends on unless it is stopped by a fog trap
    fn apply(&self, handler: &mut EventHandler<D>, mut board_at_the_end: bool, actively: bool) -> Result<Option<Point>, CommandError> {
        if let Ok(unit) = self.get_unit(handler.get_map()) {
            let mut path_taken = self.path.clone();
            let mut path_taken_works = !board_at_the_end && self.unload_index.is_none() && path_taken.steps.len() == 0;
            while !path_taken_works {
                path_taken_works = Self::check_path(handler.get_game(), &unit, &path_taken, None, board_at_the_end);
                if path_taken.steps.len() == 0 {
                    // doesn't matter if path_taken_works is true or not at this point
                    break
                } else if !path_taken_works {
                    path_taken.steps.pop();
                    board_at_the_end = false;
                }
            }
            if path_taken_works {
                if path_taken != self.path {
                    // no event for the path is necessary if the unit is unable to move at all
                    if path_taken.steps.len() > 0 {
                        handler.unit_path(self.unload_index, &path_taken, board_at_the_end, !actively);
                    }
                    // special case of a unit being unable to move that's loaded in a transport
                    if path_taken.steps.len() == 0 && self.unload_index.is_some() {
                        handler.unit_exhaust_boarded(path_taken.start, self.unload_index.unwrap());
                    } else {
                        handler.unit_exhaust(path_taken.end(handler.get_map())?);
                    }
                    Ok(None)
                } else {
                    if path_taken.steps.len() > 0 {
                        handler.unit_path(self.unload_index, &path_taken, board_at_the_end, !actively);
                    }
                    Ok(Some(path_taken.end(handler.get_map())?))
                }
            } else {
                // how could this even be handled
                Err(CommandError::InvalidPath)
            }
        } else {
            Err(CommandError::MissingUnit)
        }
    }
}

#[derive(Debug, Zippable)]
#[zippable(bits = 8)]
pub enum UnitCommandOld<D: Direction> {
    MoveAttack(CommonMovement<D>, AttackInfo<D>),
    MovePull(CommonMovement<D>, D),
    MoveCapture(CommonMovement<D>),
    MoveRepair(CommonMovement<D>),
    MoveWait(CommonMovement<D>),
    MoveBuyMerc(CommonMovement<D>, MercenaryOption),
    MoveAboard(CommonMovement<D>),
    MoveChess(ChessCommand<D>),
    MercenaryPowerSimple(Point),
    MoveBuildDrone(CommonMovement<D>, TransportableDrones),
    StructureBuildDrone(Point, TransportableDrones),
    BuyUnit(Point, D, U<255>),
}

impl<D: Direction> UnitCommandOld<D> {
    pub fn convert(self, handler: &mut EventHandler<D>) -> Result<(), CommandError> {
        let team = handler.get_game().current_player().team;
        let chess_exhaust = match self {
            Self::MoveAttack(cm, target) => {
                let intended_end = cm.intended_end(handler.get_map())?;
                cm.validate_input(handler.get_game(), false)?;
                let unit = cm.get_unit(handler.get_map())?;
                match &target {
                    AttackInfo::Point(target) => {
                        let terrain = handler.get_map().get_terrain(*target).ok_or(CommandError::InvalidPoint(*target))?;
                        match unit.get_attack_type() {
                            AttackType::Straight(_, _) => return Err(CommandError::InvalidTarget),
                            _ => {}
                        }
                        let fog_intensity = handler.get_game().get_fog_at(ClientPerspective::Team(*team as u8), *target);
                        let target_unit = handler.get_map().get_unit(*target).and_then(|u| u.fog_replacement(terrain, fog_intensity)).ok_or(CommandError::MissingUnit)?;
                        if !unit.attackable_positions(handler.get_game(), intended_end, cm.path.steps.len() > 0).contains(target) {
                            return Err(CommandError::InvalidTarget);
                        }
                        if !unit.can_attack_unit(handler.get_game(), &target_unit, *target) {
                            return Err(CommandError::InvalidTarget);
                        }
                    }
                    AttackInfo::Direction(_) => {
                        match unit.get_attack_type() {
                            AttackType::Straight(_, _) => {
                                // TODO: check if this direction is a valid option
                                // can't use options_after_path here, because the direction might be blocked by something hidden in fog
                            },
                            _ => return Err(CommandError::InvalidTarget),
                        }
                    }
                }
                if let Some(end) = cm.apply(handler, false, true)? {
                    handle_attack(handler, &unit.as_unit(), &cm.path, &target)?;
                    if handler.get_game().get_map().get_unit(end).is_some() {
                        // ensured that the unit didn't die from counter attack
                        handler.unit_exhaust(end);
                    }
                }
                Some(cm.path.start)
            }
            Self::MovePull(cm, dir) => {
                let intended_end = cm.intended_end(handler.get_map())?;
                cm.validate_input(handler.get_game(), false)?;
                let unit = cm.get_unit(handler.get_map())?;
                if !unit.can_pull() {
                    return Err(CommandError::UnitCannotPull);
                }
                if let Some(end) = cm.apply(handler, false, true)? {
                    let mut pull_path = vec![];
                    let mut dp = OrientedPoint::new(intended_end.clone(), false, dir);
                    for _ in 0..2 {
                        if let Some(next_dp) = handler.get_map().get_neighbor(dp.point, dp.direction) {
                            dp = next_dp;
                            if handler.get_map().get_unit(dp.point).is_some() {
                                break;
                            }
                            pull_path.insert(0, PathStep::Dir(dp.direction.opposite_direction()));
                        }
                    }
                    if let Some(target) = handler.get_map().get_unit(dp.point).cloned() {
                        if pull_path.len() == 2 && handler.get_game().can_see_unit_at(ClientPerspective::Team(*team as u8), dp.point, &target, true) && target.can_be_pulled(handler.get_map(), dp.point) {
                            handler.unit_path(None, &Path {start: dp.point, steps: pull_path.try_into().unwrap()}, false, true);
                        }
                    }
                    handler.unit_exhaust(end);
                }
                Some(cm.path.start)
            }
            Self::MoveCapture(cm) => {
                let intended_end = cm.intended_end(handler.get_map())?;
                cm.validate_input(handler.get_game(), false)?;
                let unit = cm.get_unit(handler.get_map())?;
                if !unit.can_capture() {
                    return Err(CommandError::UnitCannotCapture);
                }
                let new_progress = match handler.get_map().get_terrain(intended_end) {
                    Some(Terrain::Realty(_, owner, old_progress)) => {
                        if ClientPerspective::Team(*team as u8) != handler.get_game().get_team(*owner) {
                            match old_progress {
                                CaptureProgress::Capturing(capturing_owner, _) if *capturing_owner == handler.get_game().current_player().owner_id => {
                                    *old_progress
                                }
                                _ => CaptureProgress::Capturing(handler.get_game().current_player().owner_id, 0.into()),
                            }
                        } else {
                            return Err(CommandError::CannotCaptureHere);
                        }
                    }
                    _ => {
                        return Err(CommandError::CannotCaptureHere);
                    }
                };
                if let Some(end) = cm.apply(handler, false, true)? {
                    handler.terrain_capture_progress(end, new_progress);
                    handler.unit_status(end, UnitActionStatus::Capturing);
                    handler.unit_exhaust(end);
                }
                Some(cm.path.start)
            }
            Self::MoveRepair(cm) => {
                let intended_end = cm.intended_end(handler.get_map())?;
                cm.validate_input(handler.get_game(), false)?;
                let mut unit = cm.get_unit(handler.get_map())?;
                if unit.get_hp() == 100 {
                    return Err(CommandError::CannotRepairHere);
                }
                match unit.get_movement(handler.get_map().get_terrain(cm.path.start).unwrap(), None).0 {
                    MovementType::Hover(hover_mode) => {
                        for step in &cm.path.hover_steps(handler.get_map(), hover_mode) {
                            step.update_normal_unit(&mut unit);
                        }
                    }
                    _ => (),
                }
                match handler.get_map().get_terrain(intended_end) {
                    Some(Terrain::Realty(realty, owner, _)) => {
                        if owner != &Some(unit.get_owner()) || !realty.can_repair(unit.get_type()) {
                            return Err(CommandError::CannotRepairHere);
                        }
                    }
                    _ => {
                        return Err(CommandError::CannotRepairHere);
                    }
                }
                if let Some(end) = cm.apply(handler, false, true)? {
                    let unit = handler.get_map().get_unit(end).unwrap();
                    let heal:u32 = 30
                        .min(100 - unit.get_hp() as u32)
                        .min(*handler.get_game().current_player().funds as u32 * 100 / unit.type_value() as u32);
                    if heal > 0 {
                        let cost = unit.type_value() as u32 * heal / 100;
                        handler.money_buy(unit.get_owner().unwrap(), cost);
                        handler.unit_repair(end, heal as u8);
                        handler.unit_status(end, UnitActionStatus::Repairing);
                    }
                    handler.unit_exhaust(end);
                }
                Some(cm.path.start)
            }
            Self::MoveWait(cm) => {
                cm.validate_input(handler.get_game(), false)?;
                if let Some(end) = cm.apply(handler, false, true)? {
                    handler.unit_exhaust(end);
                }
                Some(cm.path.start)
            }
            Self::MoveBuyMerc(cm, merc) => {
                cm.validate_input(handler.get_game(), false)?;
                if let Some(end) = cm.apply(handler, false, true)? {
                    let unit = if let Some(UnitType::Normal(unit)) = handler.get_map().get_unit(end) {
                        unit.clone()
                    } else {
                        return Err(CommandError::UnitTypeWrong);
                    };
                    let cost = if let Some(cost) = merc.price(handler.get_game(), &unit) {
                        cost as u32
                    } else {
                        return Err(CommandError::UnitTypeWrong);
                    };
                    if handler.get_game().can_buy_merc_at(handler.get_game().current_player(), end) && cost as i32 <= *handler.get_game().current_player().funds {
                        handler.money_buy(unit.owner, cost);
                        let mut new_unit = unit.clone();
                        new_unit.data.mercenary = MaybeMercenary::Some {
                            mercenary: merc.mercenary(),
                            origin: Some(end),
                        };
                        handler.unit_replace(end, new_unit.as_unit());
                    }
                    handler.unit_exhaust(end);
                }
                Some(cm.path.start)
            }
            Self::MoveAboard(cm) => {
                let intended_end = cm.intended_end(handler.get_map())?;
                cm.validate_input(handler.get_game(), true)?;
                let terrain = handler.get_map().get_terrain(intended_end).unwrap();
                let fog_intensity = handler.get_game().get_fog_at(ClientPerspective::Team(*team as u8), intended_end);
                let transporter = handler.get_map().get_unit(intended_end).and_then(|u| u.fog_replacement(terrain, fog_intensity)).ok_or(CommandError::MissingUnit)?;
                let unit = cm.get_unit(handler.get_map())?;
                if !transporter.boardable_by(&unit) {
                    return Err(CommandError::UnitCannotBeBoarded);
                }
                let load_index = transporter.get_boarded().len() as u8;
                if let Some(end) = cm.apply(handler, true, true)? {
                    handler.unit_exhaust_boarded(end, load_index.into());
                }
                Some(cm.path.start)
            }
            Self::MoveChess(chess_command) => {
                Some(chess_command.convert(handler)?)
            }
            Self::MercenaryPowerSimple(pos) => {
                if !handler.get_map().is_point_valid(pos) {
                    return Err(CommandError::InvalidPoint(pos));
                }
                let terrain = handler.get_map().get_terrain(pos).unwrap();
                let fog_intensity = handler.get_game().get_fog_at(ClientPerspective::Team(*team as u8), pos);
                match handler.get_map().get_unit(pos).and_then(|u| u.fog_replacement(terrain, fog_intensity)) {
                    Some(UnitType::Normal(unit)) => {
                        if let MaybeMercenary::Some{mercenary, ..} = &unit.data.mercenary {
                            if mercenary.can_use_simple_power(handler.get_game(), pos) {
                                let change = mercenary.charge();
                                handler.mercenary_charge_sub(pos, change.into());
                                handler.mercenary_power_start(pos);
                            } else {
                                return Err(CommandError::PowerNotUsable);
                            }
                        } else {
                            return Err(CommandError::PowerNotUsable);
                        }
                    },
                    None => return Err(CommandError::MissingUnit),
                    _ => return Err(CommandError::UnitTypeWrong),
                }
                None
            }
            Self::MoveBuildDrone(cm, option) => {
                cm.validate_input(handler.get_game(), false)?;
                let unit = cm.get_unit(handler.get_map())?;
                let (drone_id, mut existing_drones, capacity) = match &unit.typ {
                    NormalUnits::DroneBoat(drones, drone_id) => {
                        (*drone_id, drones.len(), drones.capacity())
                    }
                    NormalUnits::Carrier(drones, drone_id) => {
                        (*drone_id, drones.len(), drones.capacity())
                    }
                    _ => return Err(CommandError::UnitTypeWrong),
                };
                for p in handler.get_map().all_points() {
                    match handler.get_map().get_unit(p) {
                        Some(UnitType::Normal(NormalUnit {typ: NormalUnits::LightDrone(id), ..})) | 
                        Some(UnitType::Normal(NormalUnit {typ: NormalUnits::HeavyDrone(id), ..})) => {
                            if drone_id == *id {
                                existing_drones += 1;
                            }
                        }
                        _ => (),
                    }
                }
                // new drones can't be built if at max-capacity
                if existing_drones >= capacity {
                    return Err(CommandError::UnitCannotBeBoarded)
                }
                if let Some(end) = cm.apply(handler, false, true)? {
                    let unit = option.to_normal(Some(drone_id));
                    let cost = unit.value() as u32;
                    if *handler.get_game().current_player().funds >= cost as i32 {
                        handler.money_buy(handler.get_game().current_player().owner_id, cost);
                        handler.unit_build_drone(end, option);
                    }
                    handler.unit_exhaust(end);
                }
                Some(cm.path.start)
            }
            Self::StructureBuildDrone(pos, option) => {
                let unit = match handler.get_map().get_unit(pos) {
                    Some(UnitType::Structure(struc)) => struc.clone(),
                    _ => return Err(CommandError::UnitTypeWrong),
                };
                let drone_id = match &unit.typ {
                    Structures::DroneTower(owner, drones, drone_id) => {
                        if *owner != handler.get_game().current_player().owner_id {
                            return Err(CommandError::NotYourUnit);
                        }
                        // new drones can't be built if at max-capacity
                        let mut existing_drones = drones.len();
                        for p in handler.get_map().all_points() {
                            match handler.get_map().get_unit(p) {
                                Some(UnitType::Normal(NormalUnit {typ: NormalUnits::LightDrone(id), ..})) | 
                                Some(UnitType::Normal(NormalUnit {typ: NormalUnits::HeavyDrone(id), ..})) => {
                                    if drone_id == id {
                                        existing_drones += 1;
                                    }
                                }
                                _ => (),
                            }
                        }
                        if existing_drones >= drones.capacity() {
                            return Err(CommandError::UnitCannotBeBoarded)
                        }
                        *drone_id
                    }
                    _ => return Err(CommandError::UnitTypeWrong),
                };
                let unit = option.to_normal(Some(drone_id));
                let cost = unit.value() as u32;
                if *handler.get_game().current_player().funds >= cost as i32 {
                    handler.money_buy(handler.get_game().current_player().owner_id, cost);
                    handler.unit_build_drone(pos, option);
                }
                handler.unit_exhaust(pos);
                Some(pos)
            }
            Self::BuyUnit(pos, dir, index) => {
                if handler.get_game().get_fog_at(ClientPerspective::Team(*team as u8), pos) != FogIntensity::NormalVision {
                    return Err(CommandError::NoVision);
                }
                let unit = handler.get_map().get_unit(pos).ok_or(CommandError::MissingUnit)?;
                let owner = handler.get_game().current_player().owner_id;
                if unit.get_owner() != Some(owner) {
                    return Err(CommandError::NotYourUnit);
                }
                if unit.is_exhausted() {
                    return Err(CommandError::UnitCannotMove);
                }
                let options = match unit {
                    UnitType::Normal(NormalUnit { typ: NormalUnits::SwimmingFactory, .. }) => build_options_swimming_factory(handler.get_game(), owner, 0),
                    _ => return Err(CommandError::UnitTypeWrong),
                };
                let index = *index as usize;
                if index >= options.len() {
                    return Err(CommandError::InvalidIndex);
                }
                let (new_unit, cost) = options[index].clone();
                if cost as i32 > *handler.get_game().current_player().funds {
                    return Err(CommandError::NotEnoughMoney);
                }
                let mut path = Path::new(pos);
                path.steps.push(PathStep::Dir(dir));
                if !CommonMovement::check_path(handler.get_game(), &new_unit.cast_normal().unwrap(), &path, None, false) {
                    return Err(CommandError::InvalidPath);
                }
                handler.money_buy(owner, cost as u32);
                let new_unit = handler.animate_unit_path(&new_unit, &path, false);
                let path_end = path.end(handler.get_map()).unwrap();
                handler.unit_creation(path_end, new_unit);
                handler.unit_exhaust(path_end);
                handler.unit_exhaust(pos);
                Some(pos)
            }
        };
        if let Some(p) = chess_exhaust {
            ChessCommand::exhaust_all_on_chess_board(handler, p);
        }
        Ok(())
    }
}

// set path to None if this is a counter-attack
pub fn calculate_attack<D: Direction>(handler: &mut EventHandler<D>, attacker_pos: Point, target: &AttackInfo<D>, path: Option<&Path<D>>) -> Result<Vec<Point>, CommandError> {
    let is_counter = path.is_none();
    let attacker = handler.get_map().get_unit(attacker_pos).and_then(|u| Some(u.clone()));
    let attacker: &NormalUnit = match &attacker {
        Some(UnitType::Normal(unit)) => Ok(unit),
        Some(UnitType::Chess(_)) => Err(CommandError::UnitTypeWrong),
        Some(UnitType::Structure(_)) => Err(CommandError::UnitTypeWrong),
        Some(UnitType::Unknown) => Err(CommandError::NoVision),
        None => Err(CommandError::MissingUnit),
    }?;
    let mut potential_counters = vec![];
    let mut recalculate_fog = false;
    let mut charges = HashMap::new();
    let mut defenders = vec![];
    let mut dead_units = HashSet::new();
    for target in attacker.attack_splash(handler.get_map(), attacker_pos, target)? {
        if let Some(defender) = handler.get_map().get_unit(target) {
            let damage = defender.calculate_attack_damage(handler.get_game(), target, attacker_pos, attacker, path);
            if let Some((weapon, damage)) = damage {
                let hp = defender.get_hp();
                if !is_counter && defender.get_owner() != Some(attacker.get_owner()) {
                    for (p, _) in handler.get_map().mercenary_influence_at(attacker_pos, Some(attacker.get_owner())) {
                        let change = if p == attacker_pos {
                            3
                        } else {
                            1
                        };
                        charges.insert(p, charges.get(&p).unwrap_or(&0) + change);
                    }
                }
                defenders.push((target.clone(), defender.clone(), damage));
                let defender = defender.clone();
                handler.effect_weapon(target, weapon);
                handler.unit_damage(target.clone(), damage);
                if damage >= hp as u16 {
                    dead_units.insert(target);
                    handler.unit_death(target, true);
                    if handler.get_game().get_team(Some(attacker.get_owner())) != handler.get_game().get_team(defender.get_owner()) {
                        if let Some(commander) = handler.get_game().get_owning_player(attacker.get_owner()).and_then(|player| Some(player.commander.clone())) {
                            commander.after_killing_unit(handler, attacker.get_owner(), target, &defender);
                        }
                    }
                    recalculate_fog = true;
                } else {
                    potential_counters.push(target);
                }
            }
        }
    }
    // add charge to nearby mercs
    for (p, change) in charges {
        if !dead_units.contains(&p) {
            handler.mercenary_charge_add(p, change);
        }
    }
    // add charge to commanders of involved players
    if defenders.len() > 0 {
        let attacker_team = handler.get_game().get_team(Some(attacker.get_owner()));
        let mut charges = HashMap::new();
        for (_, defender, damage) in &defenders {
            if let Some(player) = defender.get_owner().and_then(|owner| handler.get_game().get_owning_player(owner)) {
                if ClientPerspective::Team(*player.team as u8) != attacker_team {
                    let commander_charge = defender.get_hp().min(*damage as u8) as u32 * defender.type_value() as u32 / 100;
                    let old_charge = charges.remove(&player.owner_id).unwrap_or(0);
                    charges.insert(player.owner_id, commander_charge + old_charge);
                    let old_charge = charges.remove(&attacker.get_owner()).unwrap_or(0);
                    charges.insert(attacker.get_owner(), commander_charge / 2 + old_charge);
                }
            }
        }
        for (owner, commander_charge) in charges {
            handler.commander_charge_add(owner, commander_charge);
        }
        if let Some(commander) = handler.get_game().get_owning_player(attacker.get_owner()).and_then(|player| Some(player.commander.clone())) {
            commander.after_attacking(handler, attacker_pos, attacker, defenders, is_counter);
        }
    }
    if recalculate_fog {
        handler.recalculate_fog();
    }
    Ok(potential_counters)
}

pub fn handle_attack<D: Direction>(handler: &mut EventHandler<D>, attacker: &UnitType<D>, path: &Path<D>, target: &AttackInfo<D>) -> Result<(), CommandError> {
    let attacker_pos = path.end(handler.get_map()).unwrap();
    let potential_counters = calculate_attack(handler, attacker_pos, target, Some(path))?;
    // counter attack
    for p in &potential_counters {
        let unit: &NormalUnit = match handler.get_map().get_unit(*p) {
            Some(UnitType::Normal(unit)) => unit,
            Some(UnitType::Chess(_)) => continue,
            Some(UnitType::Structure(_)) => continue,
            Some(UnitType::Unknown) => continue,
            None => continue,
        };
        if !handler.get_game().can_see_unit_at(unit.get_team(handler.get_game()), attacker_pos, attacker, true) {
            continue;
        }
        if !unit.attackable_positions(handler.get_game(), *p, false).contains(&attacker_pos) {
            continue;
        }
        // todo: if a straight attacker is counter-attacking another straight attacker, it should first try to reverse the direction
        if let Some(attack_info) = unit.make_attack_info(handler.get_game(), *p, attacker_pos) {
            // this may return an error, but we don't care about that
            calculate_attack(handler, *p, &attack_info, None).ok();
        }
    }
    Ok(())
}*/
