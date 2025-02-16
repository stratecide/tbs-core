use rustc_hash::FxHashMap as HashMap;
use std::error::Error;

use crate::config::parse::*;
use crate::game::game_view::GameView;
use crate::map::direction::Direction;
use crate::map::point::Point;
use crate::script::custom_action::CustomAction;
use crate::script::executor::Executor;
use crate::units::hero::HeroInfluence;
use crate::units::movement::{Path, TBallast};
use crate::units::unit::Unit;

use super::file_loader::{FileLoader, TableLine};
use super::unit_filter::UnitFilter;
use super::ConfigParseError;

#[derive(Debug)]
pub struct CustomActionConfig {
    pub(crate) name: String,                    // displayed in the action menu
    pub(super) unit_filter: Vec<UnitFilter>,
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

    pub fn add_as_option<D: Direction>(
        &self,
        game: &impl GameView<D>,
        unit: &Unit<D>,
        path: &Path<D>,
        destination: Point,
        _funds: i32,
        // when moving out of a transporter, or start_turn for transported units
        transporter: Option<(&Unit<D>, usize)>,
        // the attacked unit, the unit this one was destroyed by, ...
        other_unit: Option<(&Unit<D>, Point, Option<usize>, &[HeroInfluence<D>])>,
        // the heroes affecting this unit. shouldn't be taken from game since they could have died before this function is called
        heroes: &[HeroInfluence<D>],
        // empty if the unit hasn't moved
        temporary_ballast: &[TBallast<D>],
        executor: &Executor,
    ) -> bool {
        self.unit_filter.iter().all(|f| {
            f.check(game, unit, (destination, None), transporter.map(|(u, _)| (u, path.start)), other_unit, heroes, temporary_ballast, false, executor)
        })
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
