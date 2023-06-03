
use crate::details::Detail;
use crate::game::events::*;
use crate::map::point_map::MAX_AREA;
use crate::map::wrapping_map::OrientedPoint;
use crate::player::*;
use crate::map::direction::Direction;
use crate::map::point::Point;
use crate::game::game::Game;
use crate::map::map::{NeighborMode, Map};
use crate::terrain::*;

use zipper::*;
use zipper::zipper_derive::*;

use super::*;

#[derive(Debug, PartialEq, Eq, Clone, Copy, Zippable, Hash)]
#[zippable(bits = 3)]
pub enum UnitActionStatus {
    Normal,
    Capturing,
    Repairing
}

#[derive(Debug, PartialEq, Eq, Clone, Zippable, Hash)]
pub struct UnitData {
    pub mercenary: MaybeMercenary,
    pub hp: Hp,
    pub exhausted: bool,
    pub zombie: bool,
}
impl UnitData {
    pub fn new() -> Self {
        UnitData {
            mercenary: MaybeMercenary::None,
            hp: Hp::new(100),
            exhausted: false,
            zombie: false,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Zippable, Hash)]
pub struct TransportedUnit<T: TransportableUnits> {
    pub typ: T,
    pub data: UnitData,
}
impl<T: TransportableUnits> TransportedUnit<T> {
    pub fn from_normal(unit: &NormalUnit) -> Option<Self> {
        T::from_normal(&unit.typ)
        .and_then(|typ| Some(Self {
            typ,
            data: unit.data.clone(),
        }))
    }
    pub fn to_normal(&self, owner: Owner, drone_id: Option<DroneId>) -> NormalUnit {
        NormalUnit {
            typ: self.typ.to_normal(drone_id),
            owner,
            data: self.data.clone(),
            action_status: UnitActionStatus::Normal,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Zippable, Hash)]
pub struct NormalUnit {
    pub typ: NormalUnits,
    pub owner: Owner,
    pub data: UnitData,
    pub action_status: UnitActionStatus,
}
impl NormalUnit {
    pub fn new_instance(from: NormalUnits, owner: Owner) -> Self {
        Self {
            typ: from,
            owner,
            data: UnitData {
                mercenary: MaybeMercenary::None,
                hp: 100.try_into().unwrap(),
                exhausted: false,
                zombie: false,
            },
            action_status: UnitActionStatus::Normal,
        }
    }
    pub fn value<D: Direction>(&self, game: &Game<D>) -> u16 {
        self.typ.value() + self.data.mercenary.and_then(|m, _| m.price(game, self)).unwrap_or(0)
    }

    pub fn can_capture(&self) -> bool {
        if self.data.zombie {
            return false;
        }
        match self.typ {
            NormalUnits::Sniper |
            NormalUnits::Hovercraft(_) |
            NormalUnits::Bazooka |
            NormalUnits::SharkRider => true,
            _ => false,
        }
    }

    pub fn can_pull(&self) -> bool {
        match self.typ {
            NormalUnits::Magnet => true,
            _ => false,
        }
    }
    pub fn as_unit<D: Direction>(&self) -> UnitType<D> {
        UnitType::Normal(self.clone())
    }
    pub fn get_type(&self) -> &NormalUnits {
        &self.typ
    }
    pub fn get_type_mut(&mut self) -> &mut NormalUnits {
        &mut self.typ
    }
    pub fn get_hp(&self) -> u8 {
        *self.data.hp
    }
    pub fn get_armor(&self) -> (ArmorType, f32) {
        let (armor, mut defense) = self.typ.get_armor();
        defense *= 1. + self.data.mercenary.own_defense_bonus();
        defense *= match self.action_status {
            UnitActionStatus::Normal => 1.,
            UnitActionStatus::Capturing => 0.75,
            UnitActionStatus::Repairing => 0.75,
        };
        (armor, defense)
    }

    pub fn get_weapons(&self) -> Vec<(WeaponType, f32)> {
        self.typ.get_weapons()
        .into_iter()
        .map(|(weapon, attack)| (weapon, attack + self.data.mercenary.own_attack_bonus()))
        .collect()
    }

    pub fn attack_factor_from_path<D: Direction>(&self, game: &Game<D>, path: &Path<D>) -> f32 {
        self.typ.attack_factor_from_path(game.get_map(), path)
    }

    pub fn get_owner(&self) -> Owner {
        self.owner
    }
    pub fn get_team<D: Direction>(&self, game: &Game<D>) -> ClientPerspective {
        game.get_team(Some(self.owner))
    }

    pub fn can_act(&self, player: Owner) -> bool {
        !self.data.exhausted && player == self.owner
    }

    pub fn get_boarded(&self) -> Vec<NormalUnit> {
        match &self.typ {
            NormalUnits::TransportHeli(units) => units.iter().map(|t| t.to_normal(self.owner, None)).collect(),
            NormalUnits::TransportBoat(units) => units.iter().map(|t| t.to_normal(self.owner, None)).collect(),
            NormalUnits::DroneBoat(units, id) => units.iter().map(|t| t.to_normal(self.owner, Some(*id))).collect(),
            _ => vec![],
        }
    }

    pub fn get_boarded_mut(&mut self) -> Vec<&mut UnitData> {
        match &mut self.typ {
            NormalUnits::TransportHeli(units) => units.iter_mut().map(|u| &mut u.data).collect(),
            NormalUnits::TransportBoat(units) => units.iter_mut().map(|u| &mut u.data).collect(),
            NormalUnits::DroneBoat(units, _) => units.iter_mut().map(|u| &mut u.data).collect(),
            _ => vec![],
        }
    }

    pub fn unboard(&mut self, index: u8) {
        let index = index as usize;
        match &mut self.typ {
            NormalUnits::TransportHeli(units) => {
                units.remove(index).ok();
            }
            NormalUnits::TransportBoat(units) => {
                units.remove(index).ok();
            }
            NormalUnits::DroneBoat(units, _) => {
                units.remove(index).ok();
            }
            _ => (),
        };
    }

    pub fn board(&mut self, index: u8, unit: NormalUnit) {
        let index = index as usize;
        match &mut self.typ {
            NormalUnits::TransportHeli(units) => {
                TransportedUnit::from_normal(&unit)
                .and_then(|u| units.insert(index, u).ok());
            }
            NormalUnits::TransportBoat(units) => {
                TransportedUnit::from_normal(&unit)
                .and_then(|u| units.insert(index, u).ok());
            }
            NormalUnits::DroneBoat(units, _) => {
                TransportedUnit::from_normal(&unit)
                .and_then(|u| units.insert(index, u).ok());
            }
            _ => (),
        };
    }

    pub fn get_movement<D: Direction>(&self, terrain: &Terrain<D>) -> (MovementType, MovementPoints) {
        let (movement_type, movement) = match self.typ {
            NormalUnits::Sniper => (MovementType::Foot,MovementPoints::from(3.)),
            NormalUnits::Bazooka => (MovementType::Foot,MovementPoints::from(3.)),
            NormalUnits::DragonHead => (MovementType::Wheel,MovementPoints::from(5.)),
            NormalUnits::Artillery => (MovementType::Treads,MovementPoints::from(5.)),
            NormalUnits::SmallTank => (MovementType::Treads,MovementPoints::from(5.)),
            NormalUnits::BigTank => (MovementType::Treads,MovementPoints::from(4.)),
            NormalUnits::AntiAir => (MovementType::Treads,MovementPoints::from(5.)),
            NormalUnits::RocketLauncher => (MovementType::Wheel,MovementPoints::from(5.)),
            NormalUnits::Magnet => (MovementType::Wheel,MovementPoints::from(7.)),

            NormalUnits::Hovercraft(on_sea) => {
                let mut movement_type = MovementType::Hover(HoverMode::new(on_sea));
                if terrain.like_beach_for_hovercraft() {
                    movement_type = MovementType::Hover(HoverMode::Beach);
                }
                (movement_type,MovementPoints::from(4.5))
            },
            
            NormalUnits::SharkRider => (MovementType::Boat,MovementPoints::from(3.)),
            NormalUnits::TransportBoat(_) => (MovementType::Boat,MovementPoints::from(5.)),
            NormalUnits::WaveBreaker => (MovementType::Ship,MovementPoints::from(7.)),
            NormalUnits::Submarine => (MovementType::Ship,MovementPoints::from(7.)),
            NormalUnits::SiegeShip => (MovementType::Ship,MovementPoints::from(5.)),
            NormalUnits::DroneBoat(_, _) => (MovementType::Boat,MovementPoints::from(4.)),

            NormalUnits::TransportHeli(_) => (MovementType::Heli,MovementPoints::from(6.)),
            NormalUnits::AttackHeli => (MovementType::Heli,MovementPoints::from(7.)),
            NormalUnits::Blimp => (MovementType::Heli,MovementPoints::from(5.)),
            NormalUnits::Bomber => (MovementType::Plane,MovementPoints::from(8.)),
            NormalUnits::Fighter => (MovementType::Plane,MovementPoints::from(10.)),
            
            NormalUnits::LightDrone(_) => (MovementType::Heli,MovementPoints::from(4.)),
            NormalUnits::HeavyDrone(_) => (MovementType::Heli,MovementPoints::from(2.)),
        };
        (movement_type, movement + self.data.mercenary.own_movement_bonus())
    }
    pub fn has_stealth(&self) -> bool {
        false
    }
    pub fn shortest_path_to<D: Direction>(&self, game: &Game<D>, path_so_far: &Path<D>, goal: Point) -> Option<Path<D>> {
        let mut result = None;
        movement_search(game, self, path_so_far, None, |path, p, _can_stop_here| {
            if goal == p {
                result = Some(path.clone());
                PathSearchFeedback::Found
            } else {
                PathSearchFeedback::Continue
            }
        });
        result
    }
    pub fn options_after_path<D: Direction>(&self, game: &Game<D>, path: &Path<D>) -> Vec<UnitAction<D>> {
        let mut result = vec![];
        let destination = if let Ok(p) = path.end(game.get_map()) {
            p
        } else {
            return result;
        };
        let player = game.get_owning_player(self.owner).unwrap();
        let mut funds_after_path = *player.funds;
        let mut this = self.clone();
        match self.get_movement(game.get_map().get_terrain(path.start).unwrap()).0 {
            MovementType::Hover(hover_mode) => {
                for step in &path.hover_steps(game.get_map(), hover_mode) {
                    step.update_normal_unit(&mut this);
                }
            }
            _ => (),
        }
        let path_points: HashSet<Point> = path.points(game.get_map()).unwrap().into_iter().collect();
        for p in path_points {
            for detail in game.get_map().get_details(p) {
                match detail {
                    Detail::Coins1 => funds_after_path += *player.income as i32 / 2,
                    Detail::Coins2 => funds_after_path += *player.income as i32,
                    Detail::Coins4 => funds_after_path += *player.income as i32 * 2,
                    _ => {}
                }
            }
        }
        if path.start == destination || game.get_map().get_unit(destination).is_none() {
            match &this.typ {
                NormalUnits::DroneBoat(drones, _) => {
                    if drones.remaining_capacity() > 0 {
                        for unit in NormalUnits::list() {
                            if let Some(drone) = TransportableDrones::from_normal(&unit) {
                                if unit.value() as i32 <= funds_after_path {
                                    result.push(UnitAction::BuildDrone(drone));
                                }
                            }
                        }
                    }
                }
                _ => (),
            }
            this.data.mercenary.add_options_after_path(&this, game, path, funds_after_path, &mut result);
            for target in this.attackable_positions(game, destination, path.steps.len() > 0) {
                if let Some(attack_info) = this.make_attack_info(game, destination, target) {
                    if !this.can_pull() {
                        result.push(UnitAction::Attack(attack_info));
                    } else {
                        match this.make_attack_info(game, destination, target) {
                            Some(AttackInfo::Direction(d)) => {
                                result.push(UnitAction::Pull(d));
                            }
                            _ => {}
                        }
                    }
                }
            }
            match game.get_map().get_terrain(destination) {
                Some(Terrain::Realty(realty, owner, _)) => {
                    if this.can_capture() && Some(player.team) != owner.and_then(|o| game.get_owning_player(o)).and_then(|p| Some(p.team)) {
                        result.push(UnitAction::Capture);
                    }
                    if this.get_hp() < 100 && owner == &Some(this.owner) && realty.can_repair(&this.typ) && funds_after_path * 100 >= this.typ.value() as i32 {
                        result.push(UnitAction::Repair);
                    }
                }
                _ => {}
            }
            result.push(UnitAction::Wait);
        } else if path.steps.len() > 0 {
            if let Some(transporter) = game.get_map().get_unit(destination) {
                // this is called indirectly by mercenaries, so using ::Normal could theoretically give wrong results
                if transporter.boardable_by(&this) {
                    result.push(UnitAction::Enter);
                }
            }
        }
        result
    }
    pub fn can_attack_after_moving(&self) -> bool {
        match self.typ {
            NormalUnits::Artillery => false,
            NormalUnits::RocketLauncher => false,
            _ => true,
        }
    }
    pub fn shortest_path_to_attack<D: Direction>(&self, game: &Game<D>, path_so_far: &Path<D>, goal: Point) -> Option<Path<D>> {
        if !self.can_attack_after_moving() {
            // no need to look for paths if the unit can't attack after moving
            if path_so_far.steps.len() == 0 && self.attackable_positions(game, path_so_far.start, false).contains(&goal) {
                return Some(path_so_far.clone());
            } else {
                return None;
            }
        }
        let mut result = None;
        movement_search(game, self, path_so_far, None, |path, p, can_stop_here| {
            if can_stop_here && self.attackable_positions(game, p, path.steps.len() > 0).contains(&goal) {
                result = Some(path.clone());
                PathSearchFeedback::Found
            } else {
                PathSearchFeedback::Continue
            }
        });
        result
    }

    pub fn can_move_to<D: Direction>(&self, p: Point, game: &Game<D>) -> bool {
        // doesn't check terrain
        if let Some(unit) = game.get_map().get_unit(p) {
            if !unit.can_be_moved_through(self, game) {
                return false
            }
        }
        true
    }

    pub fn movable_positions<D: Direction>(&self, game: &Game<D>, path_so_far: &Path<D>) -> HashSet<Point> {
        let mut result = HashSet::new();
        movement_search(game, self, path_so_far, None, |_path, p, _can_stop_here| {
            result.insert(p);
            PathSearchFeedback::Continue
        });
        result
    }

    pub fn check_path<D: Direction>(&self, game: &Game<D>, path_to_check: &Path<D>, board_at_the_end: bool) -> Result<(), CommandError> {
        let team = self.get_team(game);
        let fog = game.get_fog().get(&team);
        let mut path_is_valid = false;
        movement_search(game, self, path_to_check, fog, |path, p, can_stop_here| {
            if path == path_to_check {
                if board_at_the_end {
                    if let Some(unit) = game.get_map().get_unit(p) {
                        path_is_valid = p != path_to_check.start && unit.boardable_by(self);
                    }
                } else {
                    path_is_valid = can_stop_here;
                }
            }
            // if path_to_check will be found at all, it would be the first one this callback gets called with
            PathSearchFeedback::Found
        });
        // TODO: make this method's return value a bool
        if path_is_valid {
            Ok(())
        } else {
            Err(CommandError::InvalidPath)
        }
    }
    pub fn get_attack_type(&self) -> AttackType {
        self.typ.get_attack_type()
    }
    // ignores fog
    pub fn can_attack_unit<D: Direction>(&self, game: &Game<D>, target: &UnitType<D>) -> bool {
        target.get_team(game) != self.get_team(game) && self.threatens(game, target)
    }
    pub fn threatens<D: Direction>(&self, _game: &Game<D>, target: &UnitType<D>) -> bool {
        self.get_weapons().iter().any(|(weapon, _)| weapon.damage_factor(&target.get_armor().0).is_some())
    }
    pub fn attackable_positions<D: Direction>(&self, game: &Game<D>, position: Point, moved: bool) -> HashSet<Point> {
        let mut result = HashSet::new();
        if moved && !self.can_attack_after_moving() {
            return result;
        }
        match self.typ.get_attack_type() {
            AttackType::None => {},
            AttackType::Adjacent => {
                for p in game.get_map().get_neighbors(position, NeighborMode::FollowPipes) {
                    result.insert(p.point);
                }
            }
            AttackType::Straight(min_range, max_range) => {
                for d in D::list() {
                    let mut current_pos = OrientedPoint::new(position, false, d);
                    for i in 0..max_range {
                        if let Some(dp) = game.get_map().get_neighbor(current_pos.point, current_pos.direction) {
                            if i + 1 >= min_range {
                                result.insert(dp.point);
                            } else if game.get_map().get_unit(dp.point).is_some() {
                                break;
                            }
                            current_pos = dp;
                        } else {
                            break;
                        }
                    }
                }
            }
            AttackType::Ranged(min_range, max_range) => {
                // each point in a layer is probably in it 2 times
                let mut layers = game.get_map().range_in_layers(position, max_range as usize);
                for _ in min_range-1..max_range {
                    for (p, _, _) in layers.pop().unwrap() {
                        result.insert(p);
                    }
                }
            }
        }
        result
    }
    pub fn attack_splash<D: Direction>(&self, map: &Map<D>, from: Point, to: &AttackInfo<D>) -> Result<Vec<Point>, CommandError> {
        match (&self.typ, to) {
            (NormalUnits::DragonHead, AttackInfo::Direction(dir)) => {
                if let Some(dp) = map.get_neighbor(from, *dir) {
                    let mut result = vec![dp.point];
                    if let Some(dp) = map.get_neighbor(dp.point, *dir) {
                        result.push(dp.point);
                    }
                    Ok(result)
                } else {
                    Err(CommandError::InvalidTarget)
                }
            }
            (NormalUnits::DragonHead, AttackInfo::Point(_)) => {
                Err(CommandError::InvalidTarget)
            }
            (_, AttackInfo::Point(p)) => Ok(vec![p.clone()]),
            _ => Err(CommandError::InvalidTarget),
        }
    }
    // returns Some(...) if the target position can be attacked from pos
    // returns None otherwise
    pub fn make_attack_info<D: Direction>(&self, game: &Game<D>, pos: Point, target: Point) -> Option<AttackInfo<D>> {
        let unit = match game.get_map().get_unit(target) {
            None => return None,
            Some(unit) => unit,
        };
        if self.can_pull() {
            if !unit.can_be_pulled(game.get_map(), target) {
                return None;
            }
        } else {
            if !self.can_attack_unit(game, unit) {
                return None;
            }
        }
        match self.typ.get_attack_type() {
            AttackType::Straight(min, max) => {
                for d in D::list() {
                    let mut current = OrientedPoint::new(pos, false, d);
                    for i in 0..max {
                        if let Some(dp) = game.get_map().get_neighbor(current.point, current.direction) {
                            current = dp;
                            if i < min - 1 {
                                if game.get_map().get_unit(current.point).is_some() {
                                    break;
                                }
                            } else if current.point == target {
                                return Some(AttackInfo::Direction(d));
                            }
                        } else {
                            break;
                        }
                    }
                }
                None
            }
            _ => Some(AttackInfo::Point(target)),
        }
    }
    pub fn update_used_mercs(&self, mercs: &mut HashSet<MercenaryOption>) {
        if let Some(merc) = self.data.mercenary.and_then(|m, _| Some(m.build_option())) {
            mercs.insert(merc);
        }
    }
}

pub trait TransportableUnits: std::fmt::Debug + PartialEq + Eq + Clone + Zippable + std::hash::Hash {
    fn from_normal(unit: &NormalUnits) -> Option<Self>;
    fn to_normal(&self, drone_id: Option<DroneId>) -> NormalUnits;
}

#[derive(Debug, PartialEq, Eq, Clone, Zippable, Hash)]
#[zippable(bits = 8)]
pub enum TransportableHeli {
    // ground units
    Sniper,
    Bazooka,
    DragonHead,
    Artillery,
    SmallTank,
    BigTank,
    AntiAir,
    RocketLauncher,
    Magnet,

    // hover units
    Hovercraft(bool), // bool is only relevant on e.g. bridges. true if HoverMode::Sea, false if HoverMode::Land

    // sea units
    SharkRider,
    TransportBoat, // can't contain units
    WaveBreaker,
    Submarine,
    SiegeShip,
    DroneBoat(DroneId), // can't contain drones, can't have drones flying around
}
impl TransportableUnits for TransportableHeli {
    fn from_normal(unit: &NormalUnits) -> Option<Self> {
        Some(match unit {
            NormalUnits::Sniper => Self::Sniper,
            NormalUnits::Bazooka => Self::Bazooka,
            NormalUnits::DragonHead => Self::DragonHead,
            NormalUnits::Artillery => Self::Artillery,
            NormalUnits::SmallTank => Self::SmallTank,
            NormalUnits::BigTank => Self::BigTank,
            NormalUnits::AntiAir => Self::AntiAir,
            NormalUnits::RocketLauncher => Self::RocketLauncher,
            NormalUnits::Magnet => Self::Magnet,

            NormalUnits::Hovercraft(on_sea) => Self::Hovercraft(*on_sea),

            NormalUnits::SharkRider => Self::SharkRider,
            NormalUnits::TransportBoat(units) => {
                if units.len() != 0 {
                    return None;
                }
                Self::TransportBoat
            },
            NormalUnits::WaveBreaker => Self::WaveBreaker,
            NormalUnits::Submarine => Self::Submarine,
            NormalUnits::SiegeShip => Self::SiegeShip,
            NormalUnits::DroneBoat(units, id) => {
                if units.len() != 0 {
                    return None;
                }
                Self::DroneBoat(*id)
            },
            _ => return None,
        })
    }
    fn to_normal(&self, _drone_id: Option<DroneId>) -> NormalUnits {
        match self {
            Self::Sniper => NormalUnits::Sniper,
            Self::Bazooka => NormalUnits::Bazooka,
            Self::DragonHead => NormalUnits::DragonHead,
            Self::Artillery => NormalUnits::Artillery,
            Self::SmallTank => NormalUnits::SmallTank,
            Self::BigTank => NormalUnits::BigTank,
            Self::AntiAir => NormalUnits::AntiAir,
            Self::RocketLauncher => NormalUnits::RocketLauncher,
            Self::Magnet => NormalUnits::Magnet,
            
            Self::Hovercraft(on_sea) => NormalUnits::Hovercraft(*on_sea),
            
            Self::SharkRider => NormalUnits::SharkRider,
            Self::TransportBoat => NormalUnits::TransportBoat(LVec::new()),
            Self::WaveBreaker => NormalUnits::WaveBreaker,
            Self::Submarine => NormalUnits::Submarine,
            Self::SiegeShip => NormalUnits::SiegeShip,
            Self::DroneBoat(id) => NormalUnits::DroneBoat(LVec::new(), *id), // TODO: don't forget to overwrite DroneId after unboarding!
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Zippable, Hash)]
#[zippable(bits = 8)]
pub enum TransportableBoat {
    // ground units
    Sniper,
    Bazooka,
    DragonHead,
    Artillery,
    SmallTank,
    BigTank,
    AntiAir,
    RocketLauncher,
    Magnet,

    // hover units
    Hovercraft(bool), // bool is only relevant on e.g. bridges. true if HoverMode::Sea, false if HoverMode::Land
}
impl TransportableUnits for TransportableBoat {
    fn from_normal(unit: &NormalUnits) -> Option<Self> {
        Some(match unit {
            NormalUnits::Sniper => Self::Sniper,
            NormalUnits::Bazooka => Self::Bazooka,
            NormalUnits::DragonHead => Self::DragonHead,
            NormalUnits::Artillery => Self::Artillery,
            NormalUnits::SmallTank => Self::SmallTank,
            NormalUnits::BigTank => Self::BigTank,
            NormalUnits::AntiAir => Self::AntiAir,
            NormalUnits::RocketLauncher => Self::RocketLauncher,
            NormalUnits::Magnet => Self::Magnet,
            NormalUnits::Hovercraft(on_sea) => Self::Hovercraft(*on_sea),
            _ => return None,
        })
    }
    fn to_normal(&self, _drone_id: Option<DroneId>) -> NormalUnits {
        match self {
            Self::Sniper => NormalUnits::Sniper,
            Self::Bazooka => NormalUnits::Bazooka,
            Self::DragonHead => NormalUnits::DragonHead,
            Self::Artillery => NormalUnits::Artillery,
            Self::SmallTank => NormalUnits::SmallTank,
            Self::BigTank => NormalUnits::BigTank,
            Self::AntiAir => NormalUnits::AntiAir,
            Self::RocketLauncher => NormalUnits::RocketLauncher,
            Self::Magnet => NormalUnits::Magnet,
            
            Self::Hovercraft(on_sea) => NormalUnits::Hovercraft(*on_sea),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Zippable, Hash)]
#[zippable(bits = 2)]
pub enum TransportableDrones {
    Light,
    Heavy,
}
impl TransportableUnits for TransportableDrones {
    fn from_normal(unit: &NormalUnits) -> Option<Self> {
        Some(match unit {
            NormalUnits::LightDrone(_) => Self::Light,
            NormalUnits::HeavyDrone(_) => Self::Heavy,
            _ => return None,
        })
    }
    fn to_normal(&self, drone_id: Option<DroneId>) -> NormalUnits {
        match self {
            // TODO: don't forget to replace DroneId after unboarding!
            Self::Light => NormalUnits::LightDrone(drone_id.expect("drones need to get a DroneId!")),
            Self::Heavy => NormalUnits::HeavyDrone(drone_id.expect("drones need to get a DroneId!")),
        }
    }
}

pub type DroneId = U16::<{MAX_AREA as u16 * 2}>;

pub fn buildable_drones<D: Direction>(_game: &Game<D>, _owner: Owner) -> Vec<TransportableDrones> {
    vec![
        TransportableDrones::Light,
        TransportableDrones::Heavy,
    ]
}

#[derive(Debug, PartialEq, Eq, Clone, Zippable, Hash)]
#[zippable(bits = 8)]
pub enum NormalUnits {
    // ground units
    Sniper,
    Bazooka,
    DragonHead,
    Artillery,
    SmallTank,
    BigTank,
    AntiAir,
    RocketLauncher,
    Magnet,

    // hover units
    Hovercraft(bool), // bool is only relevant on e.g. bridges. true if HoverMode::Sea, false if HoverMode::Land

    // sea units
    SharkRider,
    //ChargeBoat,
    TransportBoat(LVec::<TransportedUnit<TransportableBoat>, 2>),
    WaveBreaker,
    Submarine,
    SiegeShip,
    DroneBoat(LVec::<TransportedUnit<TransportableDrones>, 2>, DroneId),

    // air units
    TransportHeli(LVec::<TransportedUnit<TransportableHeli>, 1>),
    AttackHeli,
    Blimp,
    Bomber,
    Fighter,
    
    // drones
    LightDrone(DroneId),
    HeavyDrone(DroneId),
}
impl NormalUnits {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Sniper => "Sniper",
            Self::Bazooka => "Bazooka",
            Self::DragonHead => "Dragon Head",
            Self::Artillery => "Artillery",
            Self::SmallTank => "Small Tank",
            Self::BigTank => "Big Tank",
            Self::AntiAir => "Anti Air",
            Self::RocketLauncher => "Rocket Launcher",
            Self::Magnet => "Magnet",

            Self::Hovercraft(_) => "Hovercraft",
            
            Self::SharkRider => "Shark Rider",
            //Self::ChargeBoat => "Charge Boat",
            Self::TransportBoat(_) => "Transport Boat",
            Self::WaveBreaker => "Wavebreaker",
            Self::Submarine => "Submarine",
            Self::SiegeShip => "Siege Ship",
            Self::DroneBoat(_, _) => "Drone Boat",

            Self::TransportHeli(_) => "Transport Helicopter",
            Self::AttackHeli => "Attack Helicopter",
            Self::Blimp => "Blimp",
            Self::Bomber => "Bomber",
            Self::Fighter => "Fighter",
            
            Self::LightDrone(_) => "Light Drone",
            Self::HeavyDrone(_) => "Heavy Drone",
        }
    }

    pub fn list() -> Vec<Self> {
        vec![
            Self::Sniper,
            Self::Bazooka,
            Self::DragonHead,
            Self::Artillery,
            Self::SmallTank,
            Self::BigTank,
            Self::AntiAir,
            Self::RocketLauncher,
            Self::Magnet,

            // needs both versions here, since one can be built by port and the other by factory.
            Self::Hovercraft(true),
            Self::Hovercraft(false),

            Self::SharkRider,
            Self::TransportBoat(LVec::new()),
            Self::WaveBreaker,
            Self::Submarine,
            Self::SiegeShip,
            Self::DroneBoat(LVec::new(), DroneId::new(0)),

            Self::TransportHeli(LVec::new()),
            Self::AttackHeli,
            Self::Blimp,
            Self::Bomber,
            Self::Fighter,

            Self::LightDrone(DroneId::new(0)),
            Self::HeavyDrone(DroneId::new(0)),
        ]
    }

    pub fn repairable_factory(&self) -> bool {
        match self {
            Self::Sniper |
            Self::Bazooka |
            Self::DragonHead |
            Self::Artillery |
            Self::SmallTank |
            Self::BigTank |
            Self::AntiAir |
            Self::RocketLauncher |
            Self::Magnet => true,
            Self::Hovercraft(false) => true,
            _ => false,
        }
    }

    pub fn repairable_port(&self) -> bool {
        match self {
            Self::Hovercraft(true) => true,
            Self::SharkRider |
            Self::TransportBoat(_) |
            Self::WaveBreaker |
            Self::Submarine |
            Self::SiegeShip |
            Self::DroneBoat(_, _) => true,
            _ => false,
        }
    }

    pub fn repairable_airport(&self) -> bool {
        match self {
            Self::TransportHeli(_) |
            Self::AttackHeli |
            Self::Blimp |
            Self::Bomber |
            Self::Fighter => true,
            _ => false,
        }
    }

    /**
     * None if the unit can't capture
     * Some(false) if the unit can capture but isn't currently trying to
     * Some(true) if the unit is currently trying to capture
     */
    /*pub fn capture_status(&self) -> Option<bool> {
        match self {
            Self::Sniper(capturing) |
            Self::Bazooka(capturing) |
            Self::SharkRider(capturing) |
            Self::Hovercraft(_, capturing) => Some(*capturing),
            _ => None
        }
    }
    pub fn capture_status_mut(&mut self) -> Option<&mut bool> {
        match self {
            Self::Sniper(capturing) |
            Self::Bazooka(capturing) |
            Self::SharkRider(capturing) |
            Self::Hovercraft(_, capturing) => Some(capturing),
            _ => None
        }
    }*/

    pub fn transport_capacity(&self) -> u8 {
        // TODO: stupid
        match self {
            NormalUnits::TransportHeli(_) => 1,
            NormalUnits::TransportBoat(_) => 2,
            NormalUnits::DroneBoat(_, _) => 2,
            _ => 0,
        }
    }

    pub fn could_transport(&self, unit: &NormalUnits) -> bool {
        // TODO: stupid?
        match self {
            NormalUnits::TransportHeli(_) => {
                TransportableHeli::from_normal(unit).is_some()
            }
            NormalUnits::TransportBoat(_) => {
                TransportableBoat::from_normal(unit).is_some()
            }
            NormalUnits::DroneBoat(_, _) => {
                TransportableDrones::from_normal(unit).is_some()
            }
            _ => false
        }
    }

    pub fn get_attack_type(&self) -> AttackType {
        match self {
            Self::Sniper => AttackType::Ranged(1, 2),
            Self::Bazooka => AttackType::Adjacent,
            Self::DragonHead => AttackType::Straight(1, 2),
            Self::Artillery => AttackType::Ranged(2, 3),
            Self::SmallTank => AttackType::Adjacent,
            Self::BigTank => AttackType::Adjacent,
            Self::AntiAir => AttackType::Adjacent,
            Self::RocketLauncher => AttackType::Ranged(3, 5),
            Self::Magnet => AttackType::Straight(2, 2),

            Self::Hovercraft(_) => AttackType::Adjacent,

            Self::SharkRider => AttackType::Adjacent,
            //Self::ChargeBoat => AttackType::Adjacent,
            Self::TransportBoat(_) => AttackType::None,
            Self::WaveBreaker => AttackType::Adjacent,
            Self::Submarine => AttackType::Adjacent,
            Self::SiegeShip => AttackType::Ranged(2, 4),
            Self::DroneBoat(_, _) => AttackType::None,

            Self::TransportHeli(_) => AttackType::None,
            Self::AttackHeli => AttackType::Adjacent,
            Self::Blimp => AttackType::Adjacent,
            Self::Bomber => AttackType::Adjacent,
            Self::Fighter => AttackType::Adjacent,

            Self::LightDrone(_) => AttackType::Adjacent,
            Self::HeavyDrone(_) => AttackType::Adjacent,
        }
    }

    pub fn get_weapons(&self) -> Vec<(WeaponType, f32)> {
        match self {
            Self::Sniper => vec![(WeaponType::Rifle, 1.)],
            Self::Bazooka => vec![(WeaponType::Shells, 1.)],
            Self::DragonHead => vec![(WeaponType::Flame, 1.)],
            Self::Artillery => vec![(WeaponType::SurfaceMissiles, 1.)],
            Self::SmallTank => vec![(WeaponType::Shells, 1.2)],
            Self::BigTank => vec![(WeaponType::Shells, 1.8)],
            Self::AntiAir => vec![(WeaponType::AntiAir, 1.)],
            Self::RocketLauncher => vec![(WeaponType::SurfaceMissiles, 1.)],
            Self::Magnet => vec![],

            Self::Hovercraft(_) => vec![(WeaponType::MachineGun, 1.)],

            Self::SharkRider => vec![(WeaponType::Rifle, 1.)],
            Self::TransportBoat(_) => vec![],
            Self::WaveBreaker => vec![(WeaponType::Shells, 1.)],
            Self::Submarine => vec![(WeaponType::Torpedo, 1.)],
            Self::SiegeShip => vec![(WeaponType::SurfaceMissiles, 1.), (WeaponType::AntiAir, 0.5)],
            Self::DroneBoat(_, _) => vec![],

            Self::TransportHeli(_) => vec![],
            Self::AttackHeli => vec![(WeaponType::Rocket, 1.)],
            Self::Blimp => vec![(WeaponType::Rifle, 1.)],
            Self::Bomber => vec![(WeaponType::Bombs, 1.)],
            Self::Fighter => vec![(WeaponType::AntiAir, 1.)],

            Self::LightDrone(_) => vec![(WeaponType::MachineGun, 1.)],
            Self::HeavyDrone(_) => vec![(WeaponType::Shells, 1.)],
        }
    }

    pub fn get_armor(&self) -> (ArmorType, f32) {
        let (typ, mut multiplier) = match self {
            Self::Sniper => (ArmorType::Infantry, 1.2),
            Self::Bazooka => (ArmorType::Infantry, 1.6),
            Self::DragonHead => (ArmorType::Light, 1.5),
            Self::Artillery => (ArmorType::Light, 1.5),
            Self::SmallTank => (ArmorType::Light, 2.0),
            Self::BigTank => (ArmorType::Heavy, 1.5),
            Self::AntiAir => (ArmorType::Light, 1.5),
            Self::RocketLauncher => (ArmorType::Light, 1.2),
            Self::Magnet => (ArmorType::Light, 1.5),

            Self::Hovercraft(_) => (ArmorType::Infantry, 1.6),
            
            Self::SharkRider => (ArmorType::Infantry, 1.5),
            Self::TransportBoat(_) => (ArmorType::Boat, 1.0),
            Self::WaveBreaker => (ArmorType::Boat, 2.0),
            Self::Submarine => (ArmorType::Submarine, 2.0),
            Self::SiegeShip => (ArmorType::Ship, 1.5),
            Self::DroneBoat(_, _) => (ArmorType::Boat, 1.0),
            
            Self::TransportHeli(_) => (ArmorType::Heli, 1.2),
            Self::AttackHeli => (ArmorType::Heli, 1.8),
            Self::Blimp => (ArmorType::Heli, 1.5),
            Self::Bomber => (ArmorType::Plane, 1.5),
            Self::Fighter => (ArmorType::Plane, 1.5),
            
            Self::LightDrone(_) => (ArmorType::Heli, 0.8),
            Self::HeavyDrone(_) => (ArmorType::Heli, 0.8),
        };
        (typ, multiplier)
    }

    pub fn attack_factor_from_path<D: Direction>(&self, _map: &Map<D>, path: &Path<D>) -> f32 {
        match self {
            Self::Sniper => {
                if path.steps.len() > 0 {
                    0.5
                } else {
                    1.
                }
            }
            _ => 1.,
        }
    }

    pub fn value(&self) -> u16 {
        match self {
            Self::Sniper => 150,
            Self::Bazooka => 250,
            Self::DragonHead => 400,
            Self::Artillery => 600,
            Self::SmallTank => 700,
            Self::BigTank => 1500,
            Self::AntiAir => 700,
            Self::RocketLauncher => 1500,
            Self::Magnet => 500,

            Self::Hovercraft(_) => 100,
            
            Self::SharkRider => 150,
            Self::TransportBoat(_) => 1000,
            Self::WaveBreaker => 800,
            Self::Submarine => 1000,
            Self::SiegeShip => 1400,
            Self::DroneBoat(_, _) => 300,

            Self::TransportHeli(_) => 500,
            Self::AttackHeli => 900,
            Self::Blimp => 1200,
            Self::Bomber => 1800,
            Self::Fighter => 1600,
            
            Self::LightDrone(_) => 200,
            Self::HeavyDrone(_) => 400,
        }
    }
    pub fn insert_drone_ids(&self, existing_ids: &mut HashSet<u16>) {
        match self {
            Self::DroneBoat(_, id) |
            Self::LightDrone(id) |
            Self::HeavyDrone(id) => {
                existing_ids.insert(**id);
            }
            _ => (),
        }
    }
}

pub fn check_normal_unit_can_act<D: Direction>(game: &Game<D>, at: Point, unload_index: Option<UnloadIndex>) -> Result<(), CommandError> {
    if !game.has_vision_at(ClientPerspective::Team(*game.current_player().team), at) {
        return Err(CommandError::NoVision);
    }
    let unit = game.get_map().get_unit(at).ok_or(CommandError::MissingUnit)?;
    let boarded = unit.get_boarded();
    let unit: &NormalUnit = if let Some(index) = unload_index {
        boarded.get(*index as usize).ok_or(CommandError::MissingBoardedUnit)?
    } else {
        match unit {
            UnitType::Normal(unit) => unit,
            _ => return Err(CommandError::UnitTypeWrong),
        }
    };
    if game.current_player().owner_id != unit.get_owner() {
        return Err(CommandError::NotYourUnit);
    }
    if !unit.can_act(game.current_player().owner_id) {
        return Err(CommandError::UnitCannotMove);
    }
    Ok(())
}
