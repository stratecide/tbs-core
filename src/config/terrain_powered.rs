use std::error::Error;

use num_rational::Rational32;
use rustc_hash::{FxHashMap as HashMap, FxHashSet as HashSet};

use crate::commander::commander_type::CommanderType;
use crate::config::parse::*;
use crate::game::game_view::GameView;
use crate::map::direction::Direction;
use crate::map::point::Point;
use crate::script::executor::Executor;
use crate::tags::*;
use crate::terrain::terrain::Terrain;
use crate::terrain::TerrainType;
use crate::units::hero::HeroInfluence;

use super::file_loader::{FileLoader, TableLine};
use super::number_modification::NumberMod;
use super::{ConfigParseError, OwnershipPredicate};

#[derive(Debug, Clone)]
pub(crate) enum TerrainFilter {
    Rhai(usize),
    Commander(CommanderType, Option<u8>),
    Type(HashSet<TerrainType>),
    Ownable,
    Unowned,
    OwnerTurn,
    Flag(HashSet<FlagKey>),
    Not(Vec<Self>),
}

impl FromConfig for TerrainFilter {
    fn from_conf<'a>(s: &'a str, loader: &mut FileLoader) -> Result<(Self, &'a str), ConfigParseError> {
        let (base, mut remainder) = string_base(s);
        Ok((match base {
            "Rhai" | "Script" => {
                let (name, r) = parse_tuple1::<String>(remainder, loader)?;
                remainder = r;
                Self::Rhai(loader.rhai_function(&name, 0..=0)?.index)
            }
            "Commander" | "Co" => {
                if let Ok((commander, power, r)) = parse_tuple2(remainder, loader) {
                    remainder = r;
                    Self::Commander(commander, Some(power))
                } else {
                    let (commander, r) = parse_tuple1(remainder, loader)?;
                    remainder = r;
                    Self::Commander(commander, None)
                }
            }
            "T" | "Type" => {
                let (list, r) = parse_inner_vec::<TerrainType>(remainder, true, loader)?;
                remainder = r;
                Self::Type(list.into_iter().collect())
            }
            "Ownable" => Self::Ownable,
            "Unowned" => Self::Unowned,
            "OwnerTurn" => Self::OwnerTurn,
            "Flag" | "F" => {
                let (list, r) = parse_inner_vec::<FlagKey>(remainder, true, loader)?;
                remainder = r;
                Self::Flag(list.into_iter().collect())
            }
            "Not" => {
                let (list, r) = parse_inner_vec::<Self>(remainder, true, loader)?;
                remainder = r;
                Self::Not(list)
            }
            _ => return Err(ConfigParseError::UnknownEnumMember(format!("TerrainFilter::{s}")))
        }, remainder))
    }
}

impl TerrainFilter {
    pub fn check<D: Direction>(
        &self,
        game: &impl GameView<D>,
        pos: Point,
        terrain: &Terrain<D>,
        // the heroes affecting this terrain. shouldn't be taken from game since they could have died before this function is called
        heroes: &[HeroInfluence<D>],
        executor: &Executor,
    ) -> bool {
        match self {
            Self::Rhai(function_index) => {
                match executor.run(*function_index, ()) {
                    Ok(result) => result,
                    Err(_e) => {
                        // TODO: log error
                        false
                    }
                }
            }
            Self::Commander(commander_type, power) => {
                let commander = terrain.get_commander(game);
                commander.typ() == *commander_type
                && (power.is_none() || power.clone().unwrap() as usize == commander.get_active_power())
            }
            Self::Type(t) => t.contains(&terrain.typ()),
            Self::Ownable => terrain.environment().config.terrain_ownership(terrain.typ()) != OwnershipPredicate::Never,
            Self::Unowned => terrain.get_owner_id() < 0,
            Self::OwnerTurn => terrain.get_owner_id() == game.current_owner(),
            Self::Flag(flags) => flags.iter().any(|flag| terrain.has_flag(flag.0)),
            Self::Not(negated) => {
                // returns true if at least one check returns false
                // if you need all checks to return false, put them into separate Self::Not wrappers instead
                negated.iter()
                .any(|negated| !negated.check(game, pos, terrain, heroes, executor))
            }
        }
    }
}

#[derive(Debug)]
pub(super) struct TerrainPoweredConfig {
    pub(super) affects: Vec<TerrainFilter>,
    pub(super) vision: NumberMod<i8>,
    pub(super) income_factor: NumberMod<Rational32>,
    pub(super) action_script: Option<(usize, usize)>,
}

impl TableLine for TerrainPoweredConfig {
    type Header = TerrainPoweredConfigHeader;
    fn parse(data: &HashMap<Self::Header, &str>, loader: &mut FileLoader) -> Result<Self, Box<dyn Error>> {
        use TerrainPoweredConfigHeader as H;
        let power = parse_vec_def(data, H::Power, Vec::new(), loader)?;
        let affects = parse_vec_def(data, H::Affects, Vec::new(), loader)?;
        let result = Self {
            affects: power.into_iter().chain(affects.into_iter()).collect(),
            vision: parse_def(data, H::Vision, NumberMod::Keep, loader)?,
            income_factor: parse_def(data, H::IncomeFactor, NumberMod::Keep, loader)?,
            action_script: match data.get(&H::ActionScript) {
                Some(s) if s.len() > 0 => {
                    let exe = loader.rhai_function(s, 1..=1)?;
                    let input = loader.rhai_function(&format!("{s}_input"), 0..=0)?.index;
                    Some((input, exe.index))
                }
                _ => None,
            },
        };
        Ok(result)
    }

    fn simple_validation(&self) -> Result<(), Box<dyn Error>> {
        Ok(())
    }
}

crate::listable_enum! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum TerrainPoweredConfigHeader {
        Power,
        Affects,
        Vision,
        IncomeFactor,
        ActionScript,
    }
}
