pub mod commander_type;

use std::collections::HashSet;

use commander_type::CommanderType;
use num_rational::Rational32;
use zipper::*;

use crate::config::environment::Environment;
use crate::script::player::PlayerScript;
use crate::script::unit::UnitScript;
use crate::script::attack::AttackScript;
use crate::script::kill::KillScript;
use crate::map::direction::Direction;
use crate::map::point::Point;
use crate::units::attributes::AttributeOverride;
use crate::units::unit::Unit;
use crate::game::game::Game;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Commander {
    typ: CommanderType,
    charge: u32,
    power: usize,
    environment: Environment,
}

impl SupportedZippable<&Environment> for Commander {
    fn export(&self, zipper: &mut Zipper, support: &Environment) {
        self.typ.export(zipper, &support.config);
        zipper.write_u32(self.charge, bits_needed_for_max_value(support.config.max_commander_charge()));
        zipper.write_u8(self.power as u8, bits_needed_for_max_value(support.config.commander_powers(self.typ).len() as u32 - 1));
    }
    fn import(unzipper: &mut Unzipper, support: &Environment) -> Result<Self, ZipperError> {
        let typ = CommanderType::import(unzipper, &support.config)?;
        let charge = unzipper.read_u32(bits_needed_for_max_value(support.config.max_commander_charge()))?;
        let power = unzipper.read_u8(bits_needed_for_max_value(support.config.commander_powers(typ).len() as u32 - 1))? as usize;
        Ok(Self {
            typ,
            charge,
            power,
            environment: support.clone(),
        })
    }
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

    pub fn name(&self) -> &str {
        self.environment.config.commander_name(self.typ)
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

    pub fn power_count(&self) -> usize {
        self.environment.config.commander_powers(self.typ).len()
    }

    pub fn power_name(&self, index: usize) -> &str {
        self.environment.config.commander_powers(self.typ)
        .get(index).and_then(|p| Some(p.name.as_str()))
        .unwrap_or("")
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

    pub fn power_activation_effects(&self, index: usize) -> Vec<PlayerScript> {
        let power = match self.environment.config.commander_powers(self.typ).get(index) {
            Some(power) => power,
            None => return Vec::new(),
        };
        power.effects.clone()
    }

    /*pub fn unit_start_turn_scripts<D: Direction>(&self, unit: &Unit<D>, game: &Game<D>, pos: Point) -> Vec<UnitScript> {
        self.environment.config.commander_unit_start_turn_effects(self, unit, game, pos)
    }

    pub fn unit_end_turn_scripts<D: Direction>(&self, unit: &Unit<D>, game: &Game<D>, pos: Point) -> Vec<UnitScript> {
        self.environment.config.commander_unit_end_turn_effects(self, unit, game, pos)
    }

    pub fn unit_death_scripts<D: Direction>(&self, unit: &Unit<D>, game: &Game<D>, pos: Point) -> Vec<UnitScript> {
        self.environment.config.commander_unit_death_effects(self, unit, game, pos)
    }

    pub fn unit_attack_scripts<D: Direction>(&self, unit: &Unit<D>, game: &Game<D>, pos: Point, other_unit: &Unit<D>, other_pos: Point) -> Vec<AttackScript> {
        self.environment.config.commander_unit_attack_effects(self, unit, game, pos, other_unit, other_pos)
    }

    pub fn unit_kill_scripts<D: Direction>(&self, unit: &Unit<D>, game: &Game<D>, pos: Point, other_unit: &Unit<D>, other_pos: Point) -> Vec<KillScript> {
        self.environment.config.commander_unit_kill_effects(self, unit, game, pos, other_unit, other_pos)
    }

    pub fn movement_bonus<D: Direction>(&self, unit: &Unit<D>, game: &Game<D>, pos: Point) -> Rational32 {
        self.environment.config.commander_movement_bonus(self, unit, game, pos)
    }

    pub fn attack_bonus<D: Direction>(&self, attacker: &Unit<D>, game: &Game<D>, pos: Point, is_counter: bool, other_unit: &Unit<D>, other_pos: Point) -> Rational32 {
        self.environment.config.commander_attack_bonus(self, attacker, game, pos, is_counter, other_unit, other_pos)
    }

    pub fn defense_bonus<D: Direction>(&self, defender: &Unit<D>, game: &Game<D>, pos: Point, is_counter: bool, other_unit: &Unit<D>, other_pos: Point) -> Rational32 {
        self.environment.config.commander_defense_bonus(self, defender, game, pos, is_counter, other_unit, other_pos)
    }*/
    
}
