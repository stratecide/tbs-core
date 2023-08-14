use std::fmt::Display;

use crate::details::{MAX_STACK_SIZE, Detail};
use crate::game::events::{EventHandler, Event};
use crate::game::game::*;
use crate::map::direction::*;
use crate::map::point::Point;
use crate::player::Owner;
use crate::units::*;
use crate::units::movement::{MovementPoints, MovementType};
use crate::units::normal_units::NormalUnit;

use interfaces::game_interface::ClientPerspective;
use zipper::U;
use zipper::zipper_derive::*;

pub const DEFAULT_ATTACK_BONUS_POWER: f32 = 0.1;
pub const DEFAULT_DEFENSE_BONUS_POWER: f32 = 0.1;

pub const CHARGE_UNIT: i32 = 100;
pub const MAX_CHARGE: i32 = CHARGE_UNIT * 12;
pub type Charge = U<{MAX_CHARGE}>;

#[derive(Debug, Clone, PartialEq, Zippable)]
#[zippable(bits = 8)]
pub enum Commander {
    None,
    Vampire(Charge, bool),
    Zombie(Charge, bool),
}

impl Commander {
    pub fn power_active(&self) -> bool {
        match self {
            Self::Vampire(_, power_active) => *power_active,
            Self::Zombie(_, power_active) => *power_active,
            Self::None => false,
        }
    }

    pub fn movement_bonus<D: Direction>(&self, _unit: &UnitType<D>) -> MovementPoints {
        MovementPoints::from(0.)
    }
    
    pub fn transform_movement_cost(&self, _unit: &NormalUnit, _movement_type: MovementType, cost: MovementPoints) -> MovementPoints {
        cost
    }
    
    pub fn attack_bonus<D: Direction>(&self, _game: &Game<D>, _attacker: &NormalUnit, _is_counter: bool) -> f32 {
        let mut result = match self {
            _ => 0.,
        };
        if self.power_active() {
            result += DEFAULT_ATTACK_BONUS_POWER;
        }
        result
    }

    pub fn defense_bonus<D: Direction>(&self, _game: &Game<D>, _defender: &UnitType<D>, _is_counter: bool) -> f32 {
        let mut result = match self {
            _ => 0.,
        };
        if self.power_active() {
            result += DEFAULT_ATTACK_BONUS_POWER;
        }
        result
    }
    
    pub fn after_attacked<D: Direction>(&self, _game: &Game<D>, _attacker: &NormalUnit, _defender: &UnitType<D>, _is_counter: bool) {
        match self {
            _ => {}
        }
    }

    pub fn after_attacking<D: Direction>(&self, handler: &mut EventHandler<D>, attacker_pos: Point, attacker: &NormalUnit, defenders: Vec<(Point, UnitType<D>, u16)>, _is_counter: bool) {
        match self {
            Self::Vampire(_, _) => {
                if handler.get_game().is_foggy() {
                    let mut damage: f32 = 0.0;
                    for (_, _, d) in defenders {
                        damage += d as f32;
                    }
                    let lifesteal = ((damage * 0.15 + 0.5).floor() as i8).min(100 - attacker.get_hp() as i8);
                    if lifesteal > 0 {
                        handler.add_event(Event::UnitHpChange(attacker_pos.clone(), lifesteal.into(), lifesteal.into()));
                    }
                }
            }
            _ => {}
        }
    }

    pub fn after_killing_unit<D: Direction>(&self, handler: &mut EventHandler<D>, owner: Owner, defender_pos: Point, defender: &UnitType<D>) {
        let player = handler.get_game().get_owning_player(owner).unwrap();
        match self {
            Self::Zombie(_, _) => {
                let mut details = handler.get_map().get_details(defender_pos);
                let old_details = details.clone();
                if details.len() < MAX_STACK_SIZE as usize && defender.get_team(handler.get_game()) != ClientPerspective::Team(*player.team as u8) {
                    let mut unit= match defender {
                        UnitType::Normal(unit) => unit.clone(),
                        _ => return,
                    };
                    while unit.get_boarded().len() > 0 {
                        unit.unboard(0);
                    }
                    details.push(Detail::Skull(owner, unit.typ));
                    handler.add_event(Event::ReplaceDetail(defender_pos.clone(), old_details.try_into().unwrap(), Detail::correct_stack(details).try_into().unwrap()));
                }
            }
            _ => {}
        }
    }

    pub fn max_charge(&self) -> Charge {
        match self {
            Self::None => 0.into(),
            Self::Vampire(_, _) => CommanderPower::VampireBloodStorm.charge_cost(),
            Self::Zombie(_, _) => CommanderPower::ZombieResurrection.charge_cost(),
        }
    }
    
    pub fn charge(&self) -> Charge {
        match self {
            Self::None => 0.into(),
            Self::Vampire(charge, _) => *charge,
            Self::Zombie(charge, _) => *charge,
        }
    }
    
    pub fn charge_potential(&self) -> Charge {
        if self.power_active() {
            return 0.into();
        }
        self.max_charge() - self.charge()
    }
    
    pub fn add_charge(&mut self, delta: i32) {
        match self {
            Self::None => {},
            Self::Vampire(charge, _) => *charge += delta,
            Self::Zombie(charge, _) => *charge += delta,
        }
    }
    
    pub fn powers(&self) -> Vec<CommanderPower> {
        match self {
            Self::None => vec![],
            Self::Vampire(_, _) => vec![CommanderPower::VampireBloodStorm],
            Self::Zombie(_, _) => vec![CommanderPower::ZombieResurrection],
        }
    }
    
    pub fn flip_active(&mut self) {
        match self {
            Self::None => {},
            Self::Vampire(_, active) => *active = !*active,
            Self::Zombie(_, active) => *active = !*active,
        }
    }
    
    pub fn start_turn<D: Direction>(&self, handler: &mut EventHandler<D>, owner: Owner) {
        if handler.get_game().current_player().owner_id == owner && self.power_active() {
            handler.add_event(Event::CommanderFlipActiveSimple(owner))
        }
    }
    
    pub fn list_all() -> Vec<Self> {
        vec![
            Self::None,
            Self::Vampire(0.into(), false),
            Self::Zombie(0.into(), false),
        ]
    }
}


#[derive(Debug, Clone, Copy, PartialEq, Zippable)]
#[zippable(bits = 8)]
pub enum CommanderPower {
    VampireBloodStorm,
    ZombieResurrection,
}
impl Display for CommanderPower {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::VampireBloodStorm => "Blood Storm",
            Self::ZombieResurrection => "Resurrection",
        })
    }
}
impl CommanderPower {
    pub fn charge_cost(&self) -> Charge {
        (match self {
            Self::VampireBloodStorm => 5,
            Self::ZombieResurrection => 6,
        } * CHARGE_UNIT).into()
    }
    
    pub fn is_simple(&self) -> bool {
        match self {
            _ => true,
        }
    }

    pub fn execute<D: Direction>(&self, handler: &mut EventHandler<D>, owner: Owner) {
        handler.add_event(Event::CommanderFlipActiveSimple(owner));
        handler.add_event(Event::CommanderCharge(handler.get_game().current_player().owner_id, (-*self.charge_cost()).into()));
        let player = handler.get_game().get_owning_player(owner).unwrap();
        match self {
            Self::VampireBloodStorm => {
                let team = player.team;
                for p in handler.get_map().all_points() {
                    if let Some(unit) = handler.get_map().get_unit(p) {
                        // structures aren't affected
                        match unit {
                            UnitType::Structure(_) => continue,
                            _ => {}
                        }
                        if unit.get_owner() == Some(owner) && unit.get_hp() < 100 {
                            let healing = 10.min(100 - unit.get_hp());
                            handler.add_event(Event::UnitHpChange(p, healing.into(), healing.into()));
                        } else if unit.get_team(handler.get_game()) != ClientPerspective::Team(*team as u8) && unit.get_hp() > 1 {
                            // maybe don't affect units without owner if that ever exists?
                            let damage = -(10.min(unit.get_hp() - 1) as i8);
                            handler.add_event(Event::UnitHpChange(p, damage.into(), damage.into()));
                        }
                    }
                }
            }
            Self::ZombieResurrection => {
                for p in handler.get_map().all_points() {
                    if handler.get_map().get_unit(p).is_some() {
                        continue;
                    }
                    for (index, detail) in handler.get_map().get_details(p).into_iter().enumerate() {
                        match detail {
                            Detail::Skull(o, unit_type) => {
                                if o == owner {
                                    handler.add_event(Event::RemoveDetail(p.clone(), index.into(), Detail::Skull(o, unit_type.clone())));
                                    let mut unit = NormalUnit::new_instance(unit_type, owner);
                                    unit.data.zombie = true;
                                    unit.data.hp = 50.into();
                                    handler.add_event(Event::UnitCreation(p, UnitType::Normal(unit)));
                                }
                                break;
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    }
}
