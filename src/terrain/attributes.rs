use std::fmt::{Display, Debug};

use rustc_hash::FxHashMap;
use zipper::*;
use serde::Deserialize;

use crate::config::Environment;
use crate::map::direction::{Direction, Direction4, Direction6};
use crate::units::attributes::{DEFAULT_OWNER, Owner};

use super::TerrainType;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize)]
pub(crate) enum TerrainAttributeKey {
    //PipeConnection,
    Owner,
    CaptureProgress,
    BuiltThisTurn,
    Exhausted,
    Anger,
}

impl Display for TerrainAttributeKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", match self {
            //Self::PipeConnection => "PipeConnection",
            Self::Owner => "Owner",
            Self::CaptureProgress => "Capture Progress",
            Self::BuiltThisTurn => "# Built This Turn",
            Self::Exhausted => "Exhausted",
            Self::Anger => "Anger",
        })
    }
}

impl TerrainAttributeKey {
    pub(super) fn default(&self, typ: TerrainType, env: &Environment) -> TerrainAttribute {
        use TerrainAttribute as A;
        match self {
            //Self::PipeConnection => A::PipeConnection(D::angle_0().pipe_entry()),
            Self::Owner => A::Owner(if env.config.terrain_needs_owner(typ) {DEFAULT_OWNER} else {-1}),
            Self::CaptureProgress => A::CaptureProgress(None),
            Self::BuiltThisTurn => A::BuiltThisTurn(0),
            Self::Exhausted => A::Exhausted(false),
            Self::Anger => A::Anger(0),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TerrainAttribute {
    //PipeConnection(D::P),
    Owner(i8),
    CaptureProgress(Option<(i8, u8)>),
    BuiltThisTurn(u8),
    Exhausted(bool),
    Anger(u8),
}

impl TerrainAttribute {
    pub(crate) fn key(&self) -> TerrainAttributeKey {
        use TerrainAttributeKey as A;
        match self {
            //Self::PipeConnection(_) => A::PipeConnection,
            Self::Owner(_) => A::Owner,
            Self::CaptureProgress(_) => A::CaptureProgress,
            Self::BuiltThisTurn(_) => A::BuiltThisTurn,
            Self::Exhausted(_) => A::Exhausted,
            Self::Anger(_) => A::Anger,
        }
    }

    pub(super) fn export(&self, environment: &Environment, zipper: &mut Zipper, typ: TerrainType) {
        match self {
            //Self::PipeConnection(connection) => connection.export(zipper),
            Self::Owner(id) => {
                if environment.config.terrain_needs_owner(typ) {
                    zipper.write_u8(0.max(*id) as u8, bits_needed_for_max_value(environment.config.max_player_count() as u32 - 1));
                } else {
                    zipper.write_u8((*id + 1) as u8, bits_needed_for_max_value(environment.config.max_player_count() as u32));
                }
            }
            Self::CaptureProgress(progress) => {
                zipper.write_bool(progress.is_some());
                if let Some((new_owner, progress)) = progress {
                    if environment.config.terrain_needs_owner(typ) {
                        zipper.write_u8(0.max(*new_owner) as u8, bits_needed_for_max_value(environment.config.max_player_count() as u32 - 1));
                    } else {
                        zipper.write_u8((*new_owner + 1) as u8, bits_needed_for_max_value(environment.config.max_player_count() as u32));
                    }
                    zipper.write_u8(*progress, bits_needed_for_max_value(environment.config.terrain_max_capture_progress(typ) as u32));
                }
            }
            Self::BuiltThisTurn(counter) => zipper.write_u8(*counter, bits_needed_for_max_value(environment.config.terrain_max_builds_per_turn(typ) as u32)),
            Self::Exhausted(z) => zipper.write_bool(*z),
            Self::Anger(counter) => zipper.write_u8(*counter, bits_needed_for_max_value(environment.config.terrain_max_anger(typ) as u32)),
        }
    }

    pub(super) fn import(environment: &Environment, unzipper: &mut Unzipper, key: TerrainAttributeKey, typ: TerrainType) -> Result<Self, ZipperError> {
        use TerrainAttributeKey as A;
        Ok(match key {
            //A::PipeConnection => Self::PipeConnection(<D::P as Zippable>::import(unzipper)?),
            A::Owner => {
                Self::Owner(if environment.config.terrain_needs_owner(typ) {
                    unzipper.read_u8(bits_needed_for_max_value(environment.config.max_player_count() as u32 - 1))? as i8
                } else {
                    unzipper.read_u8(bits_needed_for_max_value(environment.config.max_player_count() as u32))? as i8 - 1
                }.min(environment.config.max_player_count() - 1))
            }
            A::CaptureProgress => {
                Self::CaptureProgress(if unzipper.read_bool()? {
                    let new_owner = if environment.config.terrain_needs_owner(typ) {
                        unzipper.read_u8(bits_needed_for_max_value(environment.config.max_player_count() as u32 - 1))? as i8
                    } else {
                        unzipper.read_u8(bits_needed_for_max_value(environment.config.max_player_count() as u32))? as i8 - 1
                    }.min(environment.config.max_player_count() - 1);
                    let progress = unzipper.read_u8(bits_needed_for_max_value(environment.config.terrain_max_capture_progress(typ) as u32))?;
                    Some((new_owner, progress))
                } else {
                    None
                })
            }
            A::BuiltThisTurn => Self::BuiltThisTurn(unzipper.read_u8(bits_needed_for_max_value(environment.config.terrain_max_builds_per_turn(typ) as u32))?),
            A::Exhausted => Self::Exhausted(unzipper.read_bool()?),
            A::Anger => Self::Anger(unzipper.read_u8(bits_needed_for_max_value(environment.config.terrain_max_anger(typ) as u32))?),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct TerrainAttributeError {
    pub requested: TerrainAttributeKey,
    pub received: Option<TerrainAttributeKey>,
}

pub(crate) trait TrAttribute: TryFrom<TerrainAttribute, Error = TerrainAttributeError> + Into<TerrainAttribute> {
    fn key() -> TerrainAttributeKey;
}

macro_rules! attribute_tuple {
    ($name: ty, $attr: ident) => {
        impl TryFrom<TerrainAttribute> for $name {
            type Error = TerrainAttributeError;
            fn try_from(value: TerrainAttribute) -> Result<Self, Self::Error> {
                if let TerrainAttribute::$attr(value) = value {
                    Ok(Self(value))
                } else {
                    Err(TerrainAttributeError { requested: <Self as TrAttribute>::key(), received: Some(value.key()) })
                }
            }
        }

        impl From<$name> for TerrainAttribute {
            fn from(value: $name) -> Self {
                TerrainAttribute::$attr(value.0)
            }
        }

        impl TrAttribute for $name {
            fn key() -> TerrainAttributeKey {
                TerrainAttributeKey::$attr
            }
        }
    };
}

macro_rules! attribute {
    ($name: ty, $attr: ident) => {
        impl TryFrom<TerrainAttribute> for $name {
            type Error = TerrainAttributeError;
            fn try_from(value: TerrainAttribute) -> Result<Self, Self::Error> {
                if let TerrainAttribute::$attr(value) = value {
                    Ok(value)
                } else {
                    Err(TerrainAttributeError { requested: <Self as TrAttribute>::key(), received: Some(value.key()) })
                }
            }
        }

        impl From<$name> for TerrainAttribute {
            fn from(value: $name) -> Self {
                TerrainAttribute::$attr(value)
            }
        }
        
        impl TrAttribute for $name {
            fn key() -> TerrainAttributeKey {
                TerrainAttributeKey::$attr
            }
        }        
    };
}

attribute_tuple!(Owner, Owner);

attribute!(Option<(i8, u8)>, CaptureProgress);

pub(crate) struct Anger(pub(crate) u8);
attribute_tuple!(Anger, Anger);

pub(crate) struct BuiltThisTurn(pub(crate) u8);
attribute_tuple!(BuiltThisTurn, BuiltThisTurn);
