use rhai::*;
use rhai::plugin::*;

use crate::config::table_config::TableAxisKey;
use crate::game::game_view::GameView;
use crate::map::direction::*;
use crate::map::point::*;
use crate::map::wrapping_map::OrientedPoint;
use crate::units::unit::Unit;
use crate::terrain::terrain::Terrain;

macro_rules! board_module {
    ($name: ident, $d: ty) => {
        #[export_module]
        mod $name {
            pub type Board = Shared<dyn GameView<$d>>;

            #[rhai_fn(pure)]
            pub fn table_entry(board: &mut Board, name: &str, x: Dynamic, y: Dynamic) -> Option<Dynamic> {
                let x = TableAxisKey::from_dynamic(x)?;
                let y = TableAxisKey::from_dynamic(y)?;
                board.environment().table_entry(name, x, y)
                    .map(|value| value.into())
            }

            #[rhai_fn(pure)]
            pub fn get_neighbor(board: &mut Board, p: Point, d: $d) -> Option<Point> {
                board.get_neighbor(p, d).map(|(p, _)| p)
            }

            #[rhai_fn(pure)]
            pub fn get_unit(board: &mut Board, p: Point) -> Option<Unit<$d>> {
                board.get_unit(p)
            }

            #[rhai_fn(pure)]
            pub fn get_terrain(board: &mut Board, p: Point) -> Terrain {
                board.get_terrain(p).expect("script requested terrain at {p:?}, but that point is invalid")
            }
        }
    };
}

board_module!(board_module4, Direction4);
board_module!(board_module6, Direction6);

def_package! {
    pub BoardPackage(module)
    {
        combine_with_exported_module!(module, "board_module4", board_module4);
        combine_with_exported_module!(module, "board_module6", board_module6);
    } |> |_engine| {
    }
}
