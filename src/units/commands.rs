use std::fmt;

use crate::details::Detail;
use crate::game::events::*;
use crate::map::wrapping_map::{OrientedPoint};
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
    Attack(AttackInfo<D>),
    Pull(D),
    MercenaryPowerSimple(String),
}
impl<D: Direction> fmt::Display for UnitAction<D> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Wait => write!(f, "Wait"),
            Self::Enter => write!(f, "Enter"),
            Self::Capture => write!(f, "Capture"),
            Self::Attack(p) => write!(f, "Attack {:?}", p),
            Self::Pull(_) => write!(f, "Pull"),
            Self::MercenaryPowerSimple(name) => write!(f, "Activate \"{}\"", name),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum AttackInfo<D: Direction> {
    Point(Point),
    Direction(D)
}

pub struct CommonMovement {
    pub start: Point,
    pub unload_index: Option<u8>,
    pub path: Vec<Point>,
}
impl CommonMovement {
    pub fn new(start: Point, unload_index: Option<u8>, path: Vec<Point>) -> Self {
        Self {
            start,
            unload_index,
            path,
        }
    }
    fn intended_end(&self) -> Point {
        self.path.last().unwrap_or(&self.start).clone()
    }
    fn check_without_wait<D: Direction>(&self, game: &Game<D>) -> Result<Vec<Point>, CommandError> {
        if !game.get_map().is_point_valid(&self.start) {
            return Err(CommandError::InvalidPoint(self.start.clone()));
        }
        for p in &self.path {
            if !game.get_map().is_point_valid(p) {
                return Err(CommandError::InvalidPoint(p.clone()));
            }
        }
        check_normal_unit_can_act(game, &self.start, self.unload_index)?;
        let unit = game.get_map().get_unit(&self.start).ok_or(CommandError::MissingUnit)?;
        let unit: &dyn NormalUnitTrait<D> = if let Some(index) = self.unload_index {
            unit.get_boarded().get(index as usize).ok_or(CommandError::MissingBoardedUnit)?.as_trait()
        } else {
            unit.as_normal_trait().ok_or(CommandError::UnitTypeWrong)?
        };
        unit.check_path(game, &self.start, &self.path)
    }

    fn check_with_wait<D: Direction>(&self, game: &Game<D>) -> Result<Vec<Point>, CommandError> {
        let result = self.check_without_wait(game);
        if let Some(p) = &self.path.last() {
            if let Some(_) = game.get_map().get_unit(p) {
                if game.has_vision_at(Some(game.current_player().team), p) {
                    return Err(CommandError::InvalidPath);
                }
            }
        }
        result
    }
    fn get_unit<'a, D: Direction>(&self, map: &'a Map<D>) -> Result<&'a dyn NormalUnitTrait<D>, CommandError> {
        let unit = map.get_unit(&self.start).ok_or(CommandError::MissingUnit)?;
        let unit: &'a dyn NormalUnitTrait<D> = if let Some(index) = self.unload_index {
            unit.get_boarded().get(index as usize).ok_or(CommandError::MissingBoardedUnit)?.as_trait()
        } else {
            unit.as_normal_trait().ok_or(CommandError::UnitTypeWrong)?
        };
        Ok(unit)
    }
}

pub enum UnitCommand<D: Direction> {
    MoveAttack(CommonMovement, AttackInfo<D>),
    MovePull(CommonMovement, D),
    MoveCapture(CommonMovement),
    MoveWait(CommonMovement),
    MoveAboard(CommonMovement),
    MoveChess(Point, ChessCommand<D>),
    MercenaryPowerSimple(Point),
}
impl<D: Direction> UnitCommand<D> {
    pub fn convert(self, handler: &mut EventHandler<D>) -> Result<(), CommandError> {
        let team = handler.get_game().current_player().team;
        match self {
            Self::MoveAttack(cm, target) => {
                let intended_end = cm.intended_end();
                let path = cm.check_with_wait(handler.get_game())?;
                let unit = cm.get_unit(handler.get_map())?;
                match &target {
                    AttackInfo::Point(target) => {
                        if !handler.get_map().is_point_valid(target) {
                            return Err(CommandError::InvalidPoint(target.clone()));
                        }
                        match unit.get_attack_type() {
                            AttackType::Straight(_, _) => return Err(CommandError::InvalidTarget),
                            _ => {}
                        }
                        if !handler.get_game().has_vision_at(Some(team), target) {
                            return Err(CommandError::NoVision);
                        }
                        if !unit.attackable_positions(handler.get_map(), &intended_end, path.len() > 0).contains(target) {
                            return Err(CommandError::InvalidTarget);
                        }
                        let target_unit = handler.get_map().get_unit(target).ok_or(CommandError::MissingUnit)?;
                        if !unit.can_attack_unit_type(handler.get_game(), target_unit) {
                            return Err(CommandError::InvalidTarget);
                        }
                    }
                    AttackInfo::Direction(_) => {
                        match unit.get_attack_type() {
                            AttackType::Straight(_, _) => {},
                            _ => return Err(CommandError::InvalidTarget),
                        }
                    }
                }
                let end = path.last().unwrap_or(&cm.start).clone();
                apply_path(handler, cm.start, cm.unload_index, path);
                // checks fog trap
                if end == intended_end {
                    handle_attack(handler, &end, &target)?;
                }
                if handler.get_game().get_map().get_unit(&end).is_some() {
                    // ensured that the unit didn't die from counter attack
                    handler.add_event(Event::UnitExhaust(end));
                }
            }
            Self::MovePull(cm, dir) => {
                let intended_end = cm.intended_end();
                let path = cm.check_with_wait(handler.get_game())?;
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
                let mut blocked = false;
                let mut pull_path = vec![];
                let mut dp = OrientedPoint::new(intended_end.clone(), false, dir);
                for i in 0..max_dist {
                    if let Some(next_dp) = handler.get_map().get_neighbor(dp.point(), dp.direction()) {
                        dp = next_dp;
                        pull_path.insert(0, dp.point().clone());
                        if let Some(unit) = handler.get_map().get_unit(dp.point()) {
                            if handler.get_game().has_vision_at(Some(team), dp.point()) {
                                if i < min_dist - 1 || !unit.can_be_pulled(handler.get_map(), dp.point()) {
                                    // can't pull if the target is already next to the unit
                                    return Err(CommandError::InvalidTarget);
                                } else {
                                    // found a valid target, so no need to continue looping
                                    break;
                                }
                            } else {
                                // the pull is blocked by a unit that isn't visible to the player
                                blocked = true;
                            }
                        }
                    }
                }
                let end = path.last().unwrap_or(&cm.start).clone();
                apply_path(handler, cm.start, cm.unload_index, path);
                if intended_end == end && !blocked {
                    let pull_start = pull_path.remove(0);
                    apply_path(handler, pull_start, None, pull_path);
                }
                handler.add_event(Event::UnitExhaust(end));
            }
            Self::MoveCapture(cm) => {
                let intended_end = cm.intended_end();
                let path = cm.check_with_wait(handler.get_game())?;
                let unit = cm.get_unit(handler.get_map())?;
                if !unit.can_capture() {
                    return Err(CommandError::UnitCannotCapture);
                }
                let end = path.last().unwrap_or(&cm.start).clone();
                apply_path(handler, cm.start, cm.unload_index, path);
                if end == intended_end {
                    let terrain = handler.get_map().get_terrain(&end).unwrap().clone();
                    match &terrain {
                        Terrain::Realty(realty, owner) => {
                            if Some(team) != handler.get_game().get_team(owner.as_ref()) {
                                handler.add_event(Event::TerrainChange(end, terrain.clone(), Terrain::Realty(realty.clone(), Some(handler.get_game().current_player().owner_id))));
                            }
                        }
                        _ => {}
                    }
                }
                handler.add_event(Event::UnitExhaust(end));
            }
            Self::MoveWait(cm) => {
                let path = cm.check_with_wait(handler.get_game())?;
                let end = path.last().unwrap_or(&cm.start).clone();
                apply_path(handler, cm.start, cm.unload_index, path);
                handler.add_event(Event::UnitExhaust(end));
            }
            Self::MoveAboard(cm) => {
                let intended_end = cm.intended_end();
                let path = cm.check_without_wait(handler.get_game())?;
                if !handler.get_game().has_vision_at(Some(handler.get_game().current_player().team), &intended_end) {
                    return Err(CommandError::NoVision);
                }
                let end = path.last().unwrap_or(&cm.start).clone();
                if end == intended_end {
                    let unit = cm.get_unit(handler.get_map())?;
                    let transporter = handler.get_map().get_unit(&end).ok_or(CommandError::MissingUnit)?;
                    if !transporter.boardable_by(&unit.as_transportable()) {
                        return Err(CommandError::UnitCannotBeBoarded);
                    }
                    let load_index = transporter.get_boarded().len() as u8;
                    apply_path_with_event(handler, cm.start, cm.unload_index, path, |handler, unit, path| {
                        handler.add_event(Event::UnitPathInto(cm.unload_index, path, unit));
                    });
                    handler.add_event(Event::UnitExhaustBoarded(end, load_index));
                } else {
                    // stopped by fog, so the unit doesn't get aboard the transport
                    apply_path(handler, cm.start, cm.unload_index, path);
                    handler.add_event(Event::UnitExhaust(end));
                }
            }
            Self::MoveChess(start, chess_command) => {
                check_chess_unit_can_act(handler.get_game(), &start)?;
                match handler.get_map().get_unit(&start) {
                    Some(UnitType::Chess(unit)) => {
                        let unit = unit.clone();
                        chess_command.convert(start, &unit, handler)?;
                    },
                    _ => return Err(CommandError::UnitTypeWrong),
                }
            }
            Self::MercenaryPowerSimple(pos) => {
                if !handler.get_map().is_point_valid(&pos) {
                    return Err(CommandError::InvalidPoint(pos));
                }
                if !handler.get_game().has_vision_at(Some(handler.get_game().current_player().team), &pos) {
                    return Err(CommandError::NoVision);
                }
                match handler.get_map().get_unit(&pos) {
                    Some(UnitType::Mercenary(merc)) => {
                        if merc.can_use_simple_power(handler.get_game(), &pos) {
                            let change = -(merc.charge as i8);
                            handler.add_event(Event::MercenaryCharge(pos, change));
                            handler.add_event(Event::MercenaryPowerSimple(pos));
                        } else {
                            return Err(CommandError::PowerNotUsable);
                        }
                    },
                    _ => return Err(CommandError::UnitTypeWrong),
                }
            }
        }
        Ok(())
    }
}

pub fn on_path_details<D: Direction>(handler: &mut EventHandler<D>, path_taken: &Vec<Point>, unit: &UnitType<D>) {
    for p in path_taken {
        let old_details = handler.get_map().get_details(p);
        let details: Vec<Detail> = old_details.clone().into_iter().filter(|detail| {
            match detail {
                Detail::Coins1 => {
                    if let Some(owner) = unit.get_owner() {
                        if let Some(player) = handler.get_game().get_owning_player(owner) {
                            handler.add_event(Event::MoneyChange(*owner, player.income / 2));
                        }
                    }
                    false
                }
                Detail::Coins2 => {
                    if let Some(owner) = unit.get_owner() {
                        if let Some(player) = handler.get_game().get_owning_player(owner) {
                            handler.add_event(Event::MoneyChange(*owner, player.income));
                        }
                    }
                    false
                }
                Detail::Coins4 => {
                    if let Some(owner) = unit.get_owner() {
                        if let Some(player) = handler.get_game().get_owning_player(owner) {
                            handler.add_event(Event::MoneyChange(*owner, player.income * 2));
                        }
                    }
                    false
                }
                Detail::FactoryBubble(owner) => {
                    Some(owner) == unit.get_owner()
                }
            }
        }).collect();
        if details != old_details {
            handler.add_event(Event::ReplaceDetail(p.clone(), old_details, details));
        }
    }
}

fn apply_path_with_event<D: Direction, F: FnOnce(&mut EventHandler<D>, UnitType<D>, Vec<Option<Point>>)>(handler: &mut EventHandler<D>, start: Point, unload_index: Option<u8>, path_taken: Vec<Point>, f: F) {
    let mut unit = handler.get_map().get_unit(&start).unwrap().clone();
    if let Some(index) = unload_index {
        unit = unit.get_boarded()[index as usize].clone().as_unit();
    }
    if path_taken.len() > 0 {
        let mut event_path:Vec<Option<Point>> = path_taken.iter().map(|p| Some(p.clone())).collect();
        event_path.insert(0, Some(start.clone()));
        f(handler, unit.clone(), event_path);
        let team = handler.get_game().current_player().team;
        if Some(team) == unit.get_team(handler.get_game()) {
            let mut vision_changes = HashSet::new();
            for p in &path_taken {
                for p in unit.get_vision(handler.get_game(), p) {
                    if !handler.get_game().has_vision_at(Some(team), &p) {
                        vision_changes.insert(p);
                    }
                }
            }
            if vision_changes.len() > 0 {
                handler.add_event(Event::PureFogChange(Some(team), vision_changes));
            }
        }
    }
    on_path_details(handler, &path_taken, &unit);
}

fn apply_path<D: Direction>(handler: &mut EventHandler<D>, start: Point, unload_index: Option<u8>, path_taken: Vec<Point>) {
    apply_path_with_event(handler, start, unload_index, path_taken, |handler, unit, path| {
        handler.add_event(Event::UnitPath(unload_index, path, unit));
    })
}

pub fn calculate_attack<D: Direction>(handler: &mut EventHandler<D>, attacker_pos: &Point, target: &AttackInfo<D>, is_counter: bool) -> Result<Vec<Point>, CommandError> {
    let attacker = handler.get_map().get_unit(attacker_pos).and_then(|u| Some(u.clone()));
    let attacker: &dyn NormalUnitTrait<D> = match &attacker {
        Some(UnitType::Normal(unit)) => Ok(unit.as_trait()),
        Some(UnitType::Mercenary(unit)) => Ok(unit.as_trait()),
        Some(UnitType::Chess(_)) => Err(CommandError::UnitTypeWrong),
        Some(UnitType::Structure(_)) => Err(CommandError::UnitTypeWrong),
        None => Err(CommandError::MissingUnit),
    }?;
    let mut potential_counters = vec![];
    let mut recalculate_fog = false;
    let mut charges = HashMap::new();
    for target in attacker.attack_splash(handler.get_map(), attacker_pos, target)? {
        if let Some(defender) = handler.get_map().get_unit(&target) {
            let damage = defender.calculate_attack_damage(handler.get_game(), &target, attacker_pos, attacker, is_counter);
            if let Some(damage) = damage {
                let hp = defender.get_hp();
                if !is_counter && defender.get_owner() != Some(attacker.get_owner()) {
                    for (p, _) in handler.get_map().mercenary_influence_at(attacker_pos, Some(attacker.get_owner())) {
                        let change = if &p == attacker_pos {
                            3
                        } else {
                            1
                        };
                        charges.insert(p, charges.get(&p).unwrap_or(&0) + change);
                    }
                }
                handler.add_event(Event::UnitHpChange(target.clone(), -(damage.min(hp as u16) as i8), -(damage as i16)));
                if damage >= hp as u16 {
                    handler.add_event(Event::UnitDeath(target, handler.get_map().get_unit(&target).unwrap().clone()));
                    recalculate_fog = true;
                } else {
                    potential_counters.push(target);
                }
            }
        }
    }
    for (p, change) in charges {
        if let Some(UnitType::Mercenary(merc)) = handler.get_map().get_unit(&p) {
            let change = change.min(merc.typ.max_charge() as i16 - change).max(-(merc.charge as i16));
            if change != 0 {
                handler.add_event(Event::MercenaryCharge(p, change as i8));
            }
        }
    }
    if recalculate_fog {
        handler.recalculate_fog(true);
    }
    Ok(potential_counters)
}

pub fn handle_attack<D: Direction>(handler: &mut EventHandler<D>, attacker_pos: &Point, target: &AttackInfo<D>) -> Result<(), CommandError> {
    let potential_counters = calculate_attack(handler, attacker_pos, target, false)?;
    // counter attack
    for p in &potential_counters {
        let unit: &dyn NormalUnitTrait<D> = match handler.get_map().get_unit(p) {
            Some(UnitType::Normal(unit)) => unit.as_trait(),
            Some(UnitType::Mercenary(unit)) => unit.as_trait(),
            Some(UnitType::Chess(_)) => continue,
            Some(UnitType::Structure(_)) => continue,
            None => continue,
        };
        if !handler.get_game().has_vision_at(unit.get_team(handler.get_game()), attacker_pos) {
            continue;
        }
        if !unit.attackable_positions(handler.get_map(), &p, false).contains(attacker_pos) {
            continue;
        }
        // todo: if a straight attacker is counter-attacking another straight attacker, it should first try to reverse the direction
        let attack_info = unit.make_attack_info(handler.get_game(), p, attacker_pos).ok_or(CommandError::InvalidTarget)?;
        // this may return an error, but we don't care about that
        calculate_attack(handler, p, &attack_info, true).ok();
    }

    Ok(())
}
