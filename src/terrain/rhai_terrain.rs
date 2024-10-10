use rhai::*;
use rhai::plugin::*;

use crate::terrain::attributes::TerrainAttributeKey;

#[export_module]
mod terrain_module {
    use crate::units::movement::MovementType;


    pub type TerrainType = crate::terrain::TerrainType;

    #[rhai_fn(pure, name = "==")]
    pub fn tt_eq(p1: &mut TerrainType, p2: TerrainType) -> bool {
        *p1 == p2
    }
    #[rhai_fn(pure, name = "!=")]
    pub fn tt_neq(t1: &mut TerrainType, t2: TerrainType) -> bool {
        *t1 != t2
    }

    pub type Terrain = crate::terrain::terrain::Terrain;

    #[rhai_fn(pure, name = "==")]
    pub fn t_eq(p1: &mut Terrain, p2: Terrain) -> bool {
        *p1 == p2
    }
    #[rhai_fn(pure, name = "!=")]
    pub fn t_neq(t1: &mut Terrain, t2: Terrain) -> bool {
        *t1 != t2
    }

    #[rhai_fn(pure, get = "type")]
    pub fn get_type(terrain: &mut Terrain) -> TerrainType {
        terrain.typ()
    }

    #[rhai_fn(pure, get = "owner_id")]
    pub fn get_owner_id(terrain: &mut Terrain) -> i32 {
        terrain.get_owner_id() as i32
    }

    #[rhai_fn(pure, get = "anger")]
    pub fn get_anger(terrain: &mut Terrain) -> Dynamic {
        if terrain.has_attribute(TerrainAttributeKey::Anger) {
            (terrain.get_anger() as i32).into()
        } else {
            ().into()
        }
    }

    #[rhai_fn(pure, name = "movement_cost")]
    pub fn get_movement_cost(terrain: &mut Terrain, movement_type: MovementType) -> Dynamic {
        terrain.movement_cost(movement_type)
            .map(|mc| Dynamic::from(mc))
            .unwrap_or(().into())
    }
}

def_package! {
    pub TerrainPackage(module)
    {
        combine_with_exported_module!(module, "terrain_module", terrain_module);
    } |> |_engine| {
    }
}
