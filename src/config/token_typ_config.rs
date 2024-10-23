use std::error::Error;

use rustc_hash::FxHashMap as HashMap;

use crate::config::parse::*;
use crate::units::UnitVisibility;

use super::file_loader::{FileLoader, TableLine};
use super::ConfigParseError;

#[derive(Debug)]
pub struct TokenTypeConfig {
    pub(super) name: String,
    pub(super) can_have_owner: bool,
    pub(super) owner_is_playable: bool,
    pub(super) visibility: UnitVisibility,
    pub(super) vision_range: i8,
    pub(super) action_script: Option<(usize, usize)>,
    pub(super) on_unit_path: Option<usize>,
}

impl TableLine for TokenTypeConfig {
    type Header = TokenTypeConfigHeader;
    fn parse(data: &HashMap<Self::Header, &str>, loader: &mut FileLoader) -> Result<Self, Box<dyn Error>> {
        use TokenTypeConfigHeader as H;
        use ConfigParseError as E;
        let get = |key| {
            data.get(&key).ok_or(E::MissingColumn(format!("{key:?}")))
        };
        let result = Self {
            name: get(H::Id)?.to_string(),
            can_have_owner: parse_def(data, H::Ownable, false, loader)?,
            owner_is_playable: parse_def(data, H::OwnerPlayable, false, loader)?,
            visibility: match data.get(&H::Visibility) {
                Some(s) => UnitVisibility::from_conf(s, loader)?.0,
                None => UnitVisibility::Normal,
            },
            vision_range: parse_def(data, H::VisionRange, -1, loader)?,
            action_script: match data.get(&H::ActionScript) {
                Some(s) if s.len() > 0 => {
                    let exe = loader.rhai_function(s, 1..=1)?;
                    let input = loader.rhai_function(&format!("{s}_input"), 0..=0)?.index;
                    Some((input, exe.index))
                }
                _ => None,
            },
            on_unit_path: match data.get(&H::OnUnitPath) {
                Some(s) if s.len() > 0 => Some(loader.rhai_function(s, 2..=2)?.index),
                _ => None,
            },
        };
        Ok(result)
    }

    fn simple_validation(&self) -> Result<(), Box<dyn Error>> {
        if self.name.trim().len() == 0 {
            return Err(Box::new(ConfigParseError::NameTooShort));
        }
        Ok(())
    }
}

crate::listable_enum! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum TokenTypeConfigHeader {
        Id,
        Ownable,
        OwnerPlayable,
        Visibility,
        VisionRange,
        ActionScript,
        OnUnitPath,
    }
}
