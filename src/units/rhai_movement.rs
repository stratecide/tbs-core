use rhai::*;
use rhai::plugin::*;

use crate::map::board::BoardPointer;
use crate::map::direction::*;
use crate::map::wrapping_map::Distortion;
use crate::map::point::*;
use super::movement::*;

macro_rules! movement_module {
    ($pack: ident, $name: ident, $d: ty) => {
        #[export_module]
        mod $name {
            pub type Path = crate::units::movement::Path<$d>;

            #[rhai_fn(pure, name = "==")]
            pub fn eq(p1: &mut Path, p2: Path) -> bool {
                *p1 == p2
            }
            #[rhai_fn(pure, name = "!=")]
            pub fn neq(u1: &mut Path, u2: Path) -> bool {
                *u1 != u2
            }

            #[rhai_fn(name = "Path")]
            pub fn new_path(p: Point) -> Path {
                Path::new(p)
            }

            pub fn add(path: &mut Path, board: BoardPointer<$d>, d: $d) -> bool {
                path.steps.push(PathStep::Dir(d));
                if path.end(board.as_ref()).is_err() {
                    path.steps.pop();
                    false
                } else {
                    false
                }
            }

            pub fn pop(path: &mut Path) {
                path.steps.pop();
            }

            #[rhai_fn(pure, get = "len")]
            pub fn len(path: &mut Path) -> i32 {
                path.len() as i32
            }

            #[rhai_fn(pure, get = "start")]
            pub fn start(path: &mut Path) -> Point {
                path.start
            }
            #[rhai_fn(pure)]
            pub fn end(path: &mut Path, board: BoardPointer<$d>) -> Point {
                path.end(board.as_ref()).expect("User should not be able to create an invalid path").0
            }
            #[rhai_fn(pure)]
            pub fn distortion(path: &mut Path, board: BoardPointer<$d>) -> Distortion<$d> {
                path.end(board.as_ref()).expect("User should not be able to create an invalid path").1
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

movement_module!(MovementPackage4, movement_module4, Direction4);
movement_module!(MovementPackage6, movement_module6, Direction6);
