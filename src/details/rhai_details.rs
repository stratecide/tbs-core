use rhai::*;
use rhai::plugin::*;
use crate::map::direction::*;
use crate::units::unit::*;
use crate::units::unit_types::UnitType;
use crate::script::get_environment;

use super::*;

macro_rules! detail_module {
    ($pack: ident, $name: ident, $d: ty) => {
        #[export_module]
        mod $name {
            pub type Skull = SkullData<$d>;

            #[rhai_fn(pure, get="unit_type")]
            pub fn get_unit_type(skull: &mut Skull) -> UnitType {
                skull.unit_type
            }

            #[rhai_fn(pure, get="owner_id")]
            pub fn get_owner_id(skull: &mut Skull) -> i32 {
                skull.owner.0 as i32
            }

            #[rhai_fn(pure)]
            pub fn build_unit(context: NativeCallContext, skull: &mut Skull) -> UnitBuilder<$d> {
                let environment = get_environment(context);
                let mut builder = skull.unit_type.instance(&environment).set_owner_id(skull.owner.0);
                for attribute in &skull.attributes {
                    builder = builder.set_attribute(attribute);
                }
                builder
            }
        }

        def_package! {
            pub $pack(module)
            {
                combine_with_exported_module!(module, stringify!($name), $name);
            } |> |_engine| {
            }
        }
    };
}

detail_module!(DetailPackage4, detail_module4, Direction4);
detail_module!(DetailPackage6, detail_module6, Direction6);
