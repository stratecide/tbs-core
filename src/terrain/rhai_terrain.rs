use rhai::*;
use rhai::plugin::*;

use crate::config::environment::Environment;
use crate::map::direction::*;
use crate::units::movement::MovementType;
use crate::terrain::TerrainType;
use crate::tags::*;
use crate::config::OwnershipPredicate;

#[export_module]
mod terrain_type_module {
    pub type TerrainType = crate::terrain::TerrainType;

    #[rhai_fn(pure, name = "TerrainType")]
    pub fn new_terrain_type(environment: &mut Environment, name: &str) -> Dynamic {
        environment.config.find_terrain_by_name(name)
        .map(Dynamic::from)
        .unwrap_or(().into())
    }

    #[rhai_fn(pure, name = "==")]
    pub fn tt_eq(t1: &mut TerrainType, t2: TerrainType) -> bool {
        *t1 == t2
    }
    #[rhai_fn(pure, name = "!=")]
    pub fn tt_neq(t1: &mut TerrainType, t2: TerrainType) -> bool {
        *t1 != t2
    }
}

macro_rules! board_module {
    ($pack: ident, $name: ident, $d: ty) => {
        #[export_module]
        mod $name {
            pub type Terrain = crate::terrain::terrain::Terrain<$d>;

            #[rhai_fn(pure, name = "==")]
            pub fn t_eq(t1: &mut Terrain, t2: Terrain) -> bool {
                *t1 == t2
            }
            #[rhai_fn(pure, name = "!=")]
            pub fn t_neq(t1: &mut Terrain, t2: Terrain) -> bool {
                *t1 != t2
            }

            #[rhai_fn(pure, name = "Terrain")]
            pub fn new_terrain(environment: &mut Environment, typ: TerrainType) -> Terrain {
                Terrain::new(environment.clone(), typ)
            }

            #[rhai_fn(pure, get = "type")]
            pub fn get_type(terrain: &mut Terrain) -> TerrainType {
                terrain.typ()
            }

            #[rhai_fn(pure, get = "owner_id")]
            pub fn get_owner_id(terrain: &mut Terrain) -> Dynamic {
                if terrain.environment().config.terrain_ownership(terrain.typ()) == OwnershipPredicate::Never {
                    return ().into()
                }
                Dynamic::from(terrain.get_owner_id() as i32)
            }
            #[rhai_fn(set = "owner_id")]
            pub fn set_owner_id(terrain: &mut Terrain, owner_id: i32) {
                terrain.set_owner_id(owner_id.max(-1).min(terrain.environment().config.max_player_count() as i32) as i8)
            }

            #[rhai_fn(pure, get = "team")]
            pub fn get_team(terrain: &mut Terrain) -> i32 {
                terrain.get_team().to_i16() as i32
            }

            pub fn copy_from(terrain: &mut Terrain, other: Terrain) {
                terrain.copy_from(&other);
            }

            #[rhai_fn(pure, name = "has")]
            pub fn has_flag(terrain: &mut Terrain, flag: FlagKey) -> bool {
                terrain.has_flag(flag.0)
            }
            #[rhai_fn(name = "set")]
            pub fn set_flag(terrain: &mut Terrain, flag: FlagKey) {
                terrain.set_flag(flag.0)
            }
            #[rhai_fn(name = "remove")]
            pub fn remove_flag(terrain: &mut Terrain, flag: FlagKey) {
                terrain.remove_flag(flag.0)
            }

            #[rhai_fn(pure, name = "has")]
            pub fn has_tag(terrain: &mut Terrain, tag: TagKey) -> bool {
                terrain.get_tag(tag.0).is_some()
            }
            #[rhai_fn(pure, name = "get")]
            pub fn get_tag(terrain: &mut Terrain, key: TagKey) -> Dynamic {
                terrain.get_tag(key.0).map(|v| v.into_dynamic()).unwrap_or(().into())
            }
            #[rhai_fn(name = "set")]
            pub fn set_tag(terrain: &mut Terrain, key: TagKey, value: Dynamic) {
                if let Some(value) = TagValue::from_dynamic(value, key.0, terrain.environment()) {
                    terrain.set_tag(key.0, value);
                }
            }
            #[rhai_fn(name = "remove")]
            pub fn remove_tag(terrain: &mut Terrain, tag: TagKey) {
                terrain.remove_tag(tag.0)
            }

            #[rhai_fn(pure, name = "movement_cost")]
            pub fn get_movement_cost(terrain: &mut Terrain, movement_type: MovementType) -> Dynamic {
                terrain.movement_cost(movement_type)
                    .map(|mc| Dynamic::from(mc))
                    .unwrap_or(().into())
            }

        }

        def_package! {
            pub $pack(module)
            {
                combine_with_exported_module!(module, "terrain_type_module", terrain_type_module);
                combine_with_exported_module!(module, stringify!($name), $name);
            } |> |_engine| {
            }
        }
    };
}

board_module!(TerrainPackage4, terrain_module4, Direction4);
board_module!(TerrainPackage6, terrain_module6, Direction6);
