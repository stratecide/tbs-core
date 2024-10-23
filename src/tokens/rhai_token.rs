use rhai::*;
use rhai::plugin::*;
use crate::map::direction::*;
use crate::units::unit::*;
use crate::units::unit_types::UnitType;
use crate::script::get_environment;

use super::*;

macro_rules! token_module {
    ($pack: ident, $name: ident, $d: ty) => {
        #[export_module]
        mod $name {
            pub type Token = super::token::Token<$d>;

            /*#[rhai_fn(pure, get="unit_type")]
            pub fn get_unit_type(skull: &mut Skull) -> UnitType {
                skull.unit_type
            }*/

            #[rhai_fn(pure, get="owner_id")]
            pub fn get_owner_id(token: &mut Token) -> i32 {
                token.get_owner_id() as i32
            }

            /*#[rhai_fn(pure)]
            pub fn build_unit(context: NativeCallContext, skull: &mut Skull) -> UnitBuilder<$d> {
                let environment = get_environment(context);
                let mut builder = skull.unit_type.instance(&environment).set_owner_id(skull.owner.0);
                for attribute in &skull.attributes {
                    builder = builder.set_attribute(attribute);
                }
                builder
            }*/
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

token_module!(TokenPackage4, token_module4, Direction4);
token_module!(TokenPackage6, token_module6, Direction6);
