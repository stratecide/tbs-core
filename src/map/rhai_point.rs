use rhai::*;
use rhai::plugin::*;
use super::point::Point;
use super::direction::*;
use super::wrapping_map::OrientedPoint;

#[export_module]
mod tile_position_module {
    pub type Position = Point;

    #[rhai_fn(pure, name = "==")]
    pub fn eq(p1: &mut Position, p2: Position) -> bool {
        *p1 == p2
    }
}

macro_rules! oriented_point_module {
    ($name: ident, $d: ty) => {
        #[export_module]
        mod $name {
            pub type OrientedPosition = OrientedPoint<$d>;

            #[rhai_fn(pure, name = "==")]
            pub fn eq(d1: &mut OrientedPosition, d2: OrientedPosition) -> bool {
                *d1 == d2
            }

            #[rhai_fn(pure)]
            pub fn with_orientation(p: &mut Point) -> OrientedPosition {
                OrientedPosition::new(*p, false, <$d>::angle_0())
            }
        }
    };
}

oriented_point_module!(oriented_point_module4, Direction4);
oriented_point_module!(oriented_point_module6, Direction6);

def_package! {
    pub PositionPackage(module)
    {
        combine_with_exported_module!(module, "tile_position_module", tile_position_module);
        combine_with_exported_module!(module, "oriented_point_module4", oriented_point_module4);
        combine_with_exported_module!(module, "oriented_point_module6", oriented_point_module6);
    } |> |_engine| {
    }
}
