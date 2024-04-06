use std::collections::{HashMap, HashSet};

use crate::config::parse::*;
use crate::terrain::*;
use crate::units::attributes::AttributeOverride;

use super::ConfigParseError;

#[derive(Debug)]
pub struct TerrainTypeConfig {
    pub(super) id: TerrainType,
    pub(super) name: String,
    pub(super) needs_owner: bool,
    pub(super) update_amphibious: Option<AmphibiousTyping>,
    pub(super) max_anger: u8,
    pub(super) chess: bool,
    // can be modified by commander / hero aura
    pub(super) capture_resistance: u8,
    pub(super) max_builds_per_turn: u8,
    pub(super) income_factor: i16,
    pub(super) vision_range: i8,
    pub(super) can_repair: bool,
    pub(super) can_build: bool,
    pub(super) can_sell_hero: bool,
    pub(super) extra_movement_options: ExtraMovementOptions,
    pub(super) build_overrides: HashSet<AttributeOverride>,
}

impl TerrainTypeConfig {
    pub fn parse(data: &HashMap<TerrainTypeConfigHeader, &str>) -> Result<Self, ConfigParseError> {
        use TerrainTypeConfigHeader as H;
        use ConfigParseError as E;
        let get = |key| {
            data.get(&key).ok_or(E::MissingColumn(format!("{key:?}")))
        };
        let result = Self {
            id: parse(data, H::Id)?,
            name: get(H::Name)?.to_string(),
            needs_owner: parse_def(data, H::NeedsOwner, false)?,
            capture_resistance: parse_def(data, H::CaptureResistance, 0)?,
            update_amphibious: match data.get(&H::UpdateAmphibious) {
                Some(s) if s.len() > 0 => Some(s.parse()?),
                _ => None,
            },
            max_builds_per_turn: parse_def(data, H::MaxBuildsPerTurn, 0)?,
            max_anger: parse_def(data, H::MaxAnger, 0)?,
            vision_range: parse_def(data, H::VisionRange, -1)?,
            income_factor: parse_def(data, H::IncomeFactor, 0)?,
            can_repair: parse_def(data, H::Repair, false)?,
            can_build: parse_def(data, H::Build, false)?,
            can_sell_hero: parse_def(data, H::SellsHero, false)?,
            chess: parse_def(data, H::Chess, false)?,
            extra_movement_options: parse_def(data, H::MovementOptions, ExtraMovementOptions::None)?,
            build_overrides: parse_vec_def(data, H::BuildOverrides, Vec::new())?.into_iter().collect(),
        };
        result.simple_validation()?;
        Ok(result)
    }

    pub fn simple_validation(&self) -> Result<(), ConfigParseError> {
        if self.name.trim().len() == 0 {
            return Err(ConfigParseError::NameTooShort);
        }
        // TODO: error if build_overrides overrides its own values
        if self.max_builds_per_turn == 0 && self.can_build {
            // TODO: could remove can_build column
        }
        Ok(())
    }
}

crate::listable_enum! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum TerrainTypeConfigHeader {
        Id,
        Name,
        NeedsOwner,
        CaptureResistance,
        UpdateAmphibious,
        MaxBuildsPerTurn,
        MaxAnger,
        VisionRange,
        IncomeFactor,
        Repair,
        Build,
        SellsHero,
        Chess,
        MovementOptions,
        BuildOverrides,
    }
}
