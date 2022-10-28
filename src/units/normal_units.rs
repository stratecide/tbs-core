
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

#[derive(Debug, PartialEq, Clone, Zippable)]
pub struct NormalUnit {
    pub typ: NormalUnits,
    pub owner: Owner,
    pub hp: Hp,
    pub exhausted: bool,
    pub zombie: bool,
}
impl NormalUnit {
    pub fn new_instance(from: NormalUnits, color_id: Owner) -> NormalUnit {
        NormalUnit {
            typ: from,
            owner: color_id,
            hp: 100.try_into().unwrap(),
            exhausted: false,
            zombie: false,
        }
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
}
impl<D: Direction> NormalUnitTrait<D> for NormalUnit {
    fn as_trait(&self) -> &dyn NormalUnitTrait<D> {
        self
    }
    fn as_trait_mut(&mut self) -> &mut dyn NormalUnitTrait<D> {
        self
    }
    fn as_unit(&self) -> UnitType<D> {
        UnitType::Normal(self.clone())
    }
    fn as_transportable(&self) -> TransportableTypes {
        TransportableTypes::Normal(self.clone())
    }
    fn get_type(&self) -> &NormalUnits {
        &self.typ
    }
    fn get_type_mut(&mut self) -> &mut NormalUnits {
        &mut self.typ
    }
    fn get_hp(&self) -> u8 {
        *self.hp
    }
    fn get_weapons(&self) -> Vec<(WeaponType, f32)> {
        self.typ.get_weapons()
    }
    fn get_owner(&self) -> &Owner {
        &self.owner
    }
    fn get_team(&self, game: &Game<D>) -> Option<Team> {
        game.get_team(Some(&self.owner))
    }
    fn can_act(&self, player: &Player) -> bool {
        !self.exhausted && player.owner_id == self.owner
    }
    fn get_movement(&self, terrain: &Terrain<D>) -> (MovementType, u8) {
        let factor = 6;
        match self.typ {
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
        }
    }
    fn has_stealth(&self) -> bool {
        false
    }
    fn options_after_path(&self, game: &Game<D>, path: &Path<D>) -> Vec<UnitAction<D>> {
        let mut result = vec![];
        let destination = if let Ok(p) = path.end(game.get_map()) {
            p
        } else {
            return result;
        };
        let player = game.get_owning_player(&self.owner).unwrap();
        if path.start == destination || game.get_map().get_unit(destination).is_none() {
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
            if self.can_capture() {
                match game.get_map().get_terrain(destination) {
                    Some(Terrain::Realty(_, owner)) => {
                        if Some(player.team) != owner.and_then(|o| game.get_owning_player(&o)).and_then(|p| Some(p.team)) {
                            result.push(UnitAction::Capture);
                        }
                    }
                    _ => {}
                }
            }
            if game.can_buy_merc_at(player, destination) {
                let mercs:Vec<MercenaryOption> = game.available_mercs(player)
                    .into_iter()
                    .filter(|m| m.price(game, &self).is_some())
                    .collect();
                if mercs.len() > 0 {
                    result.push(UnitAction::BuyMercenary(mercs));
                }
            }
            result.push(UnitAction::Wait);
        } else if path.steps.len() > 0 {
            if let Some(transporter) = game.get_map().get_unit(destination) {
                // this is called indirectly by mercenaries, so using ::Normal could theoretically give wrong results
                if transporter.boardable_by(&TransportableTypes::Normal(self.clone())) {
                    result.push(UnitAction::Enter);
                }
            }
        }
        result
    }
    fn can_attack_after_moving(&self) -> bool {
        match self.typ {
            NormalUnits::Artillery => false,
            NormalUnits::RocketLauncher => false,
            _ => true,
        }
    }
    fn get_attack_type(&self) -> AttackType {
        self.typ.get_attack_type()
    }
    // ignores fog
    fn can_attack_unit(&self, game: &Game<D>, target: &UnitType<D>) -> bool {
        target.get_team(game) != self.get_team(game) && self.threatens(game, target)
    }
    fn threatens(&self, game: &Game<D>, target: &UnitType<D>) -> bool {
        let this: &dyn NormalUnitTrait<D> = self.as_trait();
        this.get_weapons().iter().any(|(weapon, _)| weapon.damage_factor(&target.get_armor().0).is_some())
    }
    fn attackable_positions(&self, game: &Game<D>, position: Point, moved: bool) -> HashSet<Point> {
        let mut result = HashSet::new();
        let this: &dyn NormalUnitTrait<D> = self.as_trait();
        if moved && !this.can_attack_after_moving() {
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
    fn attack_splash(&self, map: &Map<D>, from: Point, to: &AttackInfo<D>) -> Result<Vec<Point>, CommandError> {
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
    fn make_attack_info(&self, game: &Game<D>, pos: Point, target: Point) -> Option<AttackInfo<D>> {
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
    fn can_capture(&self) -> bool {
        self.can_capture()
    }
    fn can_pull(&self) -> bool {
        self.can_pull()
    }
}

#[derive(Debug, PartialEq, Clone, Zippable)]
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
    TransportBoat(LVec::<TransportableTypes, 2>),
    WaveBreaker,
    Submarine,
    SiegeShip,

    // air units
    TransportHeli(LVec::<TransportableTypes, 1>),
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
    pub fn get_boarded(&self) -> Vec<&TransportableTypes> {
        match self {
            NormalUnits::TransportHeli(units) => units.iter().collect(),
            _ => vec![],
        }
    }
    pub fn get_boarded_mut(&mut self) -> Vec<&mut TransportableTypes> {
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
    pub fn board(&mut self, index: u8, unit: TransportableTypes) {
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
