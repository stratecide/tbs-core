use std::error::Error;

use interfaces::ClientPerspective;
use rhai::Scope;
use rustc_hash::FxHashMap as HashMap;

use crate::config::parse::*;
use crate::game::event_fx::EffectWithoutPosition;
use crate::game::fog::{is_unit_visible, visible_unit_with_attribute, FogIntensity};
use crate::game::game::Game;
use crate::game::game_view::GameView;
use crate::handle::Handle;
use crate::map::direction::Direction;
use crate::map::point::Point;
use crate::script::executor::Executor;
use crate::script::*;
use crate::tags::{FlagKey, TagKey};

use super::file_loader::{FileLoader, TableLine};
use super::ConfigParseError;

#[derive(Debug)]
pub struct EffectConfig {
    pub name: String,
    pub is_global: bool,
    pub data_type: Option<EffectDataType>,
    pub visibility: EffectVisibility,
}

impl TableLine for EffectConfig {
    type Header = EffectConfigHeader;
    fn parse(data: &HashMap<Self::Header, &str>, loader: &mut FileLoader) -> Result<Self, Box<dyn Error>> {
        use EffectConfigHeader as H;
        use ConfigParseError as E;
        let get = |key| {
            data.get(&key).ok_or(E::MissingColumn(format!("{key:?}")))
        };
        let name = get(H::Id)?.trim().to_string();
        let (data_type, s) = string_base(get(H::DataType)?.trim());
        let data_type = match data_type.to_lowercase().as_str() {
            "" => None,
            "int" => {
                let (min, max, _) = parse_tuple2(s, loader)?;
                Some(EffectDataType::Int {
                    min,
                    max,
                })
            },
            "direction" => Some(EffectDataType::Direction),
            "terrain" => Some(EffectDataType::Terrain),
            "token" => Some(EffectDataType::Token),
            "unit" => Some(EffectDataType::Unit),
            "visibility" => Some(EffectDataType::Visibility),
            "team" => Some(EffectDataType::Team),
            unknown => return Err(ConfigParseError::UnknownEnumMember(format!("EffectDataType::{unknown}")).into())
        };
        Ok(Self {
            name,
            is_global: parse_def(data, H::Global, false, loader)?,
            data_type,
            visibility: parse(data, H::Visibility, loader)?,
        })
    }

    fn simple_validation(&self) -> Result<(), Box<dyn Error>> {
        if self.name.len() == 0 {
            return Err(Box::new(ConfigParseError::NameTooShort));
        }
        if !self.name.chars().all(|c| c == '_' || c.is_ascii_alphanumeric()) {
            return Err(Box::new(ConfigParseError::Other(format!("Effect ({}): Name can only contain ASCII letters, digits and '_'", self.name))));
        }
        if self.visibility == EffectVisibility::Data {
            // TODO: check if self.data_type can be used to check visibility
        }
        match self.data_type {
            Some(EffectDataType::Int { min, max }) => {
                if min >= max {
                    return Err(Box::new(ConfigParseError::Other(format!("Effect DataType {}'s minimum needs to be lower than maximum", self.name))));
                }
            }
            _ => ()
        }
        Ok(())
    }
}

crate::listable_enum! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum EffectConfigHeader {
        Id,
        Global,
        DataType,
        Visibility,
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum EffectDataType {
    Int{
        min: i32,
        max: i32,
    },
    Direction,
    Terrain,
    Token,
    Unit,
    Visibility,
    Team,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EffectVisibility {
    Rhai(usize),
    Full,
    CurrentTeam,
    Data,
    Unit,
    UnitFlag(FlagKey),
    UnitTag(TagKey),
    Fog(FogIntensity),
}

impl FromConfig for EffectVisibility {
    fn from_conf<'a>(s: &'a str, loader: &mut FileLoader) -> Result<(Self, &'a str), ConfigParseError> {
        let (base, mut remainder) = string_base(s);
        Ok((match base {
            "Rhai" | "Script" => {
                let (name, r) = parse_tuple1::<String>(remainder, loader)?;
                remainder = r;
                Self::Rhai(loader.rhai_function(&name, 0..=0)?.index)
            }
            "Full" => Self::Full,
            "CurrentTeam" => Self::CurrentTeam,
            "Data" => Self::Data,
            "Unit" => Self::Unit,
            "UnitFlag" => {
                let (key, r) = parse_tuple1::<FlagKey>(remainder, loader)?;
                remainder = r;
                Self::UnitFlag(key)
            }
            "UnitTag" => {
                let (key, r) = parse_tuple1::<TagKey>(remainder, loader)?;
                remainder = r;
                Self::UnitTag(key)
            }
            "Fog" => {
                let (key, r) = parse_tuple1::<FogIntensity>(remainder, loader)?;
                remainder = r;
                Self::Fog(key)
            }
            invalid => return Err(ConfigParseError::UnknownEnumMember(invalid.to_string())),
        }, remainder))
    }
}

impl EffectVisibility {
    pub fn fog_replacement<D: Direction>(&self, effect: &EffectWithoutPosition<D>, start: Option<Point>, p: Option<Point>, game: &Handle<Game<D>>, team: ClientPerspective) -> Option<EffectWithoutPosition<D>> {
        match self {
            Self::Rhai(function_index) => {
                let mut scope = Scope::new();
                if let Some(start) = start {
                    scope.push_constant(CONST_NAME_STARTING_POSITION, start);
                }
                if let Some(p) = p {
                    scope.push_constant(CONST_NAME_POSITION, p);
                }
                let engine = game.environment().get_engine_board(game);
                let executor = Executor::new(engine, scope, game.environment());
                match executor.run(*function_index, ()) {
                    Ok(result) => result,
                    Err(e) => {
                        // TODO: log error
                        println!("EffectVisibility error: {e:?}");
                        // TODO: return "glitch" effect instead
                        None
                    }
                }
            }
            Self::Full => Some(effect.clone()),
            Self::CurrentTeam => {
                if game.current_team() == team {
                    Some(effect.clone())
                } else {
                    None
                }
            }
            Self::Data => {
                let fog_intensity = game.get_fog_at(team, p?);
                let data = effect.data.fog_replacement(game, p?, fog_intensity, team)?;
                Some(EffectWithoutPosition {
                    typ: effect.typ,
                    data,
                })
            }
            Self::Unit => {
                let unit = game.get_unit(start?)?;
                if is_unit_visible(game, &unit, p?, team) {
                    Some(effect.clone())
                } else {
                    None
                }
            }
            Self::UnitFlag(key) => {
                let unit = game.get_unit(start?)?;
                if visible_unit_with_attribute(game, team, p?, unit.environment().config.flag_visibility(key.0)) {
                    Some(effect.clone())
                } else {
                    None
                }
            }
            Self::UnitTag(key) => {
                let unit = game.get_unit(start?)?;
                if visible_unit_with_attribute(game, team, p?, unit.environment().config.tag_visibility(key.0)) {
                    Some(effect.clone())
                } else {
                    None
                }
            }
            Self::Fog(fog_intensity) => {
                if game.get_fog_at(team, p?) <= *fog_intensity {
                    Some(effect.clone())
                } else {
                    None
                }
            }
        }
    }
}
