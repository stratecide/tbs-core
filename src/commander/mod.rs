pub mod commander_type;

use commander_type::CommanderType;
use num_rational::Rational32;

use crate::{config::Environment, script::{player::PlayerScript, unit::UnitScript, attack::AttackScript, kill::KillScript}, map::{direction::Direction, point::Point}, units::unit::Unit, game::game::Game};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Commander {
    typ: CommanderType,
    charge: u32,
    power: usize,
    environment: Environment,
}

impl Commander {
    pub fn new(environment: &Environment, typ: CommanderType) -> Self {
        Self {
            typ,
            charge: 0,
            power: 0,
            environment: environment.clone(),
        }
    }

    pub fn environment(&self) -> &Environment {
        &self.environment
    }

    pub fn typ(&self) -> CommanderType {
        self.typ
    }

    pub fn get_charge(&self) -> u32 {
        self.charge
    }
    pub fn get_max_charge(&self) -> u32 {
        self.environment.config.commander_max_charge(self.typ)
    }
    pub fn add_charge(&mut self, delta: i32) {
        self.charge = (self.charge as i32 + delta).max(0) as u32;
    }

    pub fn get_next_power(&self) -> usize {
        let power = match self.environment.config.commander_powers(self.typ).get(self.power) {
            Some(power) => power,
            None => return 0,
        };
        power.next_power as usize
    }

    pub fn get_active_power(&self) -> usize {
        self.power
    }
    pub fn set_active_power(&mut self, index: usize) {
        self.power = index;
    }
    pub fn can_activate_power(&self, index: usize) -> bool {
        if self.power == index {
            return false;
        }
        let power = match self.environment.config.commander_powers(self.typ).get(index) {
            Some(power) => power,
            None => return false,
        };
        power.usable_from_power.contains(&(self.power as u8))
        && power.required_charge <= self.charge
    }
    pub fn power_cost(&self, index: usize) -> u32 {
        let power = match self.environment.config.commander_powers(self.typ).get(index) {
            Some(power) => power,
            None => return 0,
        };
        power.required_charge
    }
    pub fn power_activation_effects(&self, index: usize) -> &[PlayerScript] {
        let power = match self.environment.config.commander_powers(self.typ).get(index) {
            Some(power) => power,
            None => return &[],
        };
        &power.effects
    }

    pub fn unit_death_effects<D: Direction>(&self, unit: &Unit<D>, game: &Game<D>, pos: Point) -> Vec<&UnitScript> {
        self.environment.config.commander_unit_death_effects(self, unit, game, pos)
    }

    pub fn unit_attack_scripts<D: Direction>(&self, unit: &Unit<D>, game: &Game<D>, pos: Point) -> Vec<&AttackScript> {
        self.environment.config.commander_unit_attack_effects(self, unit, game, pos)
    }

    pub fn unit_kill_scripts<D: Direction>(&self, unit: &Unit<D>, game: &Game<D>, pos: Point) -> Vec<&KillScript> {
        self.environment.config.commander_unit_kill_effects(self, unit, game, pos)
    }

    pub fn movement_bonus<D: Direction>(&self, unit: &Unit<D>, game: &Game<D>, pos: Point) -> Rational32 {
        self.environment.config.commander_movement_bonus(self, unit, game, pos)
    }

    pub fn attack_bonus<D: Direction>(&self, attacker: &Unit<D>, game: &Game<D>, pos: Point, is_counter: bool) -> Rational32 {
        self.environment.config.commander_attack_bonus(self, attacker, game, pos, is_counter)
    }

    pub fn defense_bonus<D: Direction>(&self, defender: &Unit<D>, game: &Game<D>, pos: Point, is_counter: bool) -> Rational32 {
        self.environment.config.commander_defense_bonus(self, defender, game, pos, is_counter)
    }
    
}
