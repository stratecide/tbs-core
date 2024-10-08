pub mod commander_type;
#[cfg(test)]
mod test;

use commander_type::CommanderType;
use zipper::*;

use crate::{config::environment::Environment, script::custom_action::CustomAction};

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
    pub fn can_gain_charge(&self) -> bool {
        self.environment.config.commander_can_gain_charge(self.typ, self.power)
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

    pub fn can_activate_power(&self, index: usize, automatic: bool) -> bool {
        let power = match self.environment.config.commander_powers(self.typ).get(index) {
            Some(power) => power,
            None => return false,
        };
        power.required_charge <= self.charge
        && if automatic {
            index == self.get_next_power()
        } else {
            power.usable_from_power.contains(&(self.power as u8))
        }
    }

    pub fn power_cost(&self, index: usize) -> u32 {
        let power = match self.environment.config.commander_powers(self.typ).get(index) {
            Some(power) => power,
            None => return 0,
        };
        power.required_charge
    }

    pub fn power_activation_script(&self, index: usize) -> Option<CustomAction> {
        self.environment.config.commander_powers(self.typ).get(index)
            .and_then(|config| config.script)
    }

}
