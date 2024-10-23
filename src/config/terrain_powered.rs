use std::error::Error;

use rustc_hash::{FxHashMap as HashMap, FxHashSet as HashSet};
use num_rational::Rational32;

use crate::commander::commander_type::CommanderType;
use crate::config::parse::*;
use crate::game::game_view::GameView;
use crate::map::direction::Direction;
use crate::map::point::Point;
use crate::script::executor::Executor;
use crate::terrain::terrain::Terrain;
use crate::terrain::TerrainType;
use crate::units::hero::HeroInfluence;

use super::file_loader::{FileLoader, TableLine};
use super::number_modification::NumberMod;
use super::ConfigParseError;

#[derive(Debug, Clone)]
pub(super) enum TerrainFilter {
    Rhai(usize),
    Commander(CommanderType, Option<u8>),
    Type(HashSet<TerrainType>),
    Bubble,
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
            "Bubble" => Self::Bubble,
            "Not" => {
                let (list, r) = parse_inner_vec::<Self>(remainder, true, loader)?;
                remainder = r;
                Self::Not(list)
            }
            _ => return Err(ConfigParseError::UnknownEnumMember(s.to_string()))
        }, remainder))
    }
}

impl TerrainFilter {
    pub fn check<D: Direction>(
        &self,
        game: &impl GameView<D>,
        pos: Point,
        terrain: &Terrain<D>,
        is_bubble: bool,
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
            Self::Bubble => is_bubble,
            Self::Not(negated) => {
                // returns true if at least one check returns false
                // if you need all checks to return false, put them into separate Self::Not wrappers instead
                negated.iter()
                .any(|negated| !negated.check(game, pos, terrain, is_bubble, heroes, executor))
            }
        }
    }
}

#[derive(Debug)]
pub(super) struct TerrainPoweredConfig {
    pub(super) affects: Vec<TerrainFilter>,
    pub(super) vision: NumberMod<i8>,
    pub(super) income: NumberMod<Rational32>,
    pub(super) repair: Option<bool>,
    pub(super) build: Option<bool>,
    pub(super) sells_hero: Option<bool>,
    //pub(super) build_overrides: HashSet<AttributeOverride>,
    pub(super) on_start_turn: Option<usize>,
    pub(super) on_end_turn: Option<usize>,
    //pub(super) on_build: Option<usize>,
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
            income: parse_def(data, H::Income, NumberMod::Keep, loader)?,
            repair: match data.get(&H::Repair) {
                Some(s) if s.len() > 0 => Some(s.parse().map_err(|_| ConfigParseError::InvalidBool(s.to_string()))?),
                _ => None,
            },
            build: match data.get(&H::Build) {
                Some(s) if s.len() > 0 => Some(s.parse().map_err(|_| ConfigParseError::InvalidBool(s.to_string()))?),
                _ => None,
            },
            sells_hero: match data.get(&H::SellsHero) {
                Some(s) if s.len() > 0 => Some(s.parse().map_err(|_| ConfigParseError::InvalidBool(s.to_string()))?),
                _ => None,
            },
            //build_overrides: parse_vec_def(data, H::BuildOverrides, Vec::new(), loader)?.into_iter().collect(),
            on_start_turn: match data.get(&H::OnStartTurn) {
                Some(s) if s.len() > 0 => Some(loader.rhai_function(s, 0..=0)?.index),
                _ => None,
            },
            /*on_build: match data.get(&H::OnBuild) {
                Some(s) if s.len() > 0 => Some(loader.rhai_function(s, 0..=0)?.index),
                _ => None,
            },*/
            on_end_turn: match data.get(&H::OnEndTurn) {
                Some(s) if s.len() > 0 => Some(loader.rhai_function(s, 0..=0)?.index),
                _ => None,
            },
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
        /*let mut overrides = HashSet::default();
        for key in self.build_overrides.iter().map(AttributeOverride::key) {
            if !overrides.insert(key) {
                // TODO: return error
            }
        }*/
        Ok(())
    }
}

crate::listable_enum! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum TerrainPoweredConfigHeader {
        Power,
        Affects,
        Vision,
        Income,
        Repair,
        Build,
        SellsHero,
        //BuildOverrides,
        OnStartTurn,
        OnEndTurn,
        //OnBuild,
        ActionScript,
    }
}
