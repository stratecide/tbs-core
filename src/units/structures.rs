use std::collections::HashMap;
use std::collections::HashSet;

use crate::game::event_handler::EventHandler;
use crate::game::game::Game;
use crate::map::direction::Direction;
use crate::map::map::Map;
use crate::map::point::Point;
use crate::map::wrapping_map::OrientedPoint;
use crate::player::Owner;

use super::ArmorType;
use super::Hp;
use super::UnitType;
use super::chess::get_diagonal_neighbor;
use super::commands::UnitAction;
use super::normal_units::*;

use interfaces::game_interface::ClientPerspective;
use zipper::*;
use zipper::zipper_derive::*;

pub const MEGA_CANNON_RANGE: usize = 5;
pub const MEGA_CANNON_DAMAGE: u16 = 60;

pub const LASER_CANNON_RANGE: u32 = 50;
pub const LASER_CANNON_DAMAGE: u16 = 40;

pub const SHOCK_TOWER_RANGE: usize = 4;
pub const SHOCK_TOWER_DAMAGE: u16 = 40;



#[derive(Debug, PartialEq, Clone, Zippable)]
pub struct Structure<D: Direction> {
    pub typ: Structures<D>,
    pub hp: Hp,
    pub exhausted: bool,
}

impl<D: Direction> Structure<D> {
    pub fn new_instance(from: Structures<D>) -> Self {
        Self {
            typ: from,
            hp: 100.into(),
            exhausted: false,
        }
    }

    pub fn fog_replacement(&self) -> Option<Self> {
        match &self.typ {
            Structures::DroneTower(Some((owner, _, drone_id))) => {
                Some(Self {
                    typ: Structures::DroneTower(Some((*owner, LVec::new(), *drone_id))),
                    hp: self.hp,
                    exhausted: self.exhausted,
                })
            }
            _ => Some(self.clone()),
        }
    }

    pub fn get_owner(&self) -> Option<Owner> {
        self.typ.get_owner()
    }
    
    pub fn attackable_positions(&self, game: &Game<D>, position: Point, _moved: bool) -> HashSet<Point> {
        let mut result = HashSet::new();
        match self.typ {
            Structures::Pyramid(_) |
            Structures::DroneTower(_) => (),
            Structures::MegaCannon(_, direction) => {
                attack_area_cannon(game.get_map(), position, direction, |p, _| {
                    result.insert(p);
                });
            }
            Structures::LaserCannon(_, direction) => {
                for p in attack_area_laser(game.get_map(), position, direction) {
                    result.insert(p);
                }
            }
            Structures::ShockTower(_) => {
                attack_area_shock_tower(game.get_map(), position, |p, _| {
                    result.insert(p);
                });
            }
        }
        result
    }

    pub fn start_turn(&self, handler: &mut EventHandler<D>, position: Point) {
        if Some(handler.get_game().current_player().owner_id) == self.typ.get_owner() {
            match self.typ {
                Structures::Pyramid(_) |
                Structures::DroneTower(_) => {
                    if self.exhausted {
                        handler.unit_unexhaust(position);
                    }
                }
                _ => {
                    if !self.exhausted {
                        /*let team = handler.get_game().current_player().team;
                        let (attack_area, effect) = self.typ.attack_area(handler.get_game(), position);
                        if let Some(effect) = effect {
                            handler.add_event(Event::Effect(effect));
                        }
                        for (pos, damage) in attack_area {
                            // turn into match if more unit types should be hit
                            if let Some(UnitType::Normal(unit)) = handler.get_map().get_unit(pos) {
                                if unit.get_team(handler.get_game()) != ClientPerspective::Team(*team as u8) {
                                    let hp = unit.get_hp();
                                    handler.unit_damage(pos, damage as u16);
                                    if hp <= damage as u8 {
                                        handler.unit_death(pos);
                                    }
                                }
                            }
                        }*/
                        self.fire(handler, position);
                        handler.unit_exhaust(position);
                    } else {
                        handler.unit_unexhaust(position);
                    }
                }
            }
        }
    }

    pub fn fire(&self, handler: &mut EventHandler<D>, position: Point) {
        let team = handler.get_game().current_player().team;
        match self.typ {
            Structures::Pyramid(_) |
            Structures::DroneTower(_) => (), // shouldn't happen
            Structures::MegaCannon(_, direction) => {
                // TODO: effect
                let mut layers = HashMap::new();
                attack_area_cannon(handler.get_map(), position, direction, |p, distance| {
                    if let Some(UnitType::Normal(unit)) = handler.get_map().get_unit(p) {
                        if unit.get_team(handler.get_game()) != ClientPerspective::Team(*team as u8) {
                            if !layers.contains_key(&distance) {
                                layers.insert(distance, HashMap::new());
                            }
                            let layer = layers.get_mut(&distance).unwrap();
                            layer.insert(p, layer.get(&p).cloned().unwrap_or(0) + MEGA_CANNON_DAMAGE - 5 * distance as u16);
                        }
                    }
                });
                let mut deaths = Vec::new();
                for distance in 0..MEGA_CANNON_RANGE * 2 {
                    if let Some(layer) = layers.remove(&distance) {
                        let mut new_deaths = HashSet::new();
                        for (p, damage) in &layer {
                            let hp = handler.get_map().get_unit(*p).unwrap().get_hp();
                            if hp > 0 && (hp as u16) <= *damage {
                                new_deaths.insert(*p);
                            }
                        }
                        handler.unit_mass_damage(layer);
                        deaths.push(new_deaths);
                    }
                }
                for death in deaths {
                    handler.unit_mass_death(death);
                }
            }
            Structures::LaserCannon(_, direction) => {
                let attack_area = attack_area_laser(handler.get_map(), position, direction);
                // TODO: effect
                //handler.effect_laser(position, direction, attack_area.len());
                let mut deaths = Vec::new();
                for pos in attack_area {
                    if let Some(UnitType::Normal(unit)) = handler.get_map().get_unit(pos) {
                        if unit.get_team(handler.get_game()) != ClientPerspective::Team(*team as u8) {
                            let hp = unit.get_hp();
                            handler.unit_damage(pos, LASER_CANNON_DAMAGE);
                            if hp > 0 && hp <= LASER_CANNON_DAMAGE as u8 {
                                deaths.push(pos);
                            }
                        }
                    }
                }
                // TODO: make them die in sequence or all at once?
                handler.unit_mass_death(deaths.into_iter().collect());
            }
            Structures::ShockTower(_) => {
                // TODO: effect
                let mut layers = HashMap::new();
                attack_area_shock_tower(handler.get_map(), position, |p, distance| {
                    if let Some(UnitType::Normal(unit)) = handler.get_map().get_unit(p) {
                        if unit.get_team(handler.get_game()) != ClientPerspective::Team(*team as u8) {
                            if !layers.contains_key(&distance) {
                                layers.insert(distance, HashMap::new());
                            }
                            let layer = layers.get_mut(&distance).unwrap();
                            layer.insert(p, layer.get(&p).cloned().unwrap_or(0) + SHOCK_TOWER_DAMAGE - 10 * distance as u16);
                        }
                    }
                });
                let mut deaths = Vec::new();
                for distance in 0..SHOCK_TOWER_RANGE {
                    if let Some(layer) = layers.remove(&distance) {
                        let mut new_deaths = HashSet::new();
                        for (p, damage) in &layer {
                            let hp = handler.get_map().get_unit(*p).unwrap().get_hp();
                            if hp > 0 && (hp as u16) <= *damage {
                                new_deaths.insert(*p);
                            }
                        }
                        handler.unit_mass_damage(layer);
                        deaths.push(new_deaths);
                    }
                }
                for death in deaths {
                    handler.unit_mass_death(death);
                }
            }
        }
    }

    pub fn available_options(&self, game: &Game<D>) -> Vec<UnitAction<D>> {
        let mut result = vec![];
        if self.exhausted {
            return result;
        }
        let player = if let Some(player) = self.typ.get_owner().and_then(|o| game.get_owning_player(o)) {
            player
        } else {
            return result;
        };
        match &self.typ {
            Structures::DroneTower(Some((_, drones, _))) => {
                if drones.remaining_capacity() > 0 {
                    for unit in NormalUnits::list() {
                        if let Some(drone) = TransportableDrones::from_normal(&unit) {
                            if unit.value() as i32 <= *player.funds {
                                result.push(UnitAction::BuildDrone(drone));
                            }
                        }
                    }
                }
            }
            _ => (),
        }
        result
    }

    pub fn can_act(&self, player: Owner) -> bool {
        match &self.typ {
            Structures::DroneTower(Some((owner, _, _))) => {
                !self.exhausted && *owner == player
            }
            _ => false,
        }
    }

    pub fn get_boarded(&self) -> Vec<NormalUnit> {
        match &self.typ {
            Structures::DroneTower(Some((owner, units, id))) => units.iter().map(|t| t.to_normal(*owner, Some(*id))).collect(),
            _ => vec![],
        }
    }

    pub fn get_boarded_mut(&mut self) -> Vec<&mut UnitData> {
        match &mut self.typ {
            Structures::DroneTower(Some((_, units, _))) => units.iter_mut().map(|u| &mut u.data).collect(),
            _ => vec![],
        }
    }

    pub fn unboard(&mut self, index: u8) {
        let index = index as usize;
        match &mut self.typ {
            Structures::DroneTower(Some((_, units, _))) => {
                units.remove(index).ok();
            }
            _ => (),
        };
    }

    pub fn board(&mut self, index: u8, unit: NormalUnit) {
        let index = index as usize;
        match &mut self.typ {
            Structures::DroneTower(Some((_, units, _))) => {
                TransportedUnit::from_normal(&unit)
                .and_then(|u| units.insert(index, u).ok());
            }
            _ => (),
        };
    }

}

#[derive(Debug, PartialEq, Clone, Zippable)]
#[zippable(bits = 4)]
pub enum Structures<D: Direction> {
    Pyramid(Option<Owner>),
    MegaCannon(Option<Owner>, D),
    LaserCannon(Option<Owner>, D),
    DroneTower(Option<(Owner, LVec<TransportedUnit<TransportableDrones>, 3>, DroneId)>),
    ShockTower(Option<Owner>),
}

impl<D: Direction> Structures<D> {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Pyramid(_) => "Pyramid",
            Self::MegaCannon(_, _) => "Mega Cannon",
            Self::LaserCannon(_, _) => "Laser Cannon",
            Self::DroneTower(_) => "Drone Tower",
            Self::ShockTower(_) => "Shock Tower",
        }
    }

    pub fn get_owner(&self) -> Option<Owner> {
        match self {
            Self::Pyramid(owner) => *owner,
            Self::MegaCannon(owner, _) => *owner,
            Self::LaserCannon(owner, _) => *owner,
            Self::DroneTower(Some((owner, _, _))) => Some(*owner),
            Self::DroneTower(None) => None,
            Self::ShockTower(owner) => *owner,
        }
    }

    pub fn get_armor(&self) -> (ArmorType, f32) {
        match self {
            Self::Pyramid(_) => (ArmorType::Heavy, 2.0),
            _ => (ArmorType::Heavy, 2.5),
        }
    }

    pub fn value(&self) -> u16 {
        match self {
            Self::Pyramid(_) => 0,
            Self::MegaCannon(_, _) => 500,
            Self::LaserCannon(_, _) => 500,
            Self::DroneTower(_) => 500,
            Self::ShockTower(_) => 500,
        }
    }

    pub fn transport_capacity(&self) -> u8 {
        // TODO: stupid
        match self {
            Self::DroneTower(_) => 3,
            _ => 0,
        }
    }

    pub fn could_transport(&self, unit: &NormalUnits) -> bool {
        // TODO: stupid?
        match self {
            Self::DroneTower(_) => {
                TransportableDrones::from_normal(unit).is_some()
            }
            _ => false
        }
    }
}

fn attack_area_cannon<D: Direction, F: FnMut(Point, usize)>(map: &Map<D>, position: Point, direction: D, mut callback: F) {
    if let Some(dp) = map.get_neighbor(position, direction) {
        if D::is_hex() {
            let mut old_front = HashMap::new();
            let mut front = HashMap::new();
            front.insert((dp.point, dp.direction), true);
            for i in 0..(MEGA_CANNON_RANGE * 2 - 1) {
                let older_front = old_front;
                old_front = front;
                front = HashMap::new();
                for ((position, direction), _) in older_front {
                    callback(position, i);
                    if let Some(dp) = map.get_neighbor(position, direction) {
                        front.insert((dp.point, dp.direction), true);
                    }
                }
                // in order to not spread too much, only spread if
                //      - previously moved straight forward
                //      - current position was spread to from both sides
                for ((position, direction), may_spread) in &old_front {
                    if *may_spread {
                        if let Some(dp) = map.get_neighbor(*position, direction.rotate(true)) {
                            let key = (dp.point, dp.direction.rotate(dp.mirrored));
                            front.insert(key, front.contains_key(&key));
                        }
                        if let Some(dp) = map.get_neighbor(*position, direction.rotate(false)) {
                            let key = (dp.point, dp.direction.rotate(!dp.mirrored));
                            front.insert(key, front.contains_key(&key));
                        }
                    }
                }
            }
            for (position, _) in old_front.keys() {
                callback(*position, MEGA_CANNON_RANGE * 2 - 1);
            }
            for (position, _) in front.keys() {
                callback(*position, MEGA_CANNON_RANGE * 2);
            }
        } else {
            let mut front = HashSet::new();
            front.insert((dp.point, dp.direction));
            for i in 0..MEGA_CANNON_RANGE {
                let old_front = front;
                front = HashSet::new();
                for (position, direction) in old_front {
                    callback(position, i * 2);
                    if let Some(dp) = map.get_neighbor(position, direction) {
                        front.insert((dp.point, dp.direction));
                    }
                    if let Some(dp) = get_diagonal_neighbor(map, position, direction) {
                        front.insert((dp.point, dp.direction));
                    }
                    if let Some(dp) = get_diagonal_neighbor(map, position, direction.rotate(true)) {
                        front.insert((dp.point, dp.direction.rotate(dp.mirrored)));
                    }
                }
            }
            for (position, _) in front {
                callback(position, MEGA_CANNON_RANGE * 2);
            }
        }
    }
}

// the same point may be contained multiple times
fn attack_area_laser<D: Direction>(map: &Map<D>, position: Point, direction: D) -> Vec<Point> {
    let mut result = Vec::new();
    let mut current = OrientedPoint::new(position, false, direction);
    for _ in 0..LASER_CANNON_RANGE {
        if let Some(dp) = map.get_neighbor(current.point, current.direction) {
            if let Some(UnitType::Structure(_)) = map.get_unit(dp.point) {
                break;
            }
            result.push(dp.point);
            current = dp;
        } else {
            break;
        }
    }
    result
}

fn attack_area_shock_tower<D: Direction, F: FnMut(Point, usize)>(map: &Map<D>, position: Point, mut callback: F) {
    for (i, layer) in map.range_in_layers(position, SHOCK_TOWER_RANGE).into_iter().enumerate() {
        if i > 0 {
            for p in layer {
                callback(p, i - 1);
            }
        }
    }
}

