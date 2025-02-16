use rhai::*;
use rhai::plugin::*;
use super::point::{Point, PointWithDistortion};
use super::direction::*;
use super::wrapping_map::{Distortion, OrientedPoint};

#[export_module]
mod tile_position_module {
    pub type Position = Point;

    #[rhai_fn(pure, name = "==")]
    pub fn eq(p1: &mut Position, p2: Position) -> bool {
        *p1 == p2
    }
    #[rhai_fn(pure, name = "!=")]
    pub fn neq(p1: &mut Position, p2: Position) -> bool {
        *p1 != p2
    }
}

macro_rules! oriented_point_module {
    ($pack: ident, $name: ident, $d: ty) => {
        #[export_module]
        mod $name {
            pub type OrientedPosition = OrientedPoint<$d>;
            pub type PositionWithDistortion = PointWithDistortion<$d>;

            #[rhai_fn(pure, name = "==")]
            pub fn eq(d1: &mut OrientedPosition, d2: OrientedPosition) -> bool {
                *d1 == d2
            }
            #[rhai_fn(pure, name = "!=")]
            pub fn neq(p1: &mut OrientedPosition, p2: OrientedPosition) -> bool {
                *p1 != p2
            }
        
            #[rhai_fn(pure, name = "==")]
            pub fn eq_pwd(d1: &mut PositionWithDistortion, d2: PositionWithDistortion) -> bool {
                *d1 == d2
            }
            #[rhai_fn(pure, name = "!=")]
            pub fn neq_pwd(p1: &mut PositionWithDistortion, p2: PositionWithDistortion) -> bool {
                *p1 != p2
            }
        
            #[rhai_fn(pure)]
            pub fn with_orientation(p: &mut Point) -> OrientedPosition {
                OrientedPosition::new(*p, false, <$d>::angle_0())
            }
        
            #[rhai_fn(pure)]
            pub fn with_distortion(p: &mut Point) -> PositionWithDistortion {
                PositionWithDistortion::new(*p, Distortion::neutral())
            }
        
            #[rhai_fn(pure, get = "point")]
            pub fn get_point(p: &mut OrientedPosition) -> Point {
                p.point
            }
            #[rhai_fn(pure, get = "direction")]
            pub fn get_direction(p: &mut OrientedPosition) -> $d {
                p.direction
            }
            #[rhai_fn(pure, get = "mirrored")]
            pub fn get_mirrored(p: &mut OrientedPosition) -> bool {
                p.mirrored
            }
        
            #[rhai_fn(pure, get = "point")]
            pub fn get_point_pwd(p: &mut PositionWithDistortion) -> Point {
                p.point
            }
            #[rhai_fn(pure, get = "distortion")]
            pub fn get_distortion(p: &mut PositionWithDistortion) -> Distortion<$d> {
                p.distortion
            }
        }

        def_package! {
            pub $pack(module)
            {
                combine_with_exported_module!(module, "tile_position_module", tile_position_module);
                combine_with_exported_module!(module, stringify!($name), $name);
            } |> |_engine| {
            }
        }
    };
}

oriented_point_module!(PositionPackage4, oriented_point_module4, Direction4);
oriented_point_module!(PositionPackage6, oriented_point_module6, Direction6);
