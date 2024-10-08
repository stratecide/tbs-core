use std::error::Error;

use rustc_hash::{FxHashMap as HashMap, FxHashSet as HashSet};

use crate::config::parse::*;
use crate::terrain::*;
use crate::units::attributes::AttributeOverride;

use super::file_loader::{FileLoader, TableLine};
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
    #[cfg(feature = "rendering")]
    pub(super) preview: Vec<(interfaces::PreviewShape, Option<[u8; 4]>)>,
}

impl TableLine for TerrainTypeConfig {
    type Header = TerrainTypeConfigHeader;
    fn parse(data: &HashMap<Self::Header, &str>, loader: &mut FileLoader) -> Result<Self, Box<dyn Error>> {
        use TerrainTypeConfigHeader as H;
        use ConfigParseError as E;
        let get = |key| {
            data.get(&key).ok_or(E::MissingColumn(format!("{key:?}")))
        };
        let result = Self {
            id: parse(data, H::Id, loader)?,
            name: get(H::Name)?.to_string(),
            needs_owner: parse_def(data, H::NeedsOwner, false, loader)?,
            capture_resistance: parse_def(data, H::CaptureResistance, 0, loader)?,
            update_amphibious: match data.get(&H::UpdateAmphibious) {
                Some(s) if s.len() > 0 => Some(s.parse()?),
                _ => None,
            },
            max_builds_per_turn: parse_def(data, H::MaxBuildsPerTurn, 0, loader)?,
            max_anger: parse_def(data, H::MaxAnger, 0, loader)?,
            vision_range: parse_def(data, H::VisionRange, -1, loader)?,
            income_factor: parse_def(data, H::IncomeFactor, 0, loader)?,
            can_repair: parse_def(data, H::Repair, false, loader)?,
            can_build: parse_def(data, H::Build, false, loader)?,
            can_sell_hero: parse_def(data, H::SellsHero, false, loader)?,
            chess: parse_def(data, H::Chess, false, loader)?,
            extra_movement_options: parse_def(data, H::MovementOptions, ExtraMovementOptions::None, loader)?,
            build_overrides: parse_vec_def(data, H::BuildOverrides, Vec::new(), loader)?.into_iter().collect(),
            #[cfg(feature = "rendering")]
            preview: parse_vec_def(data, H::Preview, Vec::new(), loader)?,
        };
        Ok(result)
    }

    fn simple_validation(&self) -> Result<(), Box<dyn Error>> {
        if self.name.trim().len() == 0 {
            return Err(Box::new(ConfigParseError::NameTooShort));
        }
        // TODO: error if build_overrides overrides its own values
        if self.max_builds_per_turn == 0 && self.can_build {
            // TODO: could remove can_build column
        }
        #[cfg(feature = "rendering")]
        if self.preview.len() == 0 {
            // TODO
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
        Preview,
    }
}
