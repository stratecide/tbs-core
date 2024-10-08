use rhai::*;
use rhai::plugin::*;

use crate::map::direction::*;
use crate::units::unit_types::UnitType;

#[export_module]
mod unit_type_module {
    pub type UnitType = crate::units::unit_types::UnitType;

    #[rhai_fn(pure, name = "==")]
    pub fn eq(p1: &mut UnitType, p2: UnitType) -> bool {
        *p1 == p2
    }
}

macro_rules! board_module {
    ($name: ident, $d: ty) => {
        #[export_module]
        mod $name {
            pub type Unit = crate::units::unit::Unit<$d>;

            #[rhai_fn(pure, name = "==")]
            pub fn eq(p1: &mut Unit, p2: Unit) -> bool {
                *p1 == p2
            }

            #[rhai_fn(pure, get = "type")]
            pub fn get_type(unit: &mut Unit) -> UnitType {
                unit.typ()
            }

            #[rhai_fn(pure, get = "owner_id")]
            pub fn get_owner_id(unit: &mut Unit) -> i32 {
                unit.get_owner_id() as i32
            }

            #[rhai_fn(pure, get = "hp")]
            pub fn get_hp(unit: &mut Unit) -> i32 {
                unit.get_hp() as i32
            }
        }
    };
}

board_module!(unit_module4, Direction4);
board_module!(unit_module6, Direction6);

def_package! {
    pub UnitPackage(module)
    {
        combine_with_exported_module!(module, "unit_type_module", unit_type_module);
        combine_with_exported_module!(module, "unit_module4", unit_module4);
        combine_with_exported_module!(module, "unit_module6", unit_module6);
    } |> |_engine| {
    }
}
