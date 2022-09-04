
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
            NormalUnits::Hovercraft => true,
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
    fn as_unit(&self) -> UnitType<D> {
        UnitType::Normal(self.clone())
    }
    fn as_transportable(&self) -> TransportableTypes {
        TransportableTypes::Normal(self.clone())
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
    fn get_movement(&self) -> (MovementType, u8) {
        let factor = 6;
        match self.typ {
            NormalUnits::Hovercraft => (MovementType::Hover, 3 * factor),
            NormalUnits::TransportHeli(_) => (MovementType::Heli, 6 * factor),
            NormalUnits::DragonHead => (MovementType::Wheel, 6 * factor),
            NormalUnits::Artillery => (MovementType::Treads, 5 * factor),
            NormalUnits::Magnet => (MovementType::Wheel, 7 * factor),
        }
    }
    fn has_stealth(&self) -> bool {
        false
    }
    fn options_after_path(&self, game: &Game<D>, start: &Point, path: &Vec<Point>) -> Vec<UnitAction<D>> {
        let mut result = vec![];
        let destination = path.last().unwrap_or(start).clone();
        if path.len() == 0 || game.get_map().get_unit(&destination).is_none() {
            for target in self.attackable_positions(game.get_map(), &destination, path.len() > 0) {
                if let Some(attack_info) = self.make_attack_info(game, &destination, &target) {
                    if !self.can_pull() {
                        result.push(UnitAction::Attack(attack_info));
                    } else {
                        match self.make_attack_info(game, &destination, &target) {
                            Some(AttackInfo::Direction(d)) => {
                                result.push(UnitAction::Pull(d));
                            }
                            _ => {}
                        }
                    }
                }
            }
            if self.can_capture() {
                match game.get_map().get_terrain(&destination) {
                    Some(Terrain::Realty(_, owner)) => {
                        if Some(game.get_owning_player(&self.owner).unwrap().team) != owner.and_then(|o| game.get_owning_player(&o)).and_then(|p| Some(p.team)) {
                            result.push(UnitAction::Capture);
                        }
                    }
                    _ => {}
                }
            }
            result.push(UnitAction::Wait);
        } else if path.len() > 0 {
            if let Some(transporter) = game.get_map().get_unit(&destination) {
                // TODO: this is called indirectly by mercenaries, so using ::Normal isn't necessarily correct
                if transporter.boardable_by(&TransportableTypes::Normal(self.clone())) {
                    result.push(UnitAction::Enter);
                }
            }
        }
        result
    }
    fn can_attack_after_moving(&self) -> bool {
        match self.typ {
            NormalUnits::Hovercraft => true,
            NormalUnits::TransportHeli(_) => true,
            NormalUnits::DragonHead => true,
            NormalUnits::Artillery => false,
            NormalUnits::Magnet => true,
        }
    }
    fn get_attack_type(&self) -> AttackType {
        self.typ.get_attack_type()
    }
    // ignores fog
    fn can_attack_unit_type(&self, game: &Game<D>, target: &UnitType<D>) -> bool {
        let this: &dyn NormalUnitTrait<D> = self.as_trait();
        target.get_team(game) != self.get_team(game) && this.get_weapons().iter().any(|(weapon, _)| weapon.damage_factor(&target.get_armor().0).is_some())
    }
    fn attackable_positions(&self, map: &Map<D>, position: &Point, moved: bool) -> HashSet<Point> {
        let mut result = HashSet::new();
        let this: &dyn NormalUnitTrait<D> = self.as_trait();
        if moved && !this.can_attack_after_moving() {
            return result;
        }
        match self.typ.get_attack_type() {
            AttackType::None => {},
            AttackType::Adjacent => {
                for p in map.get_neighbors(position, NeighborMode::FollowPipes) {
                    result.insert(p.point().clone());
                }
            }
            AttackType::Straight(min_range, max_range) => {
                for d in D::list() {
                    let mut current_pos = None;
                    for i in 0..max_range {
                        if let Some(dp) = map.get_neighbor(&current_pos.and_then(|dp: OrientedPoint<D>| Some(dp.point().clone())).unwrap_or(*position), &d) {
                            if i + 1 >= min_range {
                                result.insert(dp.point().clone());
                            } else if map.get_unit(dp.point()).is_some() {
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
                let mut layers = map.range_in_layers(position, max_range as usize);
                for _ in min_range-1..max_range {
                    for (p, _, _) in layers.pop().unwrap() {
                        result.insert(p);
                    }
                }
            }
        }
        result
    }
    fn attack_splash(&self, map: &Map<D>, from: &Point, to: &AttackInfo<D>) -> Result<Vec<Point>, CommandError> {
        match (&self.typ, to) {
            (NormalUnits::DragonHead, AttackInfo::Direction(dir)) => {
                if let Some(dp) = map.get_neighbor(from, dir) {
                    let mut result = vec![dp.point().clone()];
                    if let Some(dp) = map.get_neighbor(dp.point(), dir) {
                        result.push(dp.point().clone());
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
    fn make_attack_info(&self, game: &Game<D>, pos: &Point, target: &Point) -> Option<AttackInfo<D>> {
        let unit = match game.get_map().get_unit(target) {
            None => return None,
            Some(unit) => unit,
        };
        if self.can_pull() {
            if !unit.can_be_pulled(game.get_map(), target) {
                return None;
            }
        } else {
            if !self.can_attack_unit_type(game, unit) {
                return None;
            }
        }
        match self.typ.get_attack_type() {
            AttackType::Straight(min, max) => {
                for d in D::list() {
                    let mut current = OrientedPoint::new(*pos, false, *d);
                    for i in 0..max {
                        if let Some(dp) = game.get_map().get_neighbor(current.point(), current.direction()) {
                            current = dp;
                            if i < min - 1 {
                                if game.get_map().get_unit(current.point()).is_some() {
                                    break;
                                }
                            } else if current.point() == target {
                                return Some(AttackInfo::Direction(*d));
                            }
                        } else {
                            break;
                        }
                    }
                }
                None
            }
            _ => Some(AttackInfo::Point(*target)),
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
    Hovercraft,
    TransportHeli(LVec::<TransportableTypes, 1>),
    DragonHead,
    Artillery,
    Magnet,
}
impl NormalUnits {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Hovercraft => "Hovercraft",
            Self::TransportHeli(_) => "Transport Helicopter",
            Self::DragonHead => "Dragon Head",
            Self::Artillery => "Artillery",
            Self::Magnet => "Magnet",
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
                    NormalUnits::Hovercraft => true,
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
            Self::Hovercraft => AttackType::Adjacent,
            Self::TransportHeli(_) => AttackType::None,
            Self::DragonHead => AttackType::Straight(1, 2),
            Self::Artillery => AttackType::Ranged(2, 3),
            Self::Magnet => AttackType::Straight(2, 2),
        }
    }
    pub fn get_weapons(&self) -> Vec<(WeaponType, f32)> {
        match self {
            Self::Hovercraft => vec![(WeaponType::MachineGun, 1.)],
            Self::TransportHeli(_) => vec![],
            Self::DragonHead => vec![(WeaponType::Flame, 1.)],
            Self::Artillery => vec![(WeaponType::SurfaceMissiles, 1.)],
            Self::Magnet => vec![],
        }
    }
    pub fn get_armor(&self) -> (ArmorType, f32) {
        match self {
            Self::Hovercraft => (ArmorType::Infantry, 1.5),
            Self::TransportHeli(_) => (ArmorType::Heli, 1.5),
            Self::DragonHead => (ArmorType::Light, 1.5),
            Self::Artillery => (ArmorType::Light, 1.5),
            Self::Magnet => (ArmorType::Light, 1.5),
        }
    }
    pub fn value(&self) -> u16 {
        match self {
            Self::Hovercraft => 100,
            Self::TransportHeli(_) => 500,
            Self::DragonHead => 400,
            Self::Artillery => 600,
            Self::Magnet => 500,
        }
    }
}
