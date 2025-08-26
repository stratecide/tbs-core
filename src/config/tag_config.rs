use std::error::Error;

use rustc_hash::FxHashMap as HashMap;

use crate::config::editor_tag_config::TagEditorVisibility;
use crate::config::parse::*;
use crate::units::UnitVisibility;

use super::file_loader::{FileLoader, TableLine};
use super::ConfigParseError;

#[derive(Debug)]
pub struct TagConfig {
    pub(super) name: String,
    pub(super) visibility: UnitVisibility,
    pub(super) tag_type: TagType,
    pub global: TagEditorVisibility,
    pub player: TagEditorVisibility,
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
            "int" => {
                let min = parse_def(data, H::MinValue, 0, loader)?;
                let max = parse_def(data, H::MaxValue, 0, loader)?;
                let default = parse_def(data, H::DefaultValue, 0, loader)?;
                TagType::Int {
                    min,
                    max,
                    default: default.max(min).min(max),
                }
            }
            "unique" => TagType::Unique {
                pool: parse_def(data, H::UniqueWith, name.clone(), loader)?,
            },
            "point" => TagType::Point,
            "direction" => TagType::Direction,
            "terrain_type" | "terraintype" => TagType::TerrainType,
            "unit_type" | "unittype" => TagType::UnitType,
            "movement_type" | "movementtype" => TagType::MovementType,
            unknown => return Err(ConfigParseError::UnknownEnumMember(format!("TagType::{unknown}")).into())
        };
        Ok(Self {
            name,
            visibility: parse_def(data, H::Visibility, UnitVisibility::Normal, loader)?,
            tag_type,
            global: parse_def(data, H::Global, TagEditorVisibility::Hidden, loader)?,
            player: parse_def(data, H::Player, TagEditorVisibility::Hidden, loader)?,
        })
    }

    fn simple_validation(&self) -> Result<(), Box<dyn Error>> {
        if self.name.len() == 0 {
            return Err(Box::new(ConfigParseError::NameTooShort));
        }
        match self.tag_type {
            TagType::Int { min, max, .. } => {
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
        DefaultValue,
        UniqueWith,
        Global,
        Player,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TagType {
    Flag,
    Int{
        min: i32,
        max: i32,
        default: i32,
    },
    Unique {
        pool: String,
    },
    Point,
    Direction,
    TerrainType,
    UnitType,
    MovementType,
}
