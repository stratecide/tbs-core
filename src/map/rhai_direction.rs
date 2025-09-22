use rhai::*;
use rhai::plugin::*;

use crate::map::direction::*;
use crate::map::direction::Direction as Dir;

macro_rules! direction_module {
    ($pack: ident, $name: ident, $d: ty) => {
        #[export_module]
        mod $name {
            pub type Direction = $d;
            pub type Distortion = crate::map::wrapping_map::Distortion<$d>;

            #[rhai_fn(pure, name = "==")]
            pub fn eq(d1: &mut Direction, d2: Direction) -> bool {
                *d1 == d2
            }
            #[rhai_fn(pure, name = "!=")]
            pub fn neq(d1: &mut Direction, d2: Direction) -> bool {
                *d1 != d2
            }

            #[rhai_fn(pure)]
            pub fn rotate_counter_clockwise(d: &mut Direction) -> Direction {
                d.rotate(false)
            }

            #[rhai_fn(pure)]
            pub fn rotate_clockwise(d: &mut Direction) -> Direction {
                d.rotate(true)
            }

            #[rhai_fn(pure)]
            pub fn opposite(d: &mut Direction) -> Direction {
                d.opposite_direction()
            }

            #[rhai_fn(pure)]
            pub fn update_straight_direction(distortion: &mut Distortion, d: Direction) -> Direction {
                distortion.update_direction(d)
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

direction_module!(DirectionPackage4, direction_module4, Direction4);
direction_module!(DirectionPackage6, direction_module6, Direction6);
