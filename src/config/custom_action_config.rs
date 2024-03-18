use std::collections::HashMap;

use crate::config::parse::*;
use crate::script::custom_action::CustomAction;

use super::unit_filter::UnitFilter;
use super::ConfigParseError;

#[derive(Debug)]
pub struct CustomActionConfig {
    pub(crate) name: String,                    // displayed in the action menu
    pub(super) unit_filter: Vec<UnitFilter>,
    pub(crate) script: CustomAction,
}

impl CustomActionConfig {
    pub fn parse(data: &HashMap<CustomActionConfigHeader, &str>) -> Result<Self, ConfigParseError> {
        use CustomActionConfigHeader as H;
        use ConfigParseError as E;
        let get = |key| {
            data.get(&key).ok_or(E::MissingColumn(format!("{key:?}")))
        };
        let result = Self {
            name: get(H::Name)?.to_string(),
            unit_filter: parse_vec_def(data, H::UnitFilter, Vec::new())?,
            script: parse(data, H::Script)?,
        };
        result.simple_validation()?;
        Ok(result)
    }

    pub fn simple_validation(&self) -> Result<(), ConfigParseError> {
        if self.name.trim().len() == 0 {
            return Err(ConfigParseError::NameTooShort);
        }
        Ok(())
    }

    pub fn get_script(&self) -> &CustomAction {
        &self.script
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
