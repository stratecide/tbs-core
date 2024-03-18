use std::collections::{HashMap, HashSet};
use std::fmt::Debug;
use zipper::*;

mod test;

use crate::config::environment::Environment;
use crate::game::game_view::GameView;
use crate::map::direction::Direction;
use crate::map::point::Point;
use crate::script::custom_action::CustomActionTestResult;
use super::attributes::*;
use super::commands::UnitAction;
use super::movement::{Path, TBallast};
use super::unit::Unit;

crate::enum_with_custom! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum HeroType {
        None,
        EarlGrey,
        Crystal,
        CrystalObelisk,
        BlueBerry,
        Tess,
        Edwin,
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

// TODO: implement Display
/**
 * Hero purposefully doesn't have Environment.
 * This way, it's easier to create a dummy unit without access to Environment/Config
 */
#[derive(Clone, PartialEq, Eq)]
pub struct Hero {
    typ: HeroType,
    power: usize,
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
    pub fn new(typ: HeroType, origin: Option<Point>) -> Self {
        Self {
            typ,
            power: 0,
            charge: 0,
            origin,
        }
    }

    pub fn typ(&self) -> HeroType {
        self.typ
    }

    pub fn get_origin(&self) -> Option<Point> {
        self.origin
    }

    pub fn max_charge(&self, environment: &Environment) -> u8 {
        self.typ.max_charge(environment)
    }
    pub fn get_charge(&self) -> u8 {
        self.charge
    }
    pub fn set_charge(&mut self, environment: &Environment, charge: u8) {
        self.charge = charge.min(self.typ.max_charge(environment));
    }
    pub fn can_gain_charge(&self, environment: &Environment) -> bool {
        environment.config.hero_can_gain_charge(self.typ, self.power)
    }

    pub fn get_next_power(&self, environment: &Environment) -> usize {
        let power = match environment.config.hero_powers(self.typ).get(self.power) {
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

    pub fn can_activate_power(&self, environment: &Environment, index: usize) -> bool {
        if self.power == index {
            return false;
        }
        let power = match environment.config.hero_powers(self.typ).get(index) {
            Some(power) => power,
            None => return false,
        };
        power.usable_from_power.contains(&(self.power as u8))
        && power.required_charge <= self.charge
    }

    pub fn power_cost(&self, environment: &Environment, index: usize) -> u8 {
        let power = match environment.config.hero_powers(self.typ).get(index) {
            Some(power) => power,
            None => return 0,
        };
        power.required_charge
    }

    pub fn aura_range<D: Direction>(
        map: &impl GameView<D>,
        unit: &Unit<D>,
        unit_pos: Point,
        transporter: Option<(&Unit<D>, usize)>,
    ) -> Option<usize> {
        map.environment().config.hero_aura_range(map, unit, unit_pos, transporter).map(|r| r as usize)
    }

    pub fn in_range<D: Direction>(
        map: &impl GameView<D>,
        unit: &Unit<D>,
        unit_pos: Point,
        transporter: Option<(&Unit<D>, usize)>,
        target: Point,
    ) -> bool {
        Self::aura(map, unit, unit_pos, transporter).contains(&target)
    }

    pub fn aura<D: Direction>(
        map: &impl GameView<D>,
        unit: &Unit<D>,
        unit_pos: Point,
        transporter: Option<(&Unit<D>, usize)>,
    ) -> HashSet<Point> {
        let mut result = HashSet::new();
        let aura_range = match Self::aura_range(map, unit, unit_pos, transporter) {
            Some(aura_range) => aura_range,
            _ => return result
        };
        result.insert(unit_pos);
        for layer in map.range_in_layers(unit_pos, aura_range) {
            for p in layer {
                result.insert(p);
            }
        }
        result
    }

    pub fn hero_influence_at<D: Direction>(map: &impl GameView<D>, point: Point, only_owner_id: i8) -> Vec<(Unit<D>, Hero, Point, Option<usize>)> {
        let mut result = Vec::new();
        for p in map.all_points() {
            if let Some(unit) = map.get_unit(p) {
                if only_owner_id >= 0 && unit.get_owner_id() != only_owner_id {
                    continue;
                }
                if unit.is_hero() && Hero::in_range(map, unit, p, None, point) {
                    result.push((unit.clone(), unit.get_hero(), p, None));
                }
                for (i, u) in unit.get_transported().iter().enumerate() {
                    if unit.is_hero() && Hero::in_range(map, u, p, Some((unit, i)), point) {
                        result.push((unit.clone(), unit.get_hero(), p, Some(i)));
                    }
                }
            }
        }
        if let Some(mut additional) = map.additional_hero_influence_at(point, only_owner_id) {
            result.append(&mut additional);
        }
        result
    }

    pub fn map_influence<D: Direction>(map: &impl GameView<D>, only_owner_id: i8) -> HashMap<(Point, i8), Vec<(Unit<D>, Hero, Point, Option<usize>)>> {
        let mut heroes = Vec::new();
        for p in map.all_points() {
            if let Some(unit) = map.get_unit(p) {
                if only_owner_id >= 0 && unit.get_owner_id() != only_owner_id {
                    continue;
                }
                if unit.is_hero() {
                    heroes.push((unit.clone(), unit.get_hero(), p, None));
                }
                for (i, unit) in unit.get_transported().iter().enumerate() {
                    if unit.is_hero() {
                        heroes.push((unit.clone(), unit.get_hero(), p, Some(i)));
                    }
                }
            }
        }
        let mut hero_auras: HashMap<(Point, i8), Vec<(Unit<D>, Hero, Point, Option<usize>)>> = HashMap::new();
        for hero in heroes {
            let transporter = hero.3.map(|i| (map.get_unit(hero.2).unwrap(), i));
            for p in Hero::aura(map, &hero.0, hero.2, transporter) {
                let key = (p, hero.0.get_owner_id());
                if let Some(list) = hero_auras.get_mut(&key) {
                    list.push(hero.clone());
                } else {
                    hero_auras.insert(key, vec![hero.clone()]);
                }
            }
        }
        if let Some(additional) = map.additional_hero_influence_map(only_owner_id) {
            for (key, mut additional) in additional {
                if let Some(list) = hero_auras.get_mut(&key) {
                    list.append(&mut additional);
                } else {
                    hero_auras.insert(key, additional);
                }
            }
        }
        hero_auras
    }

    pub fn add_options_after_path<D: Direction>(
        &self,
        list: &mut Vec<UnitAction<D>>,
        unit: &Unit<D>,
        game: &impl GameView<D>,
        funds: i32,
        path: &Path<D>,
        destination: Point,
        transporter: Option<(&Unit<D>, usize)>,
        heroes: &[(Unit<D>, Hero, Point, Option<usize>)],
        ballast: &[TBallast<D>],
    ) {
        let data = Vec::new();
        for (i, power) in game.environment().config.hero_powers(self.typ).iter().enumerate() {
            if self.charge >= power.required_charge
            && power.usable_from_power.contains(&(self.power as u8))
            && power.script.next_condition(game, funds, unit, path, destination, transporter, heroes, ballast, &data) != CustomActionTestResult::Failure {
                list.push(UnitAction::HeroPower(i, Vec::new()));
            }
        }
    }
}

impl SupportedZippable<&Environment> for Hero {
    fn export(&self, zipper: &mut Zipper, environment: &Environment) {
        let bits = bits_needed_for_max_value(environment.config.hero_count() as u32 - 1);
        zipper.write_u32(environment.config.hero_types().iter().position(|t| *t == self.typ).unwrap_or(0) as u32, bits);
        if self.typ == HeroType::None {
            return;
        }
        self.origin.export(zipper, environment);
        zipper.write_u8(self.power as u8, bits_needed_for_max_value(environment.config.hero_powers(self.typ).len() as u32 - 1));
        if self.typ.max_charge(&environment) > 0 {
            let bits = bits_needed_for_max_value(self.typ.max_charge(&environment) as u32);
            zipper.write_u8(self.charge, bits);
        }
    }

    fn import(unzipper: &mut Unzipper, environment: &Environment) -> Result<Self, ZipperError> {
        let bits = bits_needed_for_max_value(environment.config.hero_count() as u32 - 1);
        let typ = *environment.config.hero_types().get(unzipper.read_u32(bits)? as usize).ok_or(ZipperError::EnumOutOfBounds("HeroType".to_string()))?;
        let origin = if typ != HeroType::None {
            Option::<Point>::import(unzipper, environment)?
        } else {
            None
        };
        let mut result = Self::new(typ, origin);
        if typ != HeroType::None {
            result.power = unzipper.read_u8(bits_needed_for_max_value(environment.config.hero_powers(typ).len() as u32 - 1))? as usize;
            if typ.max_charge(environment) > 0 {
                let bits = bits_needed_for_max_value(typ.max_charge(environment) as u32);
                result.charge = typ.max_charge(environment).min(unzipper.read_u8(bits)?);
            }
        }
        Ok(result)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HeroChargeChange(pub i8);

impl SupportedZippable<&Environment> for HeroChargeChange {
    fn export(&self, zipper: &mut Zipper, support: &Environment) {
        let max = support.config.max_hero_charge() as i8;
        zipper.write_u8((self.0 + max) as u8, bits_needed_for_max_value(max as u32 * 2));
    }
    fn import(unzipper: &mut Unzipper, support: &Environment) -> Result<Self, ZipperError> {
        let max = support.config.max_hero_charge() as i8;
        Ok(Self(unzipper.read_u8(bits_needed_for_max_value(max as u32 * 2))? as i8 - max))
    }
}

impl From<i8> for HeroChargeChange {
    fn from(value: i8) -> Self {
        Self(value)
    }
}
