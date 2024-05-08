use std::collections::HashMap;
use std::error::Error;

use crate::config::parse::*;
use crate::game::game_view::GameView;
use crate::map::direction::Direction;
use crate::map::point::Point;
use crate::script::custom_action::{CustomAction, CustomActionTestResult};
use crate::units::hero::Hero;
use crate::units::movement::{Path, TBallast};
use crate::units::unit::Unit;

use super::unit_filter::UnitFilter;
use super::ConfigParseError;

#[derive(Debug)]
pub struct CustomActionConfig {
    pub(crate) name: String,                    // displayed in the action menu
    pub(super) unit_filter: Vec<UnitFilter>,
    pub(crate) script: CustomAction,
}

impl CustomActionConfig {
    pub fn parse(data: &HashMap<CustomActionConfigHeader, &str>, load_config: &Box<dyn Fn(&str) -> Result<String, Box<dyn Error>>>) -> Result<Self, ConfigParseError> {
        use CustomActionConfigHeader as H;
        use ConfigParseError as E;
        let get = |key| {
            data.get(&key).ok_or(E::MissingColumn(format!("CustomActionConfig::{key:?}")))
        };
        let result = Self {
            name: get(H::Name)?.to_string(),
            unit_filter: parse_vec_dyn_def(data, H::UnitFilter, Vec::new(), |s| UnitFilter::from_conf(s, load_config))?,
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

    pub fn get_name(&self) -> &str {
        &self.name
    }

    pub fn get_script(&self) -> &CustomAction {
        &self.script
    }

    pub fn add_as_option<D: Direction>(
        &self,
        game: &impl GameView<D>,
        unit: &Unit<D>,
        path: &Path<D>,
        destination: Point,
        funds: i32,
        // when moving out of a transporter, or start_turn for transported units
        transporter: Option<(&Unit<D>, usize)>,
        // the attacked unit, the unit this one was destroyed by, ...
        other_unit: Option<(&Unit<D>, Point)>,
        // the heroes affecting this unit. shouldn't be taken from game since they could have died before this function is called
        heroes: &[(Unit<D>, Hero, Point, Option<usize>)],
        // empty if the unit hasn't moved
        temporary_ballast: &[TBallast<D>],
    ) -> bool {
        if self.unit_filter.iter().all(|f| {
            f.check(game, unit, (destination, None), transporter.map(|(u, _)| (u, path.start)), other_unit, heroes, temporary_ballast, false)
        }) {
            self.script.next_condition(game, funds, unit, path, destination, transporter, heroes, temporary_ballast, &[])
            != CustomActionTestResult::Failure
        } else {
            false
        }
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
