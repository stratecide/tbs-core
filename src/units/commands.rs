use std::fmt;

use zipper::*;
use zipper::zipper_derive::*;

use crate::details::Detail;
use crate::game::events::*;
use crate::map::wrapping_map::OrientedPoint;
use crate::map::direction::Direction;
use crate::map::point::Point;
use crate::game::game::Game;
use crate::terrain::*;
use super::chess::*;

use super::*;

#[derive(Debug, Clone)]
pub enum UnitAction<D: Direction> {
    Wait,
    Enter,
    Capture,
    Attack(AttackInfo::<D>),
    Pull(D),
    BuyMercenary(MercenaryOption),
    MercenaryPowerSimple,
    Castle,
    PawnUpgrade(chess::PawnUpgrade),
    Repair,
    BuildDrone(TransportableDrones),
}
impl<D: Direction> fmt::Display for UnitAction<D> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Wait => write!(f, "Wait"),
            Self::Enter => write!(f, "Enter"),
            Self::Capture => write!(f, "Capture"),
            Self::Attack(p) => write!(f, "Attack {:?}", p),
            Self::Pull(_) => write!(f, "Pull"),
            Self::BuyMercenary(_) => write!(f, "Buy Mercenary"),
            Self::MercenaryPowerSimple => write!(f, "Activate Power"),
            Self::Castle => write!(f, "Castle"),
            Self::PawnUpgrade(p) => write!(f, "{}", p),
            Self::Repair => write!(f, "Repair"),
            Self::BuildDrone(o) => write!(f, "Build {}", o.to_normal(Some(DroneId::new(0))).name()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Zippable)]
#[zippable(bits = 2)]
pub enum AttackInfo<D: Direction> {
    Point(Point),
    Direction(D)
}

pub type UnloadIndex = U8<7>;

#[derive(Debug, Clone, PartialEq, Zippable)]
pub struct CommonMovement<D: Direction> {
    pub unload_index: Option::<UnloadIndex>,
    pub path: Path::<D>,
}
impl<D: Direction> CommonMovement<D> {
    pub fn new(unload_index: Option<u8>, path: Path<D>) -> Self {
        Self {
            unload_index: unload_index.and_then(|i| Some(i.try_into().unwrap())),
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

    fn validate_input(&self, game: &Game<D>) -> Result<(), CommandError> {
        if !game.get_map().is_point_valid(self.path.start) {
            return Err(CommandError::InvalidPoint(self.path.start));
        }
        check_normal_unit_can_act(game, self.path.start, self.unload_index)?;
        if self.unload_index.is_some() && self.path.steps.len() == 0 {
            return Err(CommandError::InvalidPath);
        }
        let unit = self.get_unit(game.get_map())?;
        unit.check_path(game, &self.path)
    }
    
    // returns the point the unit ends on unless it is stopped by a fog trap
    fn apply(&self, handler: &mut EventHandler<D>, into: bool, actively: bool) -> Result<Option<Point>, CommandError> {
        if let Ok(unit) = self.get_unit(handler.get_map()) {
            let mut path_taken = self.path.clone();
            let mut path_taken_works = false;
            while !path_taken_works {
                movement_search(handler.get_game(), &unit, &path_taken, None, |_path, _, can_stop_here| {
                    if can_stop_here {
                        path_taken_works = true;
                    }
                    PathSearchFeedback::Found
                });
                if path_taken.steps.len() == 0 {
                    // doesn't matter if path_taken_works is true or not at this point
                    break
                } else if !path_taken_works {
                    path_taken.steps.pop();
                }
            }
            if path_taken_works {
                if path_taken != self.path {
                    // no event for the path is necessary if the unit is unable to move at all
                    if path_taken.steps.len() > 0 {
                        let unit = unit.as_unit();
                        Self::add_path(handler, self.unload_index, &path_taken, into, unit.clone(), actively);
                        after_path(handler, &path_taken, &unit);
                    }
                    // special case of a unit being unable to move that's loaded in a transport
                    if path_taken.steps.len() == 0 && self.unload_index.is_some() {
                        handler.add_event(Event::UnitExhaustBoarded(self.path.start, self.unload_index.unwrap()));
                    } else {
                        handler.add_event(Event::UnitExhaust(path_taken.end(handler.get_map())?));
                    }
                    Ok(None)
                } else {
                    if path_taken.steps.len() > 0 {
                        let unit = unit.as_unit();
                        Self::add_path(handler, self.unload_index, &path_taken, into, unit.clone(), actively);
                        after_path(handler, &path_taken, &unit);
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
    fn add_path(handler: &mut EventHandler<D>, unload_index: Option<UnloadIndex>, path: &Path<D>, into: bool, unit: UnitType<D>, actively: bool) {
        if actively {
            match &unit {
                UnitType::Normal(u) => {
                    let mut movement_type = u.get_movement(handler.get_map().get_terrain(path.start).unwrap()).0;
                    match movement_type {
                        MovementType::Hover(_) => {
                            let mut steps = LVec::new();
                            let mut current = path.start;
                            let mut prev_terrain = handler.get_map().get_terrain(current).unwrap();
                            for step in &path.steps {
                                current = step.progress(handler.get_map(), current).unwrap();
                                let terrain = handler.get_map().get_terrain(current).unwrap();
                                movement_type = terrain.update_movement_type(movement_type, prev_terrain).unwrap();
                                let on_sea = movement_type != MovementType::Hover(HoverMode::Land);
                                steps.push((on_sea, step.clone())).unwrap();
                                prev_terrain = terrain;
                            }
                            handler.add_event(Event::HoverPath(Some(unload_index), path.start, steps, Some(into), unit));
                            return;
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }
        handler.add_event(Event::UnitPath(Some(unload_index), path.clone(), Some(into), unit));
    }
}

fn after_path<D: Direction>(handler: &mut EventHandler<D>, path: &Path<D>, unit: &UnitType<D>) {
    let team = handler.get_game().current_player().team;
    if ClientPerspective::Team(*team) == unit.get_team(handler.get_game()) {
        let mut vision_changes = HashSet::new();
        for p in path.points(handler.get_map()).unwrap().into_iter().skip(1) {
            for p in unit.get_vision(handler.get_game(), p) {
                if !handler.get_game().has_vision_at(ClientPerspective::Team(*team), p) {
                    vision_changes.insert(p);
                }
            }
        }
        if vision_changes.len() > 0 {
            let vision_changes: Vec<Point> = vision_changes.into_iter().collect();
            handler.add_event(Event::PureFogChange(Some(team), vision_changes.try_into().unwrap()));
        }
    }
    on_path_details(handler, &path, &unit);
}

#[derive(Debug, Zippable)]
#[zippable(bits = 8)]
pub enum UnitCommand<D: Direction> {
    MoveAttack(CommonMovement::<D>, AttackInfo::<D>),
    MovePull(CommonMovement::<D>, D),
    MoveCapture(CommonMovement::<D>),
    MoveRepair(CommonMovement::<D>),
    MoveWait(CommonMovement::<D>),
    MoveBuyMerc(CommonMovement::<D>, MercenaryOption),
    MoveAboard(CommonMovement::<D>),
    MoveChess(Point, ChessCommand::<D>),
    MercenaryPowerSimple(Point),
    MoveBuildDrone(CommonMovement::<D>, TransportableDrones),
    StructureBuildDrone(Point, TransportableDrones),
}
impl<D: Direction> UnitCommand<D> {
    pub fn convert(self, handler: &mut EventHandler<D>) -> Result<(), CommandError> {
        let team = handler.get_game().current_player().team;
        let chess_exhaust = match self {
            Self::MoveAttack(cm, target) => {
                let intended_end = cm.intended_end(handler.get_map())?;
                cm.validate_input(handler.get_game())?;
                let unit = cm.get_unit(handler.get_map())?;
                match &target {
                    AttackInfo::Point(target) => {
                        if !handler.get_map().is_point_valid(*target) {
                            return Err(CommandError::InvalidPoint(target.clone()));
                        }
                        match unit.get_attack_type() {
                            AttackType::Straight(_, _) => return Err(CommandError::InvalidTarget),
                            _ => {}
                        }
                        if !handler.get_game().has_vision_at(ClientPerspective::Team(*team), *target) {
                            handler.get_map().get_unit(*target)
                            .and_then(|u| u.fog_replacement())
                            .ok_or(CommandError::NoVision)?;
                        }
                        if !unit.attackable_positions(handler.get_game(), intended_end, cm.path.steps.len() > 0).contains(target) {
                            return Err(CommandError::InvalidTarget);
                        }
                        let target_unit = handler.get_map().get_unit(*target).ok_or(CommandError::MissingUnit)?;
                        if !unit.can_attack_unit(handler.get_game(), target_unit) {
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
                    handle_attack(handler, end, &target)?;
                    if handler.get_game().get_map().get_unit(end).is_some() {
                        // ensured that the unit didn't die from counter attack
                        handler.add_event(Event::UnitExhaust(end));
                    }
                }
                Some(cm.path.start)
            }
            Self::MovePull(cm, dir) => {
                let intended_end = cm.intended_end(handler.get_map())?;
                cm.validate_input(handler.get_game())?;
                let unit = cm.get_unit(handler.get_map())?;
                if !unit.can_pull() {
                    return Err(CommandError::UnitCannotPull);
                }
                let (min_dist, max_dist) = match unit.get_attack_type() {
                    AttackType::Straight(min_dist, max_dist) => (min_dist, max_dist),
                    _ => {
                        return Err(CommandError::UnitCannotPull);
                    }
                };
                let mut pull_path = vec![];
                let mut dp = OrientedPoint::new(intended_end.clone(), false, dir);
                for i in 0..max_dist {
                    if let Some(next_dp) = handler.get_map().get_neighbor(dp.point, dp.direction) {
                        dp = next_dp;
                        if let Some(unit) = handler.get_map().get_unit(dp.point).cloned() {
                            if let Some(end) = cm.apply(handler, false, false)? {
                                if handler.get_game().has_vision_at(ClientPerspective::Team(*team), dp.point) {
                                    if i < min_dist - 1 || !unit.can_be_pulled(handler.get_map(), dp.point) {
                                        // can't pull if the target is already next to the unit
                                        return Err(CommandError::InvalidTarget);
                                    } else {
                                        // found a valid target
                                        let pull_path = Path {start: dp.point, steps: pull_path.try_into().unwrap()};
                                        handler.add_event(Event::UnitPath(Some(None), pull_path.clone(), Some(false), unit.clone()));
                                        after_path(handler, &pull_path, &unit);
                                    }
                                } else {
                                    // the pull is blocked by a unit that isn't visible to the player
                                    // maybe it should still be pulled?
                                }
                                handler.add_event(Event::UnitExhaust(end));
                            }
                            break;
                        }
                        pull_path.insert(0, PathStep::Dir(dp.direction.opposite_direction()));
                    }
                }
                Some(cm.path.start)
            }
            Self::MoveCapture(cm) => {
                let intended_end = cm.intended_end(handler.get_map())?;
                cm.validate_input(handler.get_game())?;
                let unit = cm.get_unit(handler.get_map())?;
                if !unit.can_capture() {
                    return Err(CommandError::UnitCannotCapture);
                }
                let (old_progress, new_progress) = match handler.get_map().get_terrain(intended_end) {
                    Some(Terrain::Realty(_, owner, old_progress)) => {
                        if ClientPerspective::Team(*team) != handler.get_game().get_team(*owner) {
                            (*old_progress, match old_progress {
                                CaptureProgress::Capturing(capturing_owner, _) if *capturing_owner == handler.get_game().current_player().owner_id => {
                                    *old_progress
                                }
                                _ => CaptureProgress::Capturing(handler.get_game().current_player().owner_id, U8::new(0)),
                            })
                        } else {
                            return Err(CommandError::CannotCaptureHere);
                        }
                    }
                    _ => {
                        return Err(CommandError::CannotCaptureHere);
                    }
                };
                if let Some(end) = cm.apply(handler, false, true)? {
                    //handler.add_event(Event::TerrainChange(end, handler.get_map().get_terrain(end).unwrap().clone(), Terrain::Realty(realty, Some(handler.get_game().current_player().owner_id))));
                    if old_progress != new_progress {
                        handler.add_event(Event::CaptureProgress(end, old_progress, new_progress));
                    }
                    handler.add_event(Event::UnitCapturing(end));
                    handler.add_event(Event::UnitExhaust(end));
                }
                Some(cm.path.start)
            }
            Self::MoveRepair(cm) => {
                let intended_end = cm.intended_end(handler.get_map())?;
                cm.validate_input(handler.get_game())?;
                let unit = cm.get_unit(handler.get_map())?;
                if unit.get_hp() == 100 {
                    return Err(CommandError::CannotRepairHere);
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
                        let cost = unit.type_value() as i32 * heal as i32 / 100;
                        handler.add_event(Event::MoneyChange(unit.get_owner().unwrap(), (-cost).try_into().unwrap()));
                        handler.add_event(Event::Effect(Effect::Repair(end)));
                        handler.add_event(Event::UnitHpChange(end, (heal as i8).try_into().unwrap(), (heal as i16).try_into().unwrap()));
                    }
                    handler.add_event(Event::UnitExhaust(end));
                }
                Some(cm.path.start)
            }
            Self::MoveWait(cm) => {
                cm.validate_input(handler.get_game())?;
                if let Some(end) = cm.apply(handler, false, true)? {
                    handler.add_event(Event::UnitExhaust(end));
                }
                Some(cm.path.start)
            }
            Self::MoveBuyMerc(cm, merc) => {
                cm.validate_input(handler.get_game())?;
                if let Some(end) = cm.apply(handler, false, true)? {
                    let unit = if let Some(UnitType::Normal(unit)) = handler.get_map().get_unit(end) {
                        unit.clone()
                    } else {
                        return Err(CommandError::UnitTypeWrong);
                    };
                    let cost = if let Some(cost) = merc.price(handler.get_game(), &unit) {
                        cost as i32
                    } else {
                        return Err(CommandError::UnitTypeWrong);
                    };
                    if handler.get_game().can_buy_merc_at(handler.get_game().current_player(), end) && cost <= *handler.get_game().current_player().funds {
                        handler.add_event(Event::MoneyChange(unit.owner, (-(cost as i32)).try_into().unwrap()));
                        handler.add_event(Event::UnitSetMercenary(end, merc.mercenary()));
                        // TODO: update vision ...
                    }
                    handler.add_event(Event::UnitExhaust(end));
                }
                Some(cm.path.start)
            }
            Self::MoveAboard(cm) => {
                let intended_end = cm.intended_end(handler.get_map())?;
                cm.validate_input(handler.get_game())?;
                if !handler.get_game().has_vision_at(ClientPerspective::Team(*handler.get_game().current_player().team), intended_end) {
                    return Err(CommandError::NoVision);
                }
                let unit = cm.get_unit(handler.get_map())?;
                let transporter = handler.get_map().get_unit(intended_end).ok_or(CommandError::MissingUnit)?;
                if !transporter.boardable_by(&unit) {
                    return Err(CommandError::UnitCannotBeBoarded);
                }
                let load_index = transporter.get_boarded().len() as u8;
                if let Some(end) = cm.apply(handler, true, true)? {
                    handler.add_event(Event::UnitExhaustBoarded(end, load_index.try_into().unwrap()));
                }
                Some(cm.path.start)
            }
            Self::MoveChess(start, chess_command) => {
                check_chess_unit_can_act(handler.get_game(), start)?;
                match handler.get_map().get_unit(start) {
                    Some(UnitType::Chess(unit)) => {
                        let unit = unit.clone();
                        chess_command.convert(start, &unit, handler)?;
                    },
                    _ => return Err(CommandError::UnitTypeWrong),
                }
                Some(start)
            }
            Self::MercenaryPowerSimple(pos) => {
                if !handler.get_map().is_point_valid(pos) {
                    return Err(CommandError::InvalidPoint(pos));
                }
                if !handler.get_game().has_vision_at(ClientPerspective::Team(*handler.get_game().current_player().team), pos) {
                    return Err(CommandError::NoVision);
                }
                match handler.get_map().get_unit(pos) {
                    Some(UnitType::Normal(unit)) => {
                        if let MaybeMercenary::Some{mercenary, ..} = &unit.data.mercenary {
                            if mercenary.can_use_simple_power(handler.get_game(), pos) {
                                let change = -(mercenary.charge() as i8);
                                handler.add_event(Event::MercenaryCharge(pos, change.try_into().unwrap()));
                                handler.add_event(Event::MercenaryPowerSimple(pos));
                            } else {
                                return Err(CommandError::PowerNotUsable);
                            }
                        } else {
                            return Err(CommandError::PowerNotUsable);
                        }
                    },
                    _ => return Err(CommandError::UnitTypeWrong),
                }
                None
            }
            Self::MoveBuildDrone(cm, option) => {
                cm.validate_input(handler.get_game())?;
                let unit = cm.get_unit(handler.get_map())?;
                let drone_id = match &unit.typ {
                    NormalUnits::DroneBoat(drones, drone_id) => {
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
                if let Some(end) = cm.apply(handler, false, true)? {
                    let unit = option.to_normal(Some(drone_id));
                    let cost = unit.value() as i32;
                    if *handler.get_game().current_player().funds >= cost {
                        handler.add_event(Event::MoneyChange(handler.get_game().current_player().owner_id, (-cost).try_into().unwrap()));
                        handler.add_event(Event::BuildDrone(end, option));
                    }
                    handler.add_event(Event::UnitExhaust(end));
                }
                Some(cm.path.start)
            }
            Self::StructureBuildDrone(pos, option) => {
                if !handler.get_game().has_vision_at(ClientPerspective::Team(*handler.get_game().current_player().team), pos) {
                    // you should have vision of your own structures
                    return Err(CommandError::NoVision);
                }
                let unit = match handler.get_map().get_unit(pos) {
                    Some(UnitType::Structure(struc)) => struc.clone(),
                    None => return Err(CommandError::MissingUnit),
                    _ => return Err(CommandError::UnitTypeWrong),
                };
                let drone_id = match &unit.typ {
                    Structures::DroneTower(Some((owner, drones, drone_id))) => {
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
                let cost = unit.value() as i32;
                if *handler.get_game().current_player().funds >= cost {
                    handler.add_event(Event::MoneyChange(handler.get_game().current_player().owner_id, (-cost).try_into().unwrap()));
                    handler.add_event(Event::BuildDrone(pos, option));
                }
                handler.add_event(Event::UnitExhaust(pos));
                Some(pos)
            }
        };
        if let Some(p) = chess_exhaust {
            ChessCommand::exhaust_all_on_board(handler, p);
        }
        Ok(())
    }
}

pub fn on_path_details<D: Direction>(handler: &mut EventHandler<D>, path_taken: &Path<D>, unit: &UnitType<D>) {
    for p in path_taken.points(handler.get_map()).unwrap() {
        let old_details = handler.get_map().get_details(p);
        let details: Vec<Detail> = old_details.clone().into_iter().filter(|detail| {
            match detail {
                Detail::Coins1 => {
                    if let Some(owner) = unit.get_owner() {
                        if let Some(player) = handler.get_game().get_owning_player(owner) {
                            handler.add_event(Event::MoneyChange(owner, (*player.income as i32 / 2).try_into().unwrap()));
                        }
                    }
                    false
                }
                Detail::Coins2 => {
                    if let Some(owner) = unit.get_owner() {
                        if let Some(player) = handler.get_game().get_owning_player(owner) {
                            handler.add_event(Event::MoneyChange(owner, (*player.income as i32).try_into().unwrap()));
                        }
                    }
                    false
                }
                Detail::Coins4 => {
                    if let Some(owner) = unit.get_owner() {
                        if let Some(player) = handler.get_game().get_owning_player(owner) {
                            handler.add_event(Event::MoneyChange(owner, (*player.income as i32 * 2).try_into().unwrap()));
                        }
                    }
                    false
                }
                Detail::FactoryBubble(owner) => {
                    Some(*owner) == unit.get_owner()
                }
                Detail::Skull(owner, _) => {
                    Some(*owner) == unit.get_owner()
                }
            }
        }).collect();
        if details != old_details {
            handler.add_event(Event::ReplaceDetail(p, old_details.try_into().unwrap(), details.try_into().unwrap()));
        }
    }
}

pub fn calculate_attack<D: Direction>(handler: &mut EventHandler<D>, attacker_pos: Point, target: &AttackInfo<D>, is_counter: bool) -> Result<Vec<Point>, CommandError> {
    let attacker = handler.get_map().get_unit(attacker_pos).and_then(|u| Some(u.clone()));
    let attacker: &NormalUnit = match &attacker {
        Some(UnitType::Normal(unit)) => Ok(unit),
        Some(UnitType::Chess(_)) => Err(CommandError::UnitTypeWrong),
        Some(UnitType::Structure(_)) => Err(CommandError::UnitTypeWrong),
        None => Err(CommandError::MissingUnit),
    }?;
    let mut potential_counters = vec![];
    let mut recalculate_fog = false;
    let mut charges = HashMap::new();
    let mut defenders = vec![];
    for target in attacker.attack_splash(handler.get_map(), attacker_pos, target)? {
        if let Some(defender) = handler.get_map().get_unit(target) {
            let damage = defender.calculate_attack_damage(handler.get_game(), target, attacker_pos, attacker, is_counter);
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
                handler.add_event(Event::Effect(weapon.effect(target)));
                handler.add_event(Event::UnitHpChange(target.clone(), (-(damage.min(hp as u16) as i8)).try_into().unwrap(), (-(damage as i16)).max(-999).try_into().unwrap()));
                if damage >= hp as u16 {
                    handler.add_event(Event::UnitDeath(target, handler.get_map().get_unit(target).unwrap().clone()));
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
    if defenders.len() > 0 {
        let attacker_team = handler.get_game().get_team(Some(attacker.get_owner()));
        let mut charges = HashMap::new();
        for (_, defender, damage) in &defenders {
            if let Some(player) = defender.get_owner().and_then(|owner| handler.get_game().get_owning_player(owner)) {
                if ClientPerspective::Team(*player.team) != attacker_team {
                    let commander_charge = defender.get_hp().min(*damage as u8) as u32 * defender.type_value() as u32 / 100;
                    let old_charge = charges.remove(&player.owner_id).unwrap_or(0);
                    charges.insert(player.owner_id, commander_charge + old_charge);
                    let old_charge = charges.remove(&attacker.get_owner()).unwrap_or(0);
                    charges.insert(attacker.get_owner(), commander_charge / 2 + old_charge);
                }
            }
        }
        for (owner, commander_charge) in charges {
            let commander_charge = commander_charge.min(handler.get_game().get_owning_player(owner).and_then(|player| Some(*player.commander.charge_potential())).unwrap_or(0));
            if commander_charge > 0 {
                handler.add_event(Event::CommanderCharge(owner, (commander_charge as i32).try_into().unwrap()));
            }
        }
        if let Some(commander) = handler.get_game().get_owning_player(attacker.get_owner()).and_then(|player| Some(player.commander.clone())) {
            commander.after_attacking(handler, attacker_pos, attacker, defenders, is_counter);
        }
    }
    for (p, change) in charges {
        if let Some(UnitType::Normal(unit)) = handler.get_map().get_unit(p) {
            if let MaybeMercenary::Some{mercenary, ..} = &unit.data.mercenary {
                let change = change.min(mercenary.max_charge() as i16 - change).max(-(mercenary.charge() as i16));
                if change != 0 {
                    handler.add_event(Event::MercenaryCharge(p, (change as i8).try_into().unwrap()));
                }
            }
        }
    }
    if recalculate_fog {
        handler.recalculate_fog(true);
    }
    Ok(potential_counters)
}

pub fn handle_attack<D: Direction>(handler: &mut EventHandler<D>, attacker_pos: Point, target: &AttackInfo<D>) -> Result<(), CommandError> {
    let potential_counters = calculate_attack(handler, attacker_pos, target, false)?;
    // counter attack
    for p in &potential_counters {
        let unit: &NormalUnit = match handler.get_map().get_unit(*p) {
            Some(UnitType::Normal(unit)) => unit,
            Some(UnitType::Chess(_)) => continue,
            Some(UnitType::Structure(_)) => continue,
            None => continue,
        };
        if !handler.get_game().has_vision_at(unit.get_team(handler.get_game()), attacker_pos) {
            continue;
        }
        if !unit.attackable_positions(handler.get_game(), *p, false).contains(&attacker_pos) {
            continue;
        }
        // todo: if a straight attacker is counter-attacking another straight attacker, it should first try to reverse the direction
        let attack_info = unit.make_attack_info(handler.get_game(), *p, attacker_pos).ok_or(CommandError::InvalidTarget)?;
        // this may return an error, but we don't care about that
        calculate_attack(handler, *p, &attack_info, true).ok();
    }

    Ok(())
}
