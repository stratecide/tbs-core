pub mod attributes;
pub mod terrain;
mod test;

use std::str::FromStr;
use zipper::*;

use crate::config::environment::Environment;
use crate::config::parse::{string_base, FromConfig};
use crate::config::ConfigParseError;

use self::terrain::TerrainBuilder;

pub const KRAKEN_ATTACK_RANGE: usize = 3;
pub const KRAKEN_MAX_ANGER: usize = 8;


crate::enum_with_custom! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum TerrainType {
        Airport,
        Beach,
        Bridge,
        ChessPawnTile,
        ChessTile,
        City,
        ConstructionSite,
        Factory,
        Flame,
        Forest,
        Fountain,
        Grass,
        Hill,
        Hq,
        Icebergs,
        Kraken,
        Lillypads,
        Mountain,
        OilPlatform,
        //Pipe,
        Reef,
        Ruins,
        Sea,
        ShallowSea,
        Street,
        StatueLand,
        StatueSea,
        //TrashIsland,
        //Crater,
        TentacleDepths,
        Port,
        // CO-specific terrain
        FairyForest,
    }
}

impl SupportedZippable<&Environment> for TerrainType {
    fn export(&self, zipper: &mut Zipper, support: &Environment) {
        let index = support.config.terrain_types().iter().position(|t| t == self).unwrap();
        let bits = bits_needed_for_max_value(support.config.terrain_count() as u32 - 1);
        zipper.write_u32(index as u32, bits);
    }
    fn import(unzipper: &mut Unzipper, support: &Environment) -> Result<Self, ZipperError> {
        let bits = bits_needed_for_max_value(support.config.terrain_count() as u32 - 1);
        let index = unzipper.read_u32(bits)? as usize;
        if index < support.config.terrain_count() {
            Ok(support.config.terrain_types()[index])
        } else {
            Err(ZipperError::EnumOutOfBounds(format!("TerrainType index {}", index)))
        }
    }
}

impl TerrainType {
    pub fn instance(&self, environment: &Environment) -> TerrainBuilder {
        TerrainBuilder::new(environment, *self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtraMovementOptions {
    None,
    Jump,
    //PawnStart,
}

impl FromConfig for ExtraMovementOptions {
    fn from_conf(s: &str) -> Result<(Self, &str), ConfigParseError> {
        let (base, remainder) = string_base(s);
        Ok((match base {
            "None" => Self::None,
            "Jump" => Self::Jump,
            //"PawnStart" => Self::PawnStart,
            invalid => return Err(ConfigParseError::UnknownEnumMember(invalid.to_string())),
        }, remainder))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AmphibiousTyping {
    Land,
    Sea,
    Beach,
}

impl FromStr for AmphibiousTyping {
    type Err = ConfigParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut it = s.split(&['(', ',', '-', ')'])
        .map(str::trim);
        Ok(match it.next().unwrap() {
            "Land" => Self::Land,
            "Sea" => Self::Sea,
            "Beach" => Self::Beach,
            invalid => return Err(ConfigParseError::UnknownEnumMember(invalid.to_string())),
        })
    }
}
