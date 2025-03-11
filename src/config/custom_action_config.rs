use rustc_hash::FxHashMap as HashMap;
use std::error::Error;

use crate::config::parse::*;
use crate::script::custom_action::CustomAction;

use super::file_loader::{FileLoader, TableLine};
use super::unit_filter::UnitFilter;
use super::ConfigParseError;

#[derive(Debug)]
pub struct CustomActionConfig {
    pub(crate) name: String,                    // displayed in the action menu
    pub(crate) unit_filter: Vec<UnitFilter>,
    pub(crate) script: CustomAction,
}

impl TableLine for CustomActionConfig {
    type Header = CustomActionConfigHeader;
    fn parse(data: &HashMap<Self::Header, &str>, loader: &mut FileLoader) -> Result<Self, Box<dyn Error>> {
        use CustomActionConfigHeader as H;
        use ConfigParseError as E;
        let get = |key| {
            data.get(&key).ok_or(E::MissingColumn(format!("CustomActionConfig::{key:?}")))
        };
        let name = get(H::Name)?.to_string();
        let script = match data.get(&H::Script) {
            Some(s) if s.len() > 0 => {
                let exe = loader.rhai_function(s, 0..=1)?;
                let input = if exe.parameters.len() > 0 {
                    Some(loader.rhai_function(&format!("{s}_input"), 0..=0)?.index)
                } else {
                    None
                };
                Ok((input, exe.index))
            }
            _ => Err(ConfigParseError::CustomActionScriptMissing(name.clone())),
        }?;
        let result = Self {
            unit_filter: parse_vec_dyn_def(data, H::UnitFilter, Vec::new(), |s| UnitFilter::from_conf(s, loader))?,
            script,
            name,
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

impl CustomActionConfig {
    pub fn get_name(&self) -> &str {
        &self.name
    }

    pub fn get_script(&self) -> CustomAction {
        self.script
    }
}

crate::listable_enum! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum CustomActionConfigHeader {
        Name,
        UnitFilter,
        Script,
    }
}
