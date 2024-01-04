use std::collections::HashSet;
use std::fmt::Debug;

use num_rational::Rational32;
use serde::Deserialize;
use zipper::*;

use crate::config::Environment;
use crate::game::fog::FogIntensity;
use crate::map::direction::Direction;
use crate::game::game::Game;
use crate::map::map::Map;
use crate::map::point::Point;
use super::attributes::*;
use super::commands::UnitAction;
use super::movement::Path;
use super::unit::Unit;
use super::unit_types::UnitType;


crate::listable_enum! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize)]
    pub enum HeroType {
        None,
        EarlGrey,
    }
}

impl HeroType {
    pub fn max_charge(&self, environment: &Environment) -> u8 {
        environment.config.hero_charge(*self)
    }

    pub fn transport_capacity(&self, environment: &Environment) -> usize {
        environment.config.hero_transport_capacity(*self) as usize
    }

    pub fn price<D: Direction>(&self, environment: &Environment, unit: &Unit<D>) -> Option<i32> {
        environment.config.hero_price(*self, unit.typ())
    }
}

// TODO: implement Display and Debug
#[derive(Clone, PartialEq, Eq)]
pub struct Hero {
    environment: Environment,
    typ: HeroType,
    power: bool,
    charge: u8,
    origin: Option<Point>,
}
attribute!(Hero, Hero);

impl Debug for Hero {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Hero{{typ: {:?}", self.typ)?;
        write!(f, ", charge: {}", self.charge)?;
        write!(f, ", power: {}", self.power)?;
        write!(f, "}}")
    }
}

impl Hero {
    pub fn new(environment: &Environment, typ: HeroType, origin: Option<Point>) -> Self {
        Self {
            typ,
            power: false,
            charge: 0,
            origin,
            environment: environment.clone(),
        }
    }

    pub fn typ(&self) -> HeroType {
        self.typ
    }

    pub fn get_origin(&self) -> Option<Point> {
        self.origin
    }

    pub fn max_charge(&self) -> u8 {
        self.typ.max_charge(&self.environment)
    }
    pub fn get_charge(&self) -> u8 {
        self.charge
    }
    pub fn set_charge(&mut self, charge: u8) {
        self.charge = charge.min(self.typ.max_charge(&self.environment));
    }

    pub fn is_power_active(&self) -> bool {
        self.power
    }
    pub fn set_power_active(&mut self, active: bool) {
        self.power = active;
    }

    pub fn aura_range(&self) -> usize {
        let mut range = self.environment.config.hero_aura_range(self.typ);
        if self.is_power_active() || self.charge == self.max_charge() {
            range += 1;
        }
        range as usize
    }

    pub fn in_range<D: Direction>(&self, map: &Map<D>, position: Point, target: Point) -> bool {
        self.aura(map, position).contains(&target)
    }

    pub fn aura<D: Direction>(&self, map: &Map<D>, position: Point) -> HashSet<Point> {
        let mut result = HashSet::new();
        result.insert(position.clone());
        for layer in map.range_in_layers(position, self.aura_range()) {
            for p in layer {
                result.insert(p);
            }
        }
        result
    }

    pub fn aura_attack_bonus<D: Direction>(&self, attacker: &Unit<D>, is_counter: bool) -> Rational32 {
        Rational32::from_integer(1)
    }

    pub fn aura_defense_bonus<D: Direction>(&self, defender: &Unit<D>, is_counter: bool) -> Rational32 {
        Rational32::from_integer(1)
    }

    pub fn add_options_after_path<D: Direction>(&self, list: &mut Vec<UnitAction<D>>, unit: &Unit<D>, game: &Game<D>, path: &Path<D>, destination: Point, get_fog: impl Fn(Point) -> FogIntensity) {
        // TODO
    }

    pub fn export(&self, zipper: &mut Zipper) {
        let bits = bits_needed_for_max_value(HeroType::list().len() as u32 - 1);
        zipper.write_u32(HeroType::list().iter().position(|t| *t == self.typ).unwrap_or(0) as u32, bits);
        if self.typ == HeroType::None {
            return;
        }
        self.origin.export(zipper);
        zipper.write_bool(self.power);
        if !self.power && self.typ.max_charge(&self.environment) > 0 {
            let bits = bits_needed_for_max_value(self.typ.max_charge(&self.environment) as u32);
            zipper.write_u8(self.charge, bits);
        }
    }

    pub fn import(unzipper: &mut Unzipper, environment: &Environment) -> Result<Self, ZipperError> {
        let bits = bits_needed_for_max_value(HeroType::list().len() as u32 - 1);
        let typ = *HeroType::list().get(unzipper.read_u32(bits)? as usize).ok_or(ZipperError::EnumOutOfBounds("HeroType".to_string()))?;
        let origin = if typ != HeroType::None {
            Option::<Point>::import(unzipper)?
        } else {
            None
        };
        let mut result = Self::new(environment, typ, origin);
        if typ != HeroType::None {
            result.power = unzipper.read_bool()?;
            if !result.power && typ.max_charge(environment) > 0 {
                let bits = bits_needed_for_max_value(typ.max_charge(environment) as u32);
                result.charge = typ.max_charge(environment).min(unzipper.read_u8(bits)?);
            }
        }
        Ok(result)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EarlGrey {
    Charging(u8),
    Power,
}
