
use crate::details::Detail;
use crate::game::events::*;
use crate::map::wrapping_map::{OrientedPoint};
use crate::player::*;
use crate::map::direction::Direction;
use crate::map::point::Point;
use crate::game::game::Game;
use crate::map::map::{NeighborMode, Map};
use crate::terrain::*;

use zipper::*;
use zipper::zipper_derive::*;

use super::*;

#[derive(Debug, PartialEq, Eq, Clone, Zippable, Hash)]
pub struct NormalUnit {
    pub typ: NormalUnits,
    pub mercenary: MaybeMercenary,
    pub owner: Owner,
    pub hp: Hp,
    pub exhausted: bool,
    pub zombie: bool,
}
impl NormalUnit {
    pub fn new_instance(from: NormalUnits, owner: Owner) -> NormalUnit {
        NormalUnit {
            typ: from,
            mercenary: MaybeMercenary::None,
            owner,
            hp: 100.try_into().unwrap(),
            exhausted: false,
            zombie: false,
        }
    }
    pub fn value<D: Direction>(&self, game: &Game<D>) -> u16 {
        self.typ.value() + self.mercenary.and_then(|m, _| m.price(game, self)).unwrap_or(0)
    }
    pub fn can_capture(&self) -> bool {
        if self.zombie {
            return false;
        }
        match self.typ {
            NormalUnits::Hovercraft(_) => true,
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
        *self.hp
    }
    pub fn get_armor(&self) -> (ArmorType, f32) {
        let (armor, defense) = self.typ.get_armor();
        (armor, defense + self.mercenary.own_defense_bonus())
    }
    pub fn get_weapons(&self) -> Vec<(WeaponType, f32)> {
        self.typ.get_weapons()
        .into_iter()
        .map(|(weapon, attack)| (weapon, attack + self.mercenary.own_attack_bonus()))
        .collect()
    }
    pub fn get_owner(&self) -> &Owner {
        &self.owner
    }
    pub fn get_team<D: Direction>(&self, game: &Game<D>) -> Option<Team> {
        game.get_team(Some(&self.owner))
    }
    pub fn can_act(&self, player: &Player) -> bool {
        !self.exhausted && player.owner_id == self.owner
    }
    pub fn get_movement<D: Direction>(&self, terrain: &Terrain<D>) -> (MovementType, u8) {
        let factor = 6;
        let (movement_type, movement) = match self.typ {
            NormalUnits::Sniper => (MovementType::Foot, 3 * factor),
            NormalUnits::Bazooka => (MovementType::Foot, 3 * factor),
            NormalUnits::DragonHead => (MovementType::Wheel, 6 * factor),
            NormalUnits::Artillery => (MovementType::Treads, 5 * factor),
            NormalUnits::SmallTank => (MovementType::Treads, 7 * factor),
            NormalUnits::BigTank => (MovementType::Treads, 6 * factor),
            NormalUnits::AntiAir => (MovementType::Treads, 7 * factor),
            NormalUnits::RocketLauncher => (MovementType::Wheel, 5 * factor),
            NormalUnits::Magnet => (MovementType::Wheel, 7 * factor),

            NormalUnits::Hovercraft(on_sea) => {
                let mut movement_type = MovementType::Hover(HoverMode::new(on_sea));
                if terrain.like_beach_for_hovercraft() {
                    movement_type = MovementType::Hover(HoverMode::Beach);
                }
                (movement_type, 3 * factor)
            },
            
            NormalUnits::SharkRider => (MovementType::Boat, 3 * factor),
            NormalUnits::TransportBoat(_) => (MovementType::Boat, 5 * factor),
            NormalUnits::WaveBreaker => (MovementType::Ship, 7 * factor),
            NormalUnits::Submarine => (MovementType::Ship, 7 * factor),
            NormalUnits::SiegeShip => (MovementType::Ship, 5 * factor),

            NormalUnits::TransportHeli(_) => (MovementType::Heli, 6 * factor),
            NormalUnits::AttackHeli => (MovementType::Heli, 7 * factor),
            NormalUnits::Blimp => (MovementType::Heli, 5 * factor),
            NormalUnits::Bomber => (MovementType::Plane, 8 * factor),
            NormalUnits::Fighter => (MovementType::Plane, 10 * factor),
        };
        (movement_type, movement + self.mercenary.own_movement_bonus())
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
        let player = game.get_owning_player(&self.owner).unwrap();
        if path.start == destination || game.get_map().get_unit(destination).is_none() {
            let mut funds_after_path = *player.funds;
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
            self.mercenary.add_options_after_path(self, game, path, funds_after_path, &mut result);
            for target in self.attackable_positions(game, destination, path.steps.len() > 0) {
                if let Some(attack_info) = self.make_attack_info(game, destination, target) {
                    if !self.can_pull() {
                        result.push(UnitAction::Attack(attack_info));
                    } else {
                        match self.make_attack_info(game, destination, target) {
                            Some(AttackInfo::Direction(d)) => {
                                result.push(UnitAction::Pull(d));
                            }
                            _ => {}
                        }
                    }
                }
            }
            match game.get_map().get_terrain(destination) {
                Some(Terrain::Realty(realty, owner)) => {
                    if self.can_capture() && Some(player.team) != owner.and_then(|o| game.get_owning_player(&o)).and_then(|p| Some(p.team)) {
                        result.push(UnitAction::Capture);
                    }
                    if owner == &Some(self.owner) && realty.can_repair(&self.typ) && funds_after_path * 100 >= self.typ.value() as i32 {
                        result.push(UnitAction::Repair);
                    }
                }
                _ => {}
            }
            result.push(UnitAction::Wait);
        } else if path.steps.len() > 0 {
            if let Some(transporter) = game.get_map().get_unit(destination) {
                // this is called indirectly by mercenaries, so using ::Normal could theoretically give wrong results
                if transporter.boardable_by(self) {
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
    pub fn check_path<D: Direction>(&self, game: &Game<D>, path_to_check: &Path<D>) -> Result<(), CommandError> {
        let team = self.get_team(game);
        let fog = game.get_fog().get(&team);
        let mut path_is_valid = false;
        movement_search(game, self, path_to_check, fog, |path, _p, can_stop_here| {
            if path == path_to_check {
                path_is_valid = can_stop_here;
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
    pub fn threatens<D: Direction>(&self, game: &Game<D>, target: &UnitType<D>) -> bool {
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
                    let mut current_pos = None;
                    for i in 0..max_range {
                        if let Some(dp) = game.get_map().get_neighbor(current_pos.and_then(|dp: OrientedPoint<D>| Some(dp.point)).unwrap_or(position), d) {
                            if i + 1 >= min_range {
                                result.insert(dp.point);
                            } else if game.get_map().get_unit(dp.point).is_some() {
                                break;
                            }
                            current_pos = Some(dp);
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
        if let Some(merc) = self.mercenary.and_then(|m, _| Some(m.build_option())) {
            mercs.insert(merc);
        }
    }
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
    TransportBoat(LVec::<NormalUnit, 2>),
    WaveBreaker,
    Submarine,
    SiegeShip,

    // air units
    TransportHeli(LVec::<NormalUnit, 1>),
    AttackHeli,
    Blimp,
    Bomber,
    Fighter,
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

            Self::TransportHeli(_) => "Transport Helicopter",
            Self::AttackHeli => "Attack Helicopter",
            Self::Blimp => "Blimp",
            Self::Bomber => "Bomber",
            Self::Fighter => "Fighter",
        }
    }
    pub fn get_boarded(&self) -> Vec<&NormalUnit> {
        match self {
            NormalUnits::TransportHeli(units) => units.iter().collect(),
            _ => vec![],
        }
    }
    pub fn get_boarded_mut(&mut self) -> Vec<&mut NormalUnit> {
        match self {
            NormalUnits::TransportHeli(units) => units.iter_mut().collect(),
            _ => vec![],
        }
    }
    pub fn transport_capacity(&self) -> u8 {
        match self {
            NormalUnits::TransportHeli(_) => 1,
            _ => 0,
        }
    }
    pub fn could_transport(&self, unit: &NormalUnits) -> bool {
        match self {
            NormalUnits::TransportHeli(_) => {
                match unit {
                    NormalUnits::Hovercraft(_) => true,
                    _ => false,
                }
            }
            _ => false
        }
    }
    pub fn unboard(&mut self, index: u8) {
        let units = match self {
            NormalUnits::TransportHeli(units) => units,
            _ => return,
        };
        if units.len() > index as usize {
            units.remove(index as usize);
        }
    }
    pub fn board(&mut self, index: u8, unit: NormalUnit) {
        let units = match self {
            NormalUnits::TransportHeli(units) => units,
            _ => return,
        };
        if units.len() >= index as usize {
            units.insert(index as usize, unit);
        }
    }
    pub fn get_attack_type(&self) -> AttackType {
        match self {
            Self::Sniper => AttackType::Ranged(1, 2),
            Self::Bazooka => AttackType::Adjacent,
            Self::DragonHead => AttackType::Straight(1, 2),
            Self::Artillery => AttackType::Ranged(2, 4),
            Self::SmallTank => AttackType::Adjacent,
            Self::BigTank => AttackType::Adjacent,
            Self::AntiAir => AttackType::Adjacent,
            Self::RocketLauncher => AttackType::Ranged(3, 6),
            Self::Magnet => AttackType::Straight(2, 2),

            Self::Hovercraft(_) => AttackType::Adjacent,

            Self::SharkRider => AttackType::Adjacent,
            //Self::ChargeBoat => AttackType::Adjacent,
            Self::TransportBoat(_) => AttackType::None,
            Self::WaveBreaker => AttackType::Adjacent,
            Self::Submarine => AttackType::Adjacent,
            Self::SiegeShip => AttackType::Ranged(2, 4),

            Self::TransportHeli(_) => AttackType::None,
            Self::AttackHeli => AttackType::Adjacent,
            Self::Blimp => AttackType::Adjacent,
            Self::Bomber => AttackType::Adjacent,
            Self::Fighter => AttackType::Adjacent,
        }
    }
    pub fn get_weapons(&self) -> Vec<(WeaponType, f32)> {
        match self {
            Self::Sniper => vec![(WeaponType::Rifle, 1.)],
            Self::Bazooka => vec![(WeaponType::Shells, 1.)],
            Self::DragonHead => vec![(WeaponType::Flame, 1.)],
            Self::Artillery => vec![(WeaponType::SurfaceMissiles, 1.)],
            Self::SmallTank => vec![(WeaponType::Shells, 1.)],
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

            Self::TransportHeli(_) => vec![],
            Self::AttackHeli => vec![(WeaponType::Rocket, 1.)],
            Self::Blimp => vec![(WeaponType::Rifle, 1.)],
            Self::Bomber => vec![(WeaponType::Bombs, 1.)],
            Self::Fighter => vec![(WeaponType::AntiAir, 1.)],
        }
    }
    pub fn get_armor(&self) -> (ArmorType, f32) {
        match self {
            Self::Sniper => (ArmorType::Infantry, 1.2),
            Self::Bazooka => (ArmorType::Infantry, 1.6),
            Self::DragonHead => (ArmorType::Light, 1.5),
            Self::Artillery => (ArmorType::Light, 1.5),
            Self::SmallTank => (ArmorType::Light, 2.0),
            Self::BigTank => (ArmorType::Heavy, 1.5),
            Self::AntiAir => (ArmorType::Light, 1.5),
            Self::RocketLauncher => (ArmorType::Light, 1.2),
            Self::Magnet => (ArmorType::Light, 1.5),

            Self::Hovercraft(_) => (ArmorType::Infantry, 1.5),
            
            Self::SharkRider => (ArmorType::Infantry, 1.5),
            Self::TransportBoat(_) => (ArmorType::Boat, 1.0),
            Self::WaveBreaker => (ArmorType::Boat, 2.0),
            Self::Submarine => (ArmorType::Submarine, 2.0),
            Self::SiegeShip => (ArmorType::Ship, 1.5),
            
            Self::TransportHeli(_) => (ArmorType::Heli, 1.2),
            Self::AttackHeli => (ArmorType::Heli, 1.8),
            Self::Blimp => (ArmorType::Heli, 1.5),
            Self::Bomber => (ArmorType::Plane, 1.5),
            Self::Fighter => (ArmorType::Plane, 1.5),
        }
    }
    pub fn value(&self) -> u16 {
        match self {
            Self::Sniper => 150,
            Self::Bazooka => 250,
            Self::DragonHead => 400,
            Self::Artillery => 600,
            Self::SmallTank => 800,
            Self::BigTank => 1500,
            Self::AntiAir => 800,
            Self::RocketLauncher => 1500,
            Self::Magnet => 500,

            Self::Hovercraft(_) => 100,
            
            Self::SharkRider => 150,
            Self::TransportBoat(_) => 1000,
            Self::WaveBreaker => 800,
            Self::Submarine => 1000,
            Self::SiegeShip => 1400,

            Self::TransportHeli(_) => 500,
            Self::AttackHeli => 900,
            Self::Blimp => 1200,
            Self::Bomber => 1800,
            Self::Fighter => 1600,
        }
    }
}

pub fn check_normal_unit_can_act<D: Direction>(game: &Game<D>, at: Point, unload_index: Option<UnloadIndex>) -> Result<(), CommandError> {
    if !game.has_vision_at(Some(game.current_player().team), at) {
        return Err(CommandError::NoVision);
    }
    let unit = game.get_map().get_unit(at).ok_or(CommandError::MissingUnit)?;
    let unit: &NormalUnit = if let Some(index) = unload_index {
        unit.get_boarded().get(*index as usize).ok_or(CommandError::MissingBoardedUnit)?
    } else {
        match unit {
            UnitType::Normal(unit) => unit,
            _ => return Err(CommandError::UnitTypeWrong),
        }
    };
    if &game.current_player().owner_id != unit.get_owner() {
        return Err(CommandError::NotYourUnit);
    }
    if !unit.can_act(game.current_player()) {
        return Err(CommandError::UnitCannotMove);
    }
    Ok(())
}
