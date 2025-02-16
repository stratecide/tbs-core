use rhai::ImmutableString;
use rustc_hash::FxHashMap as HashMap;
use std::error::Error;
use num_rational::Rational32;

use crate::combat::*;

use super::attack_powered::AttackFilter;
use super::file_loader::{FileLoader, TableLine};
use super::number_modification::NumberMod;
use super::ConfigParseError;
use super::parse::*;

#[derive(Debug)]
pub struct AttackConfig {
    pub attack_type: AttackType,
    unparsed_condition: String,
    pub(super) condition: Vec<AttackFilter>,
    pub priority: i8,
    pub splash_type: SplashType,
    pub splash_pattern: SplashPattern,
    pub splash_range: u8,
    pub focus: AttackTargetingFocus,
    pub(super) custom_columns: HashMap<ImmutableString, ImmutableString>,
}

impl TableLine for AttackConfig {
    type Header = AttackConfigHeader;
    fn parse(data: &HashMap<Self::Header, &str>, loader: &mut FileLoader) -> Result<Self, Box<dyn Error>> {
        use AttackConfigHeader as H;
        use ConfigParseError as E;
        let get = |key| {
            data.get(&key).ok_or(E::MissingColumn(format!("{key:?}")))
        };
        let mut custom_columns = HashMap::default();
        for (header, s) in data {
            if let H::Custom(name) = header {
                let s = s.trim();
                custom_columns.insert(name.into(), s.into());
            }
        }
        let result = Self {
            attack_type: AttackType::parse_new(get(H::AttackType)?, loader)?,
            unparsed_condition: parse_def(data, H::Condition, String::new(), loader)?,
            condition: Vec::new(), // can't parse this yet since not all AttackTypes have been parsed
            priority: parse_def(data, H::Priority, 0, loader)?,
            splash_pattern: SplashPattern {
                points: parse_def(data, H::SplashPattern, SplashDamagePointSource::AttackPattern, loader)?,
                directions: parse_def(data, H::SplashDirection, SplashDamageDirectionSource::AttackInput, loader)?,
            },
            splash_range: parse_def(data, H::SplashRange, 0, loader)?,
            focus: parse_def(data, H::Targeting, AttackTargetingFocus::Unit, loader)?,
            splash_type: parse(data, H::SplashType, loader)?,
            custom_columns,
        };
        Ok(result)
    }

    fn simple_validation(&self) -> Result<(), Box<dyn Error>> {
        Ok(())
    }
}

impl AttackConfig {
    pub(super) fn parse_deferred(&mut self, loader: &mut FileLoader) -> Result<(), Box<dyn Error>> {
        self.condition = parse_inner_vec(&std::mem::take(&mut self.unparsed_condition), false, loader)?.0;
        Ok(())
    }
}

crate::enum_with_custom! {
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum AttackConfigHeader {
        AttackType,
        Condition,
        Priority,
        SplashRange,
        SplashPattern,
        SplashDirection,
        Targeting,
        SplashType,
    }
}

#[derive(Debug)]
pub struct AttackSplashConfig {
    pub splash_type: SplashType,
    unparsed_condition: String,
    pub(super) condition: Vec<AttackFilter>,
    pub allows_counter_attack: bool,
    pub priority: Rational32,
    pub direction_modifier: DisplaceDirectionModifier,
    pub script: AttackInstanceScript,
    custom_columns: HashMap<String, NumberMod<Rational32>>,
}

impl TableLine for AttackSplashConfig {
    type Header = AttackSplashConfigHeader;
    fn parse(data: &HashMap<Self::Header, &str>, loader: &mut FileLoader) -> Result<Self, Box<dyn Error>> {
        use AttackSplashConfigHeader as H;
        use ConfigParseError as E;
        let get = |key| {
            data.get(&key).ok_or(E::MissingColumn(format!("{key:?}")))
        };
        let script = match get(H::Script)?.trim() {
            "Displace" => {
                AttackInstanceScript::Displace {
                    distance: parse_def(data, H::PushDistance, Rational32::from_integer(1), loader)?,
                    push_limit: parse_def(data, H::PushLimit, Rational32::from_integer(0), loader)?,
                    throw: parse_def(data, H::Throw, false, loader)?,
                    neighbor_mode: crate::map::map::NeighborMode::FollowPipes, // TODO
                }
            }
            s => {
                AttackInstanceScript::Rhai {
                    build_script: loader.rhai_function(&format!("{s}"), 0..=0)?.index,
                }
            }
        };
        let mut custom_columns = HashMap::default();
        for (header, s) in data {
            if let H::Custom(name) = header {
                let s = s.trim();
                if s.len() > 0 {
                    let nm = NumberMod::from_conf(s, loader)?.0;
                    custom_columns.insert(name.clone(), nm);
                }
            }
        }
        let splash_type = get(H::SplashType)?;
        if splash_type.len() == 0 {
            return Err(E::NameTooShort.into());
        }
        let splash_type = match loader.splash_types.iter().position(|s| s.as_str() == *splash_type) {
            Some(i) => SplashType(i),
            None => {
                loader.splash_types.push(splash_type.to_string());
                SplashType(loader.splash_types.len() - 1)
            }
        };
        let result = Self {
            splash_type,
            unparsed_condition: parse_def(data, H::Condition, String::new(), loader)?,
            condition: Vec::new(), // can't parse this yet since not all SplashTypes have been parsed
            allows_counter_attack: parse_def(data, H::AllowsCounterAttack, false, loader)?,
            priority: parse_def(data, H::Priority, Rational32::from_integer(0), loader)?,
            direction_modifier: parse_def(data, H::DirectionModifier, DisplaceDirectionModifier::Keep, loader)?,
            script,
            custom_columns,
        };
        Ok(result)
    }

    fn simple_validation(&self) -> Result<(), Box<dyn Error>> {
        Ok(())
    }
}

impl AttackSplashConfig {
    pub(super) fn parse_deferred(&mut self, loader: &mut FileLoader) -> Result<(), Box<dyn Error>> {
        self.condition = parse_inner_vec(&std::mem::take(&mut self.unparsed_condition), false, loader)?.0;
        Ok(())
    }
}

crate::enum_with_custom! {
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum AttackSplashConfigHeader {
        SplashType,
        Priority,
        AllowsCounterAttack,
        DirectionModifier,
        Condition,
        Script,
        // Displace parameters
        PushDistance,
        PushLimit,
        Throw,
    }
}
