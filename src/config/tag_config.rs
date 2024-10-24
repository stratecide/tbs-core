use std::error::Error;

use rustc_hash::FxHashMap as HashMap;

use crate::config::parse::*;
use crate::units::UnitVisibility;

use super::file_loader::{FileLoader, TableLine};
use super::ConfigParseError;

#[derive(Debug)]
pub struct TagConfig {
    pub(super) name: String,
    pub(super) visibility: UnitVisibility,
    pub(super) tag_type: TagType,
}

impl TableLine for TagConfig {
    type Header = TagConfigHeader;
    fn parse(data: &HashMap<Self::Header, &str>, loader: &mut FileLoader) -> Result<Self, Box<dyn Error>> {
        use TagConfigHeader as H;
        use ConfigParseError as E;
        let get = |key| {
            data.get(&key).ok_or(E::MissingColumn(format!("{key:?}")))
        };
        let name = get(H::Id)?.trim().to_string();
        let tag_type = match get(H::Type)?.trim().to_lowercase().as_str() {
            "flag" => TagType::Flag,
            "int" => TagType::Int {
                min: parse_def(data, H::MinValue, 0, loader)?,
                max: parse_def(data, H::MaxValue, 0, loader)?,
            },
            "unique" => TagType::Unique {
                pool: parse_def(data, H::UniqueWith, name.clone(), loader)?,
            },
            "point" => TagType::Point,
            "direction" => TagType::Direction,
            "terrain_type" => TagType::TerrainType,
            "unit_type" => TagType::UnitType,
            unknown => return Err(ConfigParseError::UnknownEnumMember(format!("TagType::{unknown}")).into())
        };
        Ok(Self {
            name,
            visibility: parse_def(data, H::Visibility, UnitVisibility::Normal, loader)?,
            tag_type,
        })
    }

    fn simple_validation(&self) -> Result<(), Box<dyn Error>> {
        if self.name.len() == 0 {
            return Err(Box::new(ConfigParseError::NameTooShort));
        }
        match self.tag_type {
            TagType::Int { min, max } => {
                if min >= max {
                    return Err(Box::new(ConfigParseError::Other(format!("TagType {}'s minimum needs to be lower than maximum", self.name))));
                }
            }
            _ => ()
        }
        Ok(())
    }
}

crate::listable_enum! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum TagConfigHeader {
        Id,
        Visibility,
        Type,
        MinValue,
        MaxValue,
        UniqueWith,
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum TagType {
    Flag,
    Int{
        min: i32,
        max: i32,
    },
    Unique {
        pool: String,
    },
    Point,
    Direction,
    TerrainType,
    UnitType,
}
