use crate::config::parse::{parse_tuple1, string_base, FromConfig};
use crate::config::ConfigParseError;
use crate::game::event_handler::EventHandler;
use crate::map::direction::Direction;
use crate::map::point::Point;
use crate::terrain::terrain::Terrain;
use crate::terrain::*;

#[derive(Debug, Clone)]
pub enum TerrainScript {
    Replace(TerrainType),
    SetOwner(i8),
}

impl FromConfig for TerrainScript {
    fn from_conf(s: &str) -> Result<(Self, &str), ConfigParseError> {
        let (base, mut s) = string_base(s);
        Ok((match base {
            "Replace" => {
                let (terrain_type, r) = parse_tuple1(s)?;
                s = r;
                Self::Replace(terrain_type)
            }
            "SetOwner" => {
                let (owner_id, r) = parse_tuple1(s)?;
                s = r;
                Self::SetOwner(owner_id)
            }
            invalid => return Err(ConfigParseError::UnknownEnumMember(format!("TerrainScript::{}", invalid))),
        }, s))
    }
}

impl TerrainScript {
    pub fn trigger<D: Direction>(&self, handler: &mut EventHandler<D>, position: Point, terrain: &Terrain) {
        match self {
            Self::Replace(terrain_type) => {
                let new_terrain = terrain_type.instance(handler.environment()).copy_from(terrain).build_with_defaults();
                handler.terrain_replace(position, new_terrain);
            },
            Self::SetOwner(owner_id) => {
                let new_terrain = terrain.typ().instance(handler.environment()).copy_from(terrain).set_owner_id(*owner_id).build_with_defaults();
                handler.terrain_replace(position, new_terrain);
            }
        }
    }
}
