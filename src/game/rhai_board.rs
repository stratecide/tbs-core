use std::ops::Deref;

use rhai::*;
use rhai::plugin::*;

use crate::game::game_view::GameView;
use crate::map::direction::*;
use crate::map::map::NeighborMode;
use crate::map::point::*;
use crate::map::wrapping_map::OrientedPoint;
use crate::terrain::terrain::Terrain;
use crate::details::*;

#[derive(Clone)]
pub struct SharedGameView<D: Direction>(pub Shared<dyn GameView<D>>);

impl<D: Direction> Deref for SharedGameView<D> {
    type Target = Shared<dyn GameView<D>>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

macro_rules! board_module {
    ($pack: ident, $name: ident, $d: ty) => {
        #[export_module]
        mod $name {
            pub type Board = SharedGameView<$d>;

            #[rhai_fn(pure)]
            pub fn all_points(board: &mut Board) -> Array {
                board.all_points().into_iter()
                .map(|p| Dynamic::from(p))
                .collect()
            }

            #[rhai_fn(pure)]
            pub fn get_neighbor(board: &mut Board, p: Point, d: $d) -> Dynamic {
                board.get_neighbor(p, d).map(|(p, _)| Dynamic::from(p))
                .unwrap_or(().into())
            }

            #[rhai_fn(pure)]
            pub fn get_neighbors(board: &mut Board, p: Point) -> Array {
                board.get_neighbors(p, NeighborMode::FollowPipes).into_iter()
                .map(|p| Dynamic::from(p.point))
                .collect()
            }

            #[rhai_fn(pure)]
            pub fn get_neighbors_with_direction(board: &mut Board, p: Point) -> Array {
                <$d>::list().into_iter()
                .filter_map(|d| board.get_neighbor(p, d).map(|(p, _)| Dynamic::from(OrientedPoint::simple(p, d))))
                .collect()
            }

            #[rhai_fn(pure)]
            pub fn all_positions(board: &mut Board) -> Array {
                board.all_points().into_iter()
                .map(|p| Dynamic::from(p))
                .collect()
            }

            #[rhai_fn(pure)]
            pub fn positions_in_range(board: &mut Board, p: Point, range: i32) -> Array {
                if range < 0 {
                    return Vec::new();
                }
                board.range_in_layers(p, range as usize).into_iter()
                .flatten()
                .map(|p| Dynamic::from(p))
                .collect()
            }

            #[rhai_fn(pure)]
            pub fn positions_in_range_layers(board: &mut Board, p: Point, range: i32) -> Array {
                if range < 0 {
                    return Vec::new();
                }
                board.range_in_layers(p, range as usize).into_iter()
                .map(|p| Dynamic::from(p))
                .collect()
            }

            #[rhai_fn(pure)]
            pub fn get_unit(board: &mut Board, p: Point) -> Dynamic {
                board.get_unit(p)
                .map(|u| Dynamic::from(u))
                .unwrap_or(().into())
            }

            #[rhai_fn(pure)]
            pub fn get_terrain(board: &mut Board, p: Point) -> Terrain {
                board.get_terrain(p).expect("script requested terrain at {p:?}, but that point is invalid")
            }

            #[rhai_fn(pure)]
            pub fn get_skull(board: &mut Board, p: Point, owner_id: i32) -> Dynamic {
                for detail in board.get_details(p) {
                    match detail {
                        Detail::Skull(skull) => {
                            if skull.get_owner_id() as i32 == owner_id {
                                return Dynamic::from(skull);
                            }
                        }
                        _ => ()
                    }
                }
                ().into()
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

        def_package! {
            pub $pack(module)
            {
                combine_with_exported_module!(module, stringify!($name), $name);
            } |> |_engine| {
            }
        }
    };
}

board_module!(BoardPackage4, board_module4, Direction4);
board_module!(BoardPackage6, board_module6, Direction6);
