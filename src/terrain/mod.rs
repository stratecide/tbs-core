pub mod terrain;
pub mod rhai_terrain;
#[cfg(test)]
mod test;

use zipper::*;

use crate::config::environment::Environment;
use crate::config::file_loader::FileLoader;
use crate::config::parse::{string_base, FromConfig};
use crate::config::ConfigParseError;
use crate::map::direction::Direction;

use self::terrain::TerrainBuilder;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerrainType(pub usize);

impl FromConfig for TerrainType {
    fn from_conf<'a>(s: &'a str, loader: &mut crate::config::file_loader::FileLoader) -> Result<(Self, &'a str), crate::config::ConfigParseError> {
        let (base, s) = crate::config::parse::string_base(s);
        match loader.terrain_types.iter().position(|name| name.as_str() == base) {
            Some(i) => Ok((Self(i), s)),
            None => Err(crate::config::ConfigParseError::MissingTerrain(base.to_string()))
        }
    }
}

impl SupportedZippable<&Environment> for TerrainType {
    fn export(&self, zipper: &mut Zipper, environment: &Environment) {
        let bits = bits_needed_for_max_value(environment.config.terrain_count() as u32 - 1);
        zipper.write_u32(self.0 as u32, bits);
    }
    fn import(unzipper: &mut Unzipper, environment: &Environment) -> Result<Self, ZipperError> {
        let bits = bits_needed_for_max_value(environment.config.terrain_count() as u32 - 1);
        let index = unzipper.read_u32(bits)? as usize;
        if index >= environment.config.terrain_count() {
            return Err(ZipperError::EnumOutOfBounds(format!("TerrainType index {}", index)))
        }
        Ok(Self(index))
    }
}

impl TerrainType {
    pub fn instance<D: Direction>(&self, environment: &Environment) -> TerrainBuilder<D> {
        TerrainBuilder::new(environment, *self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtraMovementOptions {
    None,
    Jump,
}

impl FromConfig for ExtraMovementOptions {
    fn from_conf<'a>(s: &'a str, _loader: &mut FileLoader) -> Result<(Self, &'a str), ConfigParseError> {
        let (base, remainder) = string_base(s);
        Ok((match base {
            "None" => Self::None,
            "Jump" => Self::Jump,
            invalid => return Err(ConfigParseError::UnknownEnumMember(invalid.to_string())),
        }, remainder))
    }
}
