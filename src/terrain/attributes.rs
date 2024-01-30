use std::fmt::{Display, Debug};

use zipper::*;

use crate::config::environment::Environment;
use crate::player::Owner;

use super::TerrainType;

crate::listable_enum! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum TerrainAttributeKey {
        //PipeConnection,
        Owner,
        CaptureProgress,
        BuiltThisTurn,
        Exhausted,
        Anger,
    }
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
    pub fn default(&self) -> TerrainAttribute {
        use TerrainAttribute as A;
        match self {
            //Self::PipeConnection => A::PipeConnection(D::angle_0().pipe_entry()),
            Self::Owner => A::Owner(0),
            Self::CaptureProgress => A::CaptureProgress(None),
            Self::BuiltThisTurn => A::BuiltThisTurn(0),
            Self::Exhausted => A::Exhausted(false),
            Self::Anger => A::Anger(0),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerrainAttribute {
    //PipeConnection(D::P),
    Owner(i8),
    CaptureProgress(CaptureProgress),
    BuiltThisTurn(u8),
    Exhausted(bool),
    Anger(u8),
}

impl TerrainAttribute {
    pub fn key(&self) -> TerrainAttributeKey {
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

    pub(super) fn export(&self, zipper: &mut Zipper, environment: &Environment, typ: TerrainType) {
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
                        zipper.write_u8(0.max(new_owner.0) as u8, bits_needed_for_max_value(environment.config.max_player_count() as u32 - 1));
                    } else {
                        zipper.write_u8((new_owner.0 + 1) as u8, bits_needed_for_max_value(environment.config.max_player_count() as u32));
                    }
                    zipper.write_u8(*progress, bits_needed_for_max_value(environment.config.terrain_capture_resistance(typ) as u32));
                }
            }
            Self::BuiltThisTurn(counter) => zipper.write_u8(*counter, bits_needed_for_max_value(environment.config.terrain_max_builds_per_turn(typ) as u32)),
            Self::Exhausted(z) => zipper.write_bool(*z),
            Self::Anger(counter) => zipper.write_u8(*counter, bits_needed_for_max_value(environment.config.terrain_anger(typ) as u32)),
        }
    }

    pub(super) fn import(unzipper: &mut Unzipper, environment: &Environment, key: TerrainAttributeKey, typ: TerrainType) -> Result<Self, ZipperError> {
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
                    let progress = unzipper.read_u8(bits_needed_for_max_value(environment.config.terrain_capture_resistance(typ) as u32))?;
                    Some((new_owner.into(), progress))
                } else {
                    None
                })
            }
            A::BuiltThisTurn => Self::BuiltThisTurn(unzipper.read_u8(bits_needed_for_max_value(environment.config.terrain_max_builds_per_turn(typ) as u32))?),
            A::Exhausted => Self::Exhausted(unzipper.read_bool()?),
            A::Anger => Self::Anger(unzipper.read_u8(bits_needed_for_max_value(environment.config.terrain_anger(typ) as u32))?),
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

pub type CaptureProgress = Option<(Owner, u8)>;
attribute!(CaptureProgress, CaptureProgress);

impl SupportedZippable<&Environment> for (Owner, u8) {
    fn export(&self, zipper: &mut Zipper, support: &Environment) {
        self.0.export(zipper, support);
        zipper.write_u8(self.1, bits_needed_for_max_value(support.config.max_capture_resistance() as u32));
    }
    fn import(unzipper: &mut Unzipper, support: &Environment) -> Result<Self, ZipperError> {
        Ok((
            Owner::import(unzipper, support)?,
            unzipper.read_u8(bits_needed_for_max_value(support.config.max_capture_resistance() as u32))?,
        ))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Anger(pub u8);
attribute_tuple!(Anger, Anger);

impl SupportedZippable<&Environment> for Anger {
    fn export(&self, zipper: &mut Zipper, support: &Environment) {
        zipper.write_u8(self.0, bits_needed_for_max_value(support.config.terrain_max_anger() as u32));
    }
    fn import(unzipper: &mut Unzipper, support: &Environment) -> Result<Self, ZipperError> {
        Ok(Self(unzipper.read_u8(bits_needed_for_max_value(support.config.terrain_max_anger() as u32))?))
    }
}

impl From<u8> for Anger {
    fn from(value: u8) -> Self {
        Self(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BuiltThisTurn(pub u8);
attribute_tuple!(BuiltThisTurn, BuiltThisTurn);

impl SupportedZippable<&Environment> for BuiltThisTurn {
    fn export(&self, zipper: &mut Zipper, support: &Environment) {
        zipper.write_u8(self.0, bits_needed_for_max_value(support.config.terrain_max_built_this_turn() as u32));
    }
    fn import(unzipper: &mut Unzipper, support: &Environment) -> Result<Self, ZipperError> {
        Ok(Self(unzipper.read_u8(bits_needed_for_max_value(support.config.terrain_max_built_this_turn() as u32))?))
    }
}

impl From<u8> for BuiltThisTurn {
    fn from(value: u8) -> Self {
        Self(value)
    }
}
