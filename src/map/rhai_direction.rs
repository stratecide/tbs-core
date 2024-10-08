use rhai::*;
use rhai::plugin::*;

use crate::map::direction::*;
use crate::map::direction::Direction as Dir;

macro_rules! direction_module {
    ($name: ident, $d: ty) => {
        #[export_module]
        mod $name {
            pub type Direction = $d;

            #[rhai_fn(pure, name = "==")]
            pub fn eq(d1: &mut $d, d2: $d) -> bool {
                *d1 == d2
            }

            #[rhai_fn(pure)]
            pub fn opposite(d: &mut $d) -> $d {
                d.opposite_direction()
            }
        }
    };
}

direction_module!(direction_module4, Direction4);
direction_module!(direction_module6, Direction6);

def_package! {
    pub DirectionPackage(module)
    {
        combine_with_exported_module!(module, "direction_module4", direction_module4);
        combine_with_exported_module!(module, "direction_module6", direction_module6);
    } |> |_engine| {
    }
}
