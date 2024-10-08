use std::ops::Deref;

use rhai::*;
use rhai::plugin::*;

use crate::game::game_view::GameView;
use crate::map::direction::*;
use crate::map::point::*;
use crate::units::unit::Unit;
use crate::terrain::terrain::Terrain;

#[derive(Clone)]
pub struct SharedGameView<D: Direction>(pub Shared<dyn GameView<D>>);

impl<D: Direction> Deref for SharedGameView<D> {
    type Target = Shared<dyn GameView<D>>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

macro_rules! board_module {
    ($name: ident, $d: ty) => {
        #[export_module]
        mod $name {
            pub type Board = SharedGameView<$d>;

            #[rhai_fn(pure)]
            pub fn get_neighbor(board: &mut Board, p: Point, d: $d) -> Option<Point> {
                board.get_neighbor(p, d).map(|(p, _)| p)
            }

            #[rhai_fn(pure)]
            pub fn get_unit(board: &mut Board, p: Point) -> Option<Unit<$d>> {
                board.get_unit(p)
            }

            #[rhai_fn()]
            pub fn get_terrain(board: Board, p: Point) -> Terrain {
                board.get_terrain(p).expect("script requested terrain at {p:?}, but that point is invalid")
            }

            #[rhai_fn(pure)]
            pub fn player_funds(board: &mut Board, owner_id: i32) -> i32 {
                if owner_id < 0 || owner_id > i8::MAX as i32 {
                    return 0;
                }
                board.get_owning_player(owner_id as i8)
                .map(|player| *player.funds)
                .unwrap_or(0)
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
