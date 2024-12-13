use std::error::Error;

use num_rational::Rational32;
use rustc_hash::FxHashMap as HashMap;

use crate::config::parse::*;
use crate::terrain::*;

use super::file_loader::{FileLoader, TableLine};
use super::{ConfigParseError, OwnershipPredicate};

#[derive(Debug)]
pub struct TerrainTypeConfig {
    pub(super) name: String,
    pub(super) owned: OwnershipPredicate,
    pub(super) owner_is_playable: bool,
    pub(super) chess: bool,
    // can be modified by commander / hero aura
    pub(super) income_factor: Rational32,
    pub(super) vision_range: i8,
    pub(super) extra_movement_options: ExtraMovementOptions,
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
            name: get(H::Id)?.to_string(),
            owned: parse_def(data, H::Owned, OwnershipPredicate::Either, loader)?,
            owner_is_playable: parse_def(data, H::OwnerPlayable, false, loader)?,
            vision_range: parse_def(data, H::VisionRange, -1, loader)?,
            income_factor: parse_def(data, H::IncomeFactor, Rational32::from_integer(0), loader)?,
            chess: parse_def(data, H::Chess, false, loader)?,
            extra_movement_options: parse_def(data, H::MovementOptions, ExtraMovementOptions::None, loader)?,
            #[cfg(feature = "rendering")]
            preview: parse_vec_def(data, H::Preview, Vec::new(), loader)?,
        };
        Ok(result)
    }

    fn simple_validation(&self) -> Result<(), Box<dyn Error>> {
        if self.name.trim().len() == 0 {
            return Err(Box::new(ConfigParseError::NameTooShort));
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
        Owned,
        OwnerPlayable,
        VisionRange,
        IncomeFactor,
        Chess,
        MovementOptions,
        Preview,
    }
}
