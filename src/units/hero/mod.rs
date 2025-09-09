use rustc_hash::FxHashMap;
use std::fmt::Debug;
use zipper::*;

pub mod rhai_hero;
#[cfg(test)]
mod test;

use crate::config::environment::Environment;
use crate::config::parse::FromConfig;
use crate::game::event_handler::EventHandler;
use crate::map::board::{Board, BoardView};
use crate::map::direction::Direction;
use crate::map::map::{get_neighbors_layers, valid_points};
use crate::map::point::Point;
use super::commands::UnitAction;
use super::unit::Unit;
use super::UnitId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HeroType(pub usize);

impl FromConfig for HeroType {
    fn from_conf<'a>(s: &'a str, loader: &mut crate::config::file_loader::FileLoader) -> Result<(Self, &'a str), crate::config::ConfigParseError> {
        let (base, s) = crate::config::parse::string_base(s);
        match loader.hero_types.iter().position(|name| name.as_str() == base) {
            Some(i) => Ok((Self(i), s)),
            None => Err(crate::config::ConfigParseError::MissingHero(base.to_string()))
        }
    }
}

impl SupportedZippable<&Environment> for HeroType {
    fn export(&self, zipper: &mut Zipper, support: &Environment) {
        let index = support.config.hero_types().iter().position(|t| t == self).unwrap();
        let bits = bits_needed_for_max_value(support.config.hero_count() as u32 - 1);
        zipper.write_u32(index as u32, bits);
    }
    fn import(unzipper: &mut Unzipper, support: &Environment) -> Result<Self, ZipperError> {
        let bits = bits_needed_for_max_value(support.config.hero_count() as u32 - 1);
        let index = unzipper.read_u32(bits)? as usize;
        if index < support.config.hero_count() {
            Ok(support.config.hero_types()[index])
        } else {
            Err(ZipperError::EnumOutOfBounds(format!("HeroType index {}", index)))
        }
    }
}

impl HeroType {
    pub fn max_charge(&self, environment: &Environment) -> u32 {
        environment.config.hero_charge(*self)
    }

    pub fn transport_capacity(&self, environment: &Environment) -> usize {
        environment.config.hero_transport_capacity(*self) as usize
    }
}

/**
 * Hero purposefully doesn't have Environment.
 * This way, it's easier to create a dummy unit without access to Environment/Config
 */
#[derive(Clone, PartialEq, Eq)]
pub struct Hero {
    typ: HeroType,
    power: usize,
    charge: u32,
}

impl Debug for Hero {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Hero{{typ: {:?}", self.typ)?;
        write!(f, ", charge: {}", self.charge)?;
        write!(f, ", power: {}", self.power)?;
        write!(f, "}}")
    }
}

impl Hero {
    pub fn new(typ: HeroType) -> Self {
        Self {
            typ,
            power: 0,
            charge: 0,
        }
    }

    pub fn typ(&self) -> HeroType {
        self.typ
    }

    pub fn max_charge(&self, environment: &Environment) -> u32 {
        self.typ.max_charge(environment)
    }
    pub fn get_charge(&self) -> u32 {
        self.charge
    }
    pub fn set_charge(&mut self, environment: &Environment, charge: u32) {
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

    pub fn can_activate_power(&self, environment: &Environment, index: usize, automatic: bool) -> bool {
        if self.power == index {
            return false;
        }
        let power = match environment.config.hero_powers(self.typ).get(index) {
            Some(power) => power,
            None => return false,
        };
        power.required_charge <= self.charge
        && if automatic {
            index == self.get_next_power(environment)
        } else {
            power.usable_from_power.contains(&(self.power as u8))
        }
    }

    pub fn power_cost(&self, environment: &Environment, index: usize) -> u32 {
        let power = match environment.config.hero_powers(self.typ).get(index) {
            Some(power) => power,
            None => return 0,
        };
        power.required_charge
    }

    pub fn aura_range<D: Direction>(
        map: &Board<D>,
        unit: &Unit<D>,
        unit_pos: Point,
        transporter: Option<(&Unit<D>, usize)>,
    ) -> Option<usize> {
        map.environment().config.hero_aura_range(map, unit, unit_pos, transporter).map(|r| r as usize)
    }

    pub fn in_range<D: Direction>(
        map: &Board<D>,
        unit: &Unit<D>,
        unit_pos: Point,
        transporter: Option<(&Unit<D>, usize)>,
        target: Point,
    ) -> bool {
        Self::aura(map, unit, unit_pos, transporter).keys().any(|p| *p == target)
    }

    pub fn aura<D: Direction>(
        map: &Board<D>,
        unit: &Unit<D>,
        unit_pos: Point,
        transporter: Option<(&Unit<D>, usize)>,
    ) -> FxHashMap<Point, usize> {
        let mut result = FxHashMap::default();
        let mut aura_range = match Self::aura_range(map, unit, unit_pos, transporter) {
            Some(aura_range) => aura_range,
            _ => return result
        };
        result.insert(unit_pos, aura_range);
        for layer in get_neighbors_layers(map, unit_pos, aura_range) {
            aura_range -= 1;
            for p in layer {
                result.insert(p, aura_range);
            }
        }
        result
    }

    pub fn hero_influence_at<D: Direction>(map: &Board<D>, point: Point, only_owner_id: Option<i8>) -> Vec<HeroInfluence<D>> {
        let mut result = Vec::new();
        for p in valid_points(map) {
            if let Some(unit) = map.get_unit(p) {
                if only_owner_id.is_some() && Some(unit.get_owner_id()) != only_owner_id {
                    continue;
                }
                if let Some(hero) = unit.get_hero() {
                    if let Some(strength) = Self::aura(map, &unit, p, None).get(&point) {
                        result.push((unit.clone(), hero.clone(), p, None, *strength as u8));
                    }
                }
                for (i, u) in unit.get_transported().iter().enumerate() {
                    if let Some(hero) = u.get_hero() {
                        if let Some(strength) = Self::aura(map, u, p, Some((&unit, i))).get(&point) {
                            result.push((u.clone(), hero.clone(), p, Some(i), *strength as u8));
                        }
                    }
                }
            }
        }
        result
    }

    pub fn add_options_after_path<D: Direction>(
        &self,
        list: &mut Vec<UnitAction<D>>,
        game: &Board<D>,
    ) {
        for (i, _) in game.environment().config.hero_powers(self.typ).iter().enumerate() {
            if self.can_activate_power(&game.environment(), i, false) {
                list.push(UnitAction::hero_power(i, Vec::new()));
            }
        }
    }
}

impl SupportedZippable<&Environment> for Hero {
    fn export(&self, zipper: &mut Zipper, environment: &Environment) {
        self.typ.export(zipper, environment);
        zipper.write_u8(self.power as u8, bits_needed_for_max_value(environment.config.hero_powers(self.typ).len() as u32 - 1));
        if self.typ.max_charge(&environment) > 0 {
            let bits = bits_needed_for_max_value(self.typ.max_charge(&environment) as u32);
            zipper.write_u32(self.charge, bits);
        }
    }

    fn import(unzipper: &mut Unzipper, environment: &Environment) -> Result<Self, ZipperError> {
        let typ = HeroType::import(unzipper, environment)?;
        let mut result = Self::new(typ);
        result.power = unzipper.read_u8(bits_needed_for_max_value(environment.config.hero_powers(typ).len() as u32 - 1))? as usize;
        if typ.max_charge(environment) > 0 {
            let bits = bits_needed_for_max_value(typ.max_charge(environment) as u32);
            result.charge = typ.max_charge(environment).min(unzipper.read_u32(bits)?);
        }
        Ok(result)
    }
}

pub type HeroInfluence<D> = (Unit<D>, Hero, Point, Option<usize>, u8);
pub type HeroInfluenceWithId<D> = (UnitId<D>, Unit<D>, Hero, Point, Option<usize>, u8);

pub struct HeroMap<D: Direction> {
    hero_auras: FxHashMap<(Point, i8), Vec<HeroInfluence<D>>>,
}

impl <D: Direction> HeroMap<D> {
    pub fn new_empty() -> Self {
        Self {
            hero_auras: FxHashMap::default(),
        }
    }

    pub fn new(map: &Board<D>, only_owner_id: Option<i8>) -> Self {
        let hero_auras = Self::_new(map, only_owner_id, |unit: &Unit<D>, unit_pos: Point, transporter: Option<(&Unit<D>, usize)>| {
            Box::new(Hero::aura(map, unit, unit_pos, transporter).into_iter())
        });
        Self {
            hero_auras,
            //only_owner_id,
        }
    }

    pub fn new_without_aura(map: &Board<D>, only_owner_id: Option<i8>) -> Self {
        let hero_auras = Self::_new(map, only_owner_id, |_, unit_pos: Point, _| {
            Box::new([(unit_pos, 0)].into_iter())
        });
        Self {
            hero_auras,
            //only_owner_id,
        }
    }

    fn _new(map: &Board<D>, only_owner_id: Option<i8>, aura: impl Fn(&Unit<D>, Point, Option<(&Unit<D>, usize)>) -> Box<dyn Iterator<Item = (Point, usize)>>) -> FxHashMap<(Point, i8), Vec<HeroInfluence<D>>> {
        let mut heroes = Vec::new();
        for p in valid_points(map) {
            if let Some(unit) = map.get_unit(p) {
                if only_owner_id.is_some() && Some(unit.get_owner_id()) != only_owner_id {
                    continue;
                }
                if let Some(hero) = unit.get_hero() {
                    heroes.push((unit.clone(), hero.clone(), p, None));
                }
                for (i, unit) in unit.get_transported().iter().enumerate() {
                    if let Some(hero) = unit.get_hero() {
                        heroes.push((unit.clone(), hero.clone(), p, Some(i)));
                    }
                }
            }
        }
        let mut hero_auras: FxHashMap<(Point, i8), Vec<HeroInfluence<D>>> = FxHashMap::default();
        for hero in heroes {
            let transporter = hero.3.map(|i| (map.get_unit(hero.2).unwrap(), i));
            for (p, strength) in aura(&hero.0, hero.2, transporter) {
                let key = (p, hero.0.get_owner_id());
                let value = (hero.0.clone(), hero.1.clone(), hero.2, hero.3, strength as u8);
                if let Some(list) = hero_auras.get_mut(&key) {
                    list.push(value);
                } else {
                    hero_auras.insert(key, vec![value]);
                }
            }
        }
        hero_auras
    }

    pub(crate) fn with_ids(&self, handler: &mut EventHandler<D>) -> HeroMapWithId<D> {
        let mut hero_auras: FxHashMap<(Point, i8), Vec<HeroInfluenceWithId<D>>> = FxHashMap::default();
        for (key, list) in self.hero_auras.iter() {
            let list = list.iter()
                .cloned()
                .map(|(unit, hero, position, unload_index, strength)| {
                    let id = handler.observe_unit(position, unload_index);
                    (id, unit, hero, position, unload_index, strength)
                }).collect();
            hero_auras.insert(*key, list);
        }
        HeroMapWithId(rhai::Shared::new(hero_auras))
    }

    pub fn get(&self, position: Point, owner_id: i8) -> &[HeroInfluence<D>] {
        self.hero_auras.get(&(position, owner_id)).map(|h| h.as_slice()).unwrap_or(&[])
    }

    pub fn iter(&self) -> impl Iterator<Item = (&(Point, i8), &Vec<HeroInfluence<D>>)> {
        self.hero_auras.iter()
    }

    pub fn iter_owned(&self, owner_id: i8) -> impl Iterator<Item = &HeroInfluence<D>> {
        self.hero_auras.iter()
            .filter(move |((_, o), _)| *o == owner_id)
            .map(|(_, influence)| influence)
            .flatten()
    }
}

#[derive(Clone)]
pub(crate) struct HeroMapWithId<D: Direction>(rhai::Shared<FxHashMap<(Point, i8), Vec<HeroInfluenceWithId<D>>>>);

impl <D: Direction> HeroMapWithId<D> {
    pub(crate) fn get(&self, position: Point, owner_id: i8) -> &[HeroInfluenceWithId<D>] {
        self.0.get(&(position, owner_id)).map(|h| h.as_slice()).unwrap_or(&[])
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HeroChargeChange(pub i32);

impl SupportedZippable<&Environment> for HeroChargeChange {
    fn export(&self, zipper: &mut Zipper, support: &Environment) {
        let max = support.config.max_hero_charge() as i32;
        zipper.write_u32((self.0 + max) as u32, bits_needed_for_max_value(max as u32 * 2));
    }
    fn import(unzipper: &mut Unzipper, support: &Environment) -> Result<Self, ZipperError> {
        let max = support.config.max_hero_charge() as i32;
        Ok(Self(unzipper.read_u32(bits_needed_for_max_value(max as u32 * 2))? as i32 - max))
    }
}

impl From<i32> for HeroChargeChange {
    fn from(value: i32) -> Self {
        Self(value)
    }
}
