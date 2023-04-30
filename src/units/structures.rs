use std::collections::HashSet;

use crate::game::events::Effect;
use crate::game::events::Event;
use crate::game::events::EventHandler;
use crate::game::game::Game;
use crate::map::direction::Direction;
use crate::map::point::Point;
use crate::map::wrapping_map::OrientedPoint;
use crate::player::Owner;
use crate::player::Player;

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
pub const MEGA_CANNON_DAMAGE: i8 = 60;

pub const LASER_CANNON_RANGE: u32 = 50;
pub const LASER_CANNON_DAMAGE: i8 = 40;

pub const SHOCK_TOWER_RANGE: usize = 4;
pub const SHOCK_TOWER_DAMAGE: i8 = 70;



#[derive(Debug, PartialEq, Clone, Zippable)]
pub struct Structure<D: Direction> {
    pub typ: Structures::<D>,
    pub hp: Hp,
    pub exhausted: bool,
}

impl<D: Direction> Structure<D> {
    pub fn get_owner(&self) -> Option<Owner> {
        self.typ.get_owner()
    }
    
    pub fn attackable_positions(&self, game: &Game<D>, position: Point, _moved: bool) -> HashSet<Point> {
        self.typ.attack_area(game, position).0
            .into_iter()
            .map(|p| p.0)
            .collect()
    }

    pub fn start_turn(&self, handler: &mut EventHandler<D>, position: Point) {
        if Some(handler.get_game().current_player().owner_id) == self.typ.get_owner() {
            if !self.exhausted {
                let team = handler.get_game().current_player().team;
                let (attack_area, effect) = self.typ.attack_area(handler.get_game(), position);
                if let Some(effect) = effect {
                    handler.add_event(Event::Effect(effect));
                }
                for (pos, damage) in attack_area {
                    // turn into match if more unit types should be hit
                    if let Some(UnitType::Normal(unit)) = handler.get_map().get_unit(pos) {
                        if unit.get_team(handler.get_game()) != ClientPerspective::Team(*team) {
                            let hp = unit.get_hp() as i8;
                            handler.add_event(Event::UnitHpChange(pos, (-damage.min(hp)).try_into().unwrap(), (-damage as i16).try_into().unwrap()));
                            if hp <= damage {
                                handler.add_event(Event::UnitDeath(pos, handler.get_map().get_unit(pos).cloned().unwrap()));
                            }
                        }
                    }
                }
            }
            match self.typ {
                Structures::DroneTower(_) => {
                    if self.exhausted {
                        handler.add_event(Event::UnitExhaust(position));
                    }
                }
                _ => handler.add_event(Event::UnitExhaust(position)),
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
            Structures::DroneTower(Some((_, drones, drone_id))) => {
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

    pub fn can_act(&self, player: &Player) -> bool {
        match &self.typ {
            Structures::DroneTower(Some((owner, _, _))) => {
                !self.exhausted && *owner == player.owner_id
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
    MegaCannon(Option::<Owner>, D),
    LaserCannon(Option::<Owner>, D),
    DroneTower(Option::<(Owner, LVec::<TransportedUnit<TransportableDrones>, 3>, DroneId)>),
    ShockTower(Option::<Owner>),
}

impl<D: Direction> Structures<D> {
    pub fn name(&self) -> &'static str {
        match self {
            Self::MegaCannon(_, _) => "Mega Cannon",
            Self::LaserCannon(_, _) => "Laser Cannon",
            Self::DroneTower(_) => "Drone Tower",
            Self::ShockTower(_) => "Shock Tower",
        }
    }

    pub fn get_owner(&self) -> Option<Owner> {
        match self {
            Self::MegaCannon(owner, _) => *owner,
            Self::LaserCannon(owner, _) => *owner,
            Self::DroneTower(Some((owner, _, _))) => Some(*owner),
            Self::DroneTower(None) => None,
            Self::ShockTower(owner) => *owner,
        }
    }

    pub fn get_armor(&self) -> (ArmorType, f32) {
        match self {
            _ => (ArmorType::Heavy, 2.5),
        }
    }

    pub fn value(&self) -> u16 {
        match self {
            Self::MegaCannon(_, _) => 500,
            Self::LaserCannon(_, _) => 500,
            Self::DroneTower(_) => 500,
            Self::ShockTower(_) => 500,
        }
    }

    pub fn attack_area(&self, game: &Game<D>, position: Point) -> (Vec<(Point, i8)>, Option<Effect<D>>) {
        let mut result = Vec::new();
        let mut effect = None;
        match self {
            Self::MegaCannon(_, d) => {
                let mut layers = Vec::new();
                layers.push(HashSet::new());
                if let Some(dp) = game.get_map().get_neighbor(position, *d) {
                    layers[0].insert(dp);
                }
                let (range, damage_dropoff) = if D::is_hex() {
                    layers.push(HashSet::new());
                    layers.push(HashSet::new());
                    if let Some(dp) = get_diagonal_neighbor(game.get_map(), position, *d) {
                        layers[1].insert(dp);
                    }
                    if let Some(dp) = get_diagonal_neighbor(game.get_map(), position, d.rotate_clockwise()) {
                        layers[1].insert(OrientedPoint::new(dp.point, dp.mirrored, dp.direction.rotate_counter_clockwise()));
                    }
                    (MEGA_CANNON_RANGE * 2, 5)
                } else {
                    (MEGA_CANNON_RANGE, 10)
                };
                let mut i = 0;
                while layers.len() < range {
                    layers.push(HashSet::new());
                    for dp in layers[i].clone() {
                        if let Some(dp) = game.get_map().get_neighbor(dp.point, dp.direction) {
                            if D::is_hex() {
                                layers[i + 2].insert(dp);
                            } else {
                                layers[i + 1].insert(dp);
                            }
                        }
                        if D::is_hex() {
                            if let Some(dp) = get_diagonal_neighbor(game.get_map(), dp.point, dp.direction) {
                                layers[i + 3].insert(dp);
                            }
                            if let Some(dp) = get_diagonal_neighbor(game.get_map(), dp.point, dp.direction.rotate_clockwise()) {
                                layers[i + 3].insert(OrientedPoint::new(dp.point, dp.mirrored, dp.direction.rotate_counter_clockwise()));
                            }
                        } else {
                            if let Some(dp) = get_diagonal_neighbor(game.get_map(), dp.point, dp.direction) {
                                layers[i + 1].insert(dp);
                            }
                            if let Some(dp) = get_diagonal_neighbor(game.get_map(), dp.point, dp.direction.rotate_clockwise()) {
                                layers[i + 1].insert(OrientedPoint::new(dp.point, dp.mirrored, dp.direction.rotate_counter_clockwise()));
                            }
                        }
                    }
                    i += 1;
                }
                for (i, layer) in layers.into_iter().enumerate() {
                    for dp in layer {
                        // the same point may appear multiple times
                        result.push((dp.point, MEGA_CANNON_DAMAGE - i as i8 * damage_dropoff));
                    }
                }
                // TODO: effect
            }
            Self::LaserCannon(_, d) => {
                let mut current = OrientedPoint::new(position, false, *d);
                let mut laser_effect = vec![];
                for _ in 0..LASER_CANNON_RANGE {
                    if let Some(dp) = game.get_map().get_neighbor(current.point, current.direction) {
                        result.push((dp.point, LASER_CANNON_DAMAGE));
                        laser_effect.push((dp.point, dp.direction));
                        current = dp;
                    } else {
                        break;
                    }
                }
                if result.len() > 0 {
                    println!("added laser effect");
                    effect = Some(Effect::Laser(laser_effect.try_into().unwrap()));
                }
            }
            Self::DroneTower(_) => (),
            Self::ShockTower(_) => {
                let mut visited = HashSet::new();
                for (i, layer) in game.get_map().range_in_layers(position, SHOCK_TOWER_RANGE)
                .into_iter()
                .enumerate() {
                    if i == 0 {
                        continue;
                    }
                    for (p, _, _) in layer {
                        // don't hit the same point multiple times
                        if visited.insert(p) {
                            result.push((p, SHOCK_TOWER_DAMAGE - 10 * i as i8));
                        }
                    }
                }
                if visited.len() > 0 {
                    let visited: Vec<Point> = visited.into_iter().collect();
                    effect = Some(Effect::Lightning(visited.try_into().unwrap()));
                }
            }
        }
        (result, effect)
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

