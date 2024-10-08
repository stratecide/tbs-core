use rhai::*;
use rhai::plugin::*;

#[export_module]
mod terrain_module {
    pub type TerrainType = crate::terrain::TerrainType;

    #[rhai_fn(pure, name = "==")]
    pub fn tt_eq(p1: &mut TerrainType, p2: TerrainType) -> bool {
        *p1 == p2
    }

    pub type Terrain = crate::terrain::terrain::Terrain;

    #[rhai_fn(pure, name = "==")]
    pub fn t_eq(p1: &mut Terrain, p2: Terrain) -> bool {
        *p1 == p2
    }

    #[rhai_fn(pure, get = "type")]
    pub fn get_type(terrain: &mut Terrain) -> TerrainType {
        terrain.typ()
    }

    #[rhai_fn(pure, get = "owner_id")]
    pub fn get_owner_id(terrain: &mut Terrain) -> i32 {
        terrain.get_owner_id() as i32
    }
}

def_package! {
    pub TerrainPackage(module)
    {
        combine_with_exported_module!(module, "terrain_module", terrain_module);
    } |> |_engine| {
    }
}
