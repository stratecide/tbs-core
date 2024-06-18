use std::collections::HashMap;
use std::collections::HashSet;

use num_rational::Rational32;

use crate::config::parse::*;
use crate::game::game_view::GameView;
use crate::map::direction::Direction;
use crate::map::point::Point;
use crate::script::terrain::TerrainScript;
use crate::terrain::terrain::Terrain;
use crate::terrain::TerrainType;
use crate::units::attributes::*;
use crate::units::hero::Hero;
use crate::units::hero::HeroInfluence;
use crate::units::unit::Unit;

use super::commander_unit_config::PowerRestriction;
use super::number_modification::NumberMod;
use super::ConfigParseError;

#[derive(Debug, Clone)]
pub(super) enum TerrainFilter {
    Type(HashSet<TerrainType>),
    Bubble,
    Not(Vec<Self>),
}

impl FromConfig for TerrainFilter {
    fn from_conf(s: &str) -> Result<(Self, &str), ConfigParseError> {
        let (base, mut remainder) = string_base(s);
        Ok((match base {
            "T" | "Type" => {
                let (list, r) = parse_inner_vec::<TerrainType>(remainder, true)?;
                remainder = r;
                Self::Type(list.into_iter().collect())
            }
            "Bubble" => Self::Bubble,
            "Not" => {
                let (list, r) = parse_inner_vec::<Self>(remainder, true)?;
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
        map: &impl GameView<D>,
        pos: Point,
        terrain: &Terrain,
        is_bubble: bool,
        // the heroes affecting this terrain. shouldn't be taken from game since they could have died before this function is called
        heroes: &[HeroInfluence<D>],
    ) -> bool {
        match self {
            Self::Type(t) => t.contains(&terrain.typ()),
            Self::Bubble => is_bubble,
            Self::Not(negated) => {
                // returns true if at least one check returns false
                // if you need all checks to return false, put them into separate Self::Not wrappers instead
                negated.iter()
                .any(|negated| !negated.check(map, pos, terrain, is_bubble, heroes))
            }
        }
    }
}

#[derive(Debug)]
pub(super) struct TerrainPoweredConfig {
    pub(super) power: PowerRestriction,
    pub(super) affects: Vec<TerrainFilter>,
    pub(super) vision: NumberMod<i8>,
    pub(super) income: NumberMod<Rational32>,
    pub(super) repair: Option<bool>,
    pub(super) build: Option<bool>,
    pub(super) sells_hero: Option<bool>,
    pub(super) build_overrides: HashSet<AttributeOverride>,
    pub(super) on_start_turn: Vec<TerrainScript>,
    pub(super) on_end_turn: Vec<TerrainScript>,
    pub(super) on_build: Vec<TerrainScript>,
}

impl TerrainPoweredConfig {
    pub fn parse(data: &HashMap<TerrainPoweredConfigHeader, &str>) -> Result<Self, ConfigParseError> {
        use TerrainPoweredConfigHeader as H;
        let result = Self {
            power: match data.get(&H::Power) {
                Some(s) if s.len() > 0 => s.parse()?,
                _ => PowerRestriction::None,
            },
            affects: parse_vec_def(data, H::Affects, Vec::new())?,
            vision: parse_def(data, H::Vision, NumberMod::Keep)?,
            income: parse_def(data, H::Income, NumberMod::Keep)?,
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
            build_overrides: parse_vec_def(data, H::BuildOverrides, Vec::new())?.into_iter().collect(),
            on_start_turn: parse_vec_def(data, H::OnStartTurn, Vec::new())?,
            on_end_turn: parse_vec_def(data, H::OnEndTurn, Vec::new())?,
            on_build: parse_vec_def(data, H::OnBuild, Vec::new())?,
        };
        result.simple_validation()?;
        Ok(result)
    }

    pub fn simple_validation(&self) -> Result<(), ConfigParseError> {
        let mut overrides = HashSet::new();
        for key in self.build_overrides.iter().map(AttributeOverride::key) {
            if !overrides.insert(key) {
                // TODO: return error
            }
        }
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
        BuildOverrides,
        OnStartTurn,
        OnEndTurn,
        OnBuild,
    }
}
