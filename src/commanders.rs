use std::fmt::Display;

use crate::details::{MAX_STACK_SIZE, Detail};
use crate::game::event_handler::EventHandler;
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
pub const MAX_CHARGE: u32 = CHARGE_UNIT as u32 * 12;
pub type Charge = U<{MAX_CHARGE as i32}>;

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

    pub fn after_attacking<D: Direction>(&self, handler: &mut EventHandler<D>, attacker_pos: Point, _attacker: &NormalUnit, defenders: Vec<(Point, UnitType<D>, u16)>, _is_counter: bool) {
        match self {
            Self::Vampire(_, _) => {
                if handler.get_game().is_foggy() {
                    let mut damage: f32 = 0.0;
                    for (_, _, d) in defenders {
                        damage += d as f32;
                    }
                    let lifesteal = (damage * 0.15 + 0.5).floor() as u8;
                    if lifesteal > 0 {
                        handler.unit_heal(attacker_pos, lifesteal);
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
                let details = handler.get_map().get_details(defender_pos);
                if details.len() < MAX_STACK_SIZE as usize && defender.get_team(handler.get_game()) != ClientPerspective::Team(*player.team as u8) {
                    let mut unit= match defender {
                        UnitType::Normal(unit) => unit.clone(),
                        _ => return,
                    };
                    while unit.get_boarded().len() > 0 {
                        unit.unboard(0);
                    }
                    handler.detail_add(defender_pos, Detail::Skull(owner, unit.typ));
                }
            }
            _ => {}
        }
    }

    pub fn max_charge(&self) -> Charge {
        match self {
            Self::None => 0,
            Self::Vampire(_, _) => CommanderPower::VampireBloodStorm.charge_cost(),
            Self::Zombie(_, _) => CommanderPower::ZombieResurrection.charge_cost(),
        }.into()
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
            handler.commander_power_end(owner);
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
    pub fn charge_cost(&self) -> u32 {
        (match self {
            Self::VampireBloodStorm => 5,
            Self::ZombieResurrection => 6,
        } * CHARGE_UNIT as u32)
    }
    
    pub fn is_simple(&self) -> bool {
        match self {
            _ => true,
        }
    }

    pub fn execute<D: Direction>(&self, handler: &mut EventHandler<D>, owner: Owner) {
        handler.commander_charge_sub(owner, self.charge_cost());
        handler.commander_power_start(owner);
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
                        if unit.get_owner() == Some(owner) {
                            handler.unit_heal(p, 10);
                        } else if unit.get_team(handler.get_game()) != ClientPerspective::Team(*team as u8) && unit.get_hp() > 1 {
                            // maybe don't affect units without owner if that ever exists?
                            handler.unit_damage(p, 10);
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
                                    handler.detail_remove(p, index.into());
                                    let mut unit = NormalUnit::new_instance(unit_type, owner);
                                    unit.data.zombie = true;
                                    unit.data.hp = 50.into();
                                    handler.unit_creation(p, UnitType::Normal(unit));
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
