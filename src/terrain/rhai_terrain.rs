use rhai::*;
use rhai::plugin::*;

use crate::config::environment::Environment;
use crate::map::direction::*;
use crate::script::get_environment;
use crate::units::movement::MovementType;
use crate::terrain::TerrainType;

#[export_module]
mod terrain_type_module {
    pub type TerrainType = crate::terrain::TerrainType;

    #[rhai_fn(pure, name = "==")]
    pub fn tt_eq(t1: &mut TerrainType, t2: TerrainType) -> bool {
        *t1 == t2
    }
    #[rhai_fn(pure, name = "!=")]
    pub fn tt_neq(t1: &mut TerrainType, t2: TerrainType) -> bool {
        *t1 != t2
    }
    #[rhai_fn(pure, name = "==")]
    pub fn eq_tt_s(context: NativeCallContext, t1: &mut TerrainType, t2: &str) -> bool {
        let environment = get_environment(context);
        Some(*t1) == environment.config.find_terrain_by_name(t2)
    }
    #[rhai_fn(pure, name = "!=")]
    pub fn neq_tt_s(context: NativeCallContext, t1: &mut TerrainType, t2: &str) -> bool {
        let environment = get_environment(context);
        Some(*t1) != environment.config.find_terrain_by_name(t2)
    }
    #[rhai_fn(name = "==")]
    pub fn eq_s_tt(context: NativeCallContext, t1: &str, t2: TerrainType) -> bool {
        let environment = get_environment(context);
        environment.config.find_terrain_by_name(t1) == Some(t2)
    }
    #[rhai_fn(name = "!=")]
    pub fn neq_s_tt(context: NativeCallContext, t1: &str, t2: TerrainType) -> bool {
        let environment = get_environment(context);
        environment.config.find_terrain_by_name(t1) != Some(t2)
    }
}

macro_rules! board_module {
    ($pack: ident, $name: ident, $d: ty) => {
        #[export_module]
        mod $name {
            pub type Terrain = crate::terrain::terrain::Terrain<$d>;
            pub type TerrainBuilder = crate::terrain::terrain::TerrainBuilder<$d>;

            #[rhai_fn(pure, name = "==")]
            pub fn t_eq(t1: &mut Terrain, t2: Terrain) -> bool {
                *t1 == t2
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

            /*#[rhai_fn(pure, get = "anger")]
            pub fn get_anger(terrain: &mut Terrain) -> Dynamic {
                if terrain.has_attribute(TerrainAttributeKey::Anger) {
                    (terrain.get_anger() as i32).into()
                } else {
                    ().into()
                }
            }*/

            #[rhai_fn(pure, name = "movement_cost")]
            pub fn get_movement_cost(terrain: &mut Terrain, movement_type: MovementType) -> Dynamic {
                terrain.movement_cost(movement_type)
                    .map(|mc| Dynamic::from(mc))
                    .unwrap_or(().into())
            }

            #[rhai_fn(pure, name="build_terrain")]
            pub fn build_terrain(environment: &mut Environment, terrain_type: TerrainType) -> TerrainBuilder {
                terrain_type.instance(environment)
            }
            #[rhai_fn(return_raw, pure, name="build_terrain")]
            pub fn build_terrain2(environment: &mut Environment, terrain_type: &str) -> Result<TerrainBuilder, Box<EvalAltResult>> {
                if let Some(terrain_type) = environment.config.find_terrain_by_name(terrain_type) {
                    Ok(build_terrain(environment, terrain_type))
                } else {
                    Err(format!("Unknown terrain type '{terrain_type}'").into())
                }
            }

            #[rhai_fn(name = "copy_from")]
            pub fn builder_copy_from(builder: TerrainBuilder, terrain: Terrain) -> TerrainBuilder {
                builder.copy_from(&terrain)
            }

            #[rhai_fn(name = "owner_id")]
            pub fn builder_owner_id(builder: TerrainBuilder, owner_id: i32) -> TerrainBuilder {
                builder.set_owner_id(owner_id.max(-1).min(i8::MAX as i32) as i8)
            }

            #[rhai_fn(name = "build")]
            pub fn builder_build(builder: TerrainBuilder) -> Terrain {
                builder.build_with_defaults()
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
