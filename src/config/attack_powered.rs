use std::error::Error;

use num_rational::Rational32;
use rustc_hash::FxHashMap as HashMap;
use rustc_hash::FxHashSet;
use rhai::*;

use crate::combat::*;
use crate::config::parse::*;
use crate::dyn_opt;
use crate::map::board::Board;
use crate::map::direction::Direction;
use crate::script::*;
use crate::units::hero::HeroMap;
use crate::units::UnitData;

use super::file_loader::FileLoader;
use super::file_loader::TableLine;
use super::number_modification::NumberMod;
use super::unit_filter::unit_filter_scope;
use super::ConfigParseError;
use super::unit_filter::UnitFilter;

#[derive(Debug)]
pub(super) struct AttackPoweredConfig {
    pub(super) affects: Vec<AttackFilter>,
    // affects ConfiguredAttack
    //pub splash_pattern: Option<SplashPattern>,
    pub attack_priority: NumberMod<i8>,
    pub splash_range: NumberMod<u8>,
    pub focus: Option<AttackTargetingFocus>,
    // affects AttackInstance
    pub allows_counter_attack: Option<bool>,
    pub splash_priority: NumberMod<Rational32>,
    pub direction_modifier: Option<DisplaceDirectionModifier>,
    // user-defined columns
    custom_columns: HashMap<String, NumberMod<Rational32>>,
    scripts: HashMap<String, (usize, usize, Option<Rational32>)>,
}

impl TableLine for AttackPoweredConfig {
    type Header = AttackPoweredConfigHeader;
    fn parse(data: &HashMap<Self::Header, &str>, loader: &mut FileLoader) -> Result<Self, Box<dyn Error>> {
        use AttackPoweredConfigHeader as H;
        let mut custom_columns = HashMap::default();
        let mut scripts = HashMap::default();
        for (header, s) in data {
            if let H::Custom(name) = header {
                let s = s.trim();
                if s.len() > 0 {
                    if name.starts_with("On") {
                        let (base, s) = string_base(s);
                        let (priority, f) = match base {
                            "S" | "Simple" => (None, parse_tuple1::<String>(s, loader)?.0),
                            "P" | "WithPriority" => {
                                let (priority, name, _) = parse_tuple2(s, loader)?;
                                (Some(priority), name)
                            }
                            _ => return Err(ConfigParseError::UnknownEnumMember(format!("OnDefend::{base}")).into())
                        };
                        let f = loader.rhai_function(&f, 0..=10)?;
                        scripts.insert(name.clone(), (f.index, f.parameters.len(), priority));
                    } else {
                        let nm =NumberMod::from_conf(s, loader)?.0;
                        custom_columns.insert(name.clone(), nm);
                    }
                }
            }
        }
        Ok(Self {
            affects: parse_vec_def(data, H::Filter, Vec::new(), loader)?,
            attack_priority: parse_def(data, H::AttackPriority, NumberMod::Keep, loader)?,
            splash_range: parse_def(data, H::SplashRange, NumberMod::Keep, loader)?,
            focus: match data.get(&H::Targeting) {
                Some(s) if s.len() > 0 => Some(AttackTargetingFocus::from_conf(s, loader)?.0),
                _ => None,
            },
            allows_counter_attack: match data.get(&H::AllowsCounterAttack) {
                Some(s) if s.len() > 0 => Some(s.parse().map_err(|_| ConfigParseError::InvalidBool(s.to_string()))?),
                _ => None,
            },
            splash_priority: parse_def(data, H::SplashPriority, NumberMod::Keep, loader)?,
            direction_modifier: match data.get(&H::DirectionModifier) {
                Some(s) if s.len() > 0 => Some(DisplaceDirectionModifier::from_conf(s, loader)?.0),
                _ => None,
            },
            custom_columns,
            scripts,
        })
    }

    fn simple_validation(&self) -> Result<(), Box<dyn Error>> {
        Ok(())
    }
}

impl AttackPoweredConfig {
    pub(super) fn get_fraction(&self, column_name: &String) -> NumberMod<Rational32> {
        self.custom_columns.get(column_name)
        .cloned()
        .unwrap_or(NumberMod::Keep)
    }

    pub(super) fn get_script(&self, column_name: &String, parameter_count: usize) -> Option<(usize, Option<Rational32>)> {
        self.scripts.get(column_name)
        .filter(|(_, p, _)| *p == parameter_count)
        .map(|(f, _, priority)| (*f, *priority))
    }
}

crate::enum_with_custom! {
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum AttackPoweredConfigHeader {
        Filter,
        // affects both ConfiguredAttack and AttackInstance
        // affects AttackInstance
        AttackPriority,
        SplashRange,
        Targeting,
        // affects AttackInstance
        AllowsCounterAttack,
        SplashPriority,
        DirectionModifier,
        // events
        OnAttack,
    }
}

#[derive(Debug, Clone)]
pub(crate) enum AttackFilter {
    Attack(FxHashSet<AttackType>),
    AttackPriority(i8, i8), // min, max
    SplashDistance(i32, i32), // min, max
    UnitFilter(UnitFilter),
    // override UnitFilter variants of the same name
    Rhai(usize),
    Not(Vec<Self>),
}

impl FromConfig for AttackFilter {
    fn from_conf<'a>(s: &'a str, loader: &mut FileLoader) -> Result<(Self, &'a str), ConfigParseError> {
        let (base, mut remainder) = string_base(s);
        Ok((match base {
            "AttackType" | "AT" => {
                let (list, r) = parse_inner_vec(remainder, true, loader)?;
                remainder = r;
                Self::Attack(list.into_iter().collect())
            }
            "AttackPriority" => {
                if let Ok((min, max, r)) = parse_tuple2(remainder, loader) {
                    remainder = r;
                    Self::AttackPriority(min, max)
                } else {
                    let (value, r) = parse_tuple1(remainder, loader)?;
                    remainder = r;
                    Self::AttackPriority(value, value)
                }
            }
            "SplashDistance" => {
                if let Ok((min, max, r)) = parse_tuple2(remainder, loader) {
                    remainder = r;
                    Self::SplashDistance(min, max)
                } else {
                    let (value, r) = parse_tuple1(remainder, loader)?;
                    remainder = r;
                    Self::SplashDistance(value, value)
                }
            }
            // override UnitFilter variants of the same name
            "Rhai" | "Script" => {
                let (name, r) = parse_tuple1::<String>(remainder, loader)?;
                remainder = r;
                Self::Rhai(loader.rhai_function(&name, 0..=0)?.index)
            }
            "Not" => {
                let (list, r) = parse_inner_vec::<Self>(remainder, true, loader)?;
                remainder = r;
                Self::Not(list)
            }
            _ => {
                match UnitFilter::from_conf(s, loader) {
                    Ok((value, r)) => {
                        remainder = r;
                        Self::UnitFilter(value)
                    }
                    // could remap UnknownEnumMember error
                    Err(e) => return Err(e)
                }
            }
        }, remainder))
    }
}

impl AttackFilter {
    pub fn check<D: Direction>(
        &self,
        game: &Board<D>,
        attack: &ConfiguredAttack,
        splash: Option<&AttackInstance>,
        unit_data: UnitData<D>,
        other_unit_data: Option<UnitData<D>>,
        // the heroes affecting this unit. shouldn't be taken from game since they could have died before this function is called
        heroes: &HeroMap<D>,
        // empty if the unit hasn't moved
        is_counter: bool,
    ) -> bool {
        match self {
            Self::Attack(t) => t.contains(&attack.typ),
            Self::AttackPriority(min, max) => *min <= attack.priority && attack.priority <= *max,
            Self::SplashDistance(min, max) => {
                let Some(splash) = splash else {
                    return false;
                };
                count_from_both_ends(*min, attack.splash_range as usize + 1) <= splash.splash_distance as i32
                && splash.splash_distance as i32 <= count_from_both_ends(*max, attack.splash_range as usize + 1)
            }
            Self::UnitFilter(uf) => uf.check(game, unit_data, other_unit_data, heroes, is_counter),
            Self::Rhai(function_index) => {
                let scope = attack_filter_scope(game, attack, splash, unit_data, other_unit_data, heroes, is_counter);
                let executor = game.executor(scope);
                match executor.run::<D, bool>(*function_index, ()) {
                    Ok(result) => result,
                    Err(_e) => {
                        // TODO: log error
                        false
                    }
                }
            }
            Self::Not(negated) => {
                // returns true if at least one check returns false
                // if you need all checks to return false, put them into separate Self::Not wrappers instead
                negated.iter()
                .any(|negated| !negated.check(game, attack, splash, unit_data, other_unit_data, heroes, is_counter))
            }
        }
    }
}

fn count_from_both_ends(value: i32, count: usize) -> i32 {
    if value >= 0 {
        value
    } else {
        (count as i32 + value).max(0)
    }
}

pub(crate) fn attack_filter_scope<D: Direction>(
    game: &Board<D>,
    _attack: &ConfiguredAttack,
    splash: Option<&AttackInstance>,
    unit_data: UnitData<D>,
    other_unit_data: Option<UnitData<D>>,
    heroes: &HeroMap<D>,
    // true only during counter-attacks
    is_counter: bool,
) -> Scope<'static> {
    let mut scope = unit_filter_scope(game, unit_data, other_unit_data, heroes, is_counter);
    scope.push_constant(CONST_NAME_SPLASH_DISTANCE, dyn_opt(splash.map(|s| s.splash_distance as i32)));
    scope

}
