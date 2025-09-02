use std::ops::Deref;
use rustc_hash::FxHashSet;

use rhai::*;
use rhai::plugin::*;
use num_rational::Rational32;

use interfaces::ClientPerspective;

use crate::game::game_view::GameView;
use uniform_smart_pointer::Urc;
use crate::tags::*;
use crate::map::direction::*;
use crate::map::map::{NeighborMode, get_income_factor};
use crate::map::point::*;
use crate::map::wrapping_map::OrientedPoint;
use crate::terrain::terrain::Terrain;
use crate::units::unit::Unit;

#[derive(Clone)]
pub struct SharedGameView<D: Direction>(pub Urc<dyn GameView<D>>);

impl<D: Direction> Deref for SharedGameView<D> {
    type Target = Urc<dyn GameView<D>>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Debug, Clone)]
struct UnitWithPosition<D: Direction> {
    unit: Unit<D>,
    position: Point,
    transport_index: Option<usize>,
}

macro_rules! board_module {
    ($pack: ident, $name: ident, $d: ty) => {
        #[export_module]
        mod $name {
            pub type Board = SharedGameView<$d>;
            pub type UnitWithPosition = super::UnitWithPosition<$d>;

            #[rhai_fn(pure)]
            pub fn all_points(board: &mut Board) -> Array {
                board.all_points().into_iter()
                .map(Dynamic::from)
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
                .map(Dynamic::from)
                .collect()
            }

            #[rhai_fn(pure)]
            pub fn all_units(board: &mut Board) -> Array {
                let mut result = Vec::new();
                for p in board.all_points() {
                    if let Some(unit) = board.get_unit(p) {
                        result.push(Dynamic::from(UnitWithPosition {
                            unit: unit.clone(),
                            position: p,
                            transport_index: None,
                        }));
                        for (i, unit) in unit.get_transported().iter().enumerate() {
                            result.push(Dynamic::from(UnitWithPosition {
                                unit: unit.clone(),
                                position: p,
                                transport_index: Some(i),
                            }));
                        }
                    }
                }
                result
            }

            #[rhai_fn(pure, get = "unit")]
            pub fn get_uwp_unit(uwp: &mut UnitWithPosition) -> Unit<$d> {
                uwp.unit.clone()
            }

            #[rhai_fn(pure, get = "position")]
            pub fn get_uwp_position(uwp: &mut UnitWithPosition) -> Point {
                uwp.position
            }

            #[rhai_fn(pure, get = "transport_index")]
            pub fn get_uwp_transport_index(uwp: &mut UnitWithPosition) -> Dynamic {
                uwp.transport_index.map(Dynamic::from).unwrap_or(().into())
            }

            #[rhai_fn(pure)]
            pub fn positions_in_range(board: &mut Board, p: Point, range: i32) -> Array {
                if range < 0 {
                    return Vec::new();
                }
                let mut set: FxHashSet<Point> = board.range_in_layers(p, range as usize).into_iter()
                    .flatten()
                    .collect();
                set.insert(p);
                set.into_iter().map(Dynamic::from).collect()
            }

            #[rhai_fn(pure)]
            pub fn positions_in_range_layers(board: &mut Board, p: Point, range: i32) -> Array {
                if range < 0 {
                    return Vec::new();
                }
                board.range_in_layers(p, range as usize).into_iter()
                .map(Dynamic::from)
                .collect()
            }

            #[rhai_fn(return_raw, pure)]
            pub fn spread_search(context: NativeCallContext, board: &mut Board, start: Point, callback: FnPtr) -> Result<Array, Box<EvalAltResult>> {
                let mut error = None;
                let result = board.width_search(start, Box::new(&mut |p| {
                    if !error.is_none() {
                        return false;
                    }
                    match callback.call_within_context(&context, (p, )) {
                        Ok(accepted) => accepted,
                        Err(e) => {
                            error = Some(e);
                            false
                        }
                    }
                }));
                if let Some(error) = error {
                    return Err(error);
                }
                Ok(result.into_iter()
                    .map(|p| Dynamic::from(p))
                    .collect())
            }

            #[rhai_fn(pure)]
            pub fn get_unit(board: &mut Board, p: Point) -> Dynamic {
                board.get_unit(p)
                .map(Dynamic::from)
                .unwrap_or(().into())
            }

            #[rhai_fn(pure)]
            pub fn get_tokens(board: &mut Board, p: Point) -> Array {
                board.get_tokens(p).into_iter()
                .map(Dynamic::from)
                .collect()
            }

            #[rhai_fn(pure)]
            pub fn get_terrain(board: &mut Board, p: Point) -> Terrain<$d> {
                board.get_terrain(p).expect("script requested terrain at {p:?}, but that point is invalid")
            }

            #[rhai_fn(pure, name = "has")]
            pub fn has_flag(board: &mut Board, owner_id: i32, flag: FlagKey) -> bool {
                if owner_id < 0 || owner_id > i8::MAX as i32 {
                    return false;
                }
                board.get_owning_player(owner_id as i8).map(|p| p.has_flag(flag.0))
                .unwrap_or(false)
            }

            #[rhai_fn(pure, name = "has")]
            pub fn has_tag(board: &mut Board, owner_id: i32, tag: TagKey) -> bool {
                if owner_id < 0 || owner_id > i8::MAX as i32 {
                    return false;
                }
                board.get_owning_player(owner_id as i8).map(|p| p.get_tag(tag.0).is_some())
                .unwrap_or(false)
            }
            #[rhai_fn(pure, name = "get")]
            pub fn get_tag(board: &mut Board, owner_id: i32, key: TagKey) -> Dynamic {
                if owner_id < 0 || owner_id > i8::MAX as i32 {
                    return ().into();
                }
                board.get_owning_player(owner_id as i8).and_then(|p| p.get_tag(key.0))
                .map(|v| v.into_dynamic())
                .unwrap_or(().into())
            }

            #[rhai_fn(pure)]
            pub fn player_income(board: &mut Board, owner_id: i32) -> Rational32 {
                if owner_id < 0 || owner_id > i8::MAX as i32 {
                    return Rational32::from_integer(0);
                }
                get_income_factor(&**board, owner_id as i8)
            }

            #[rhai_fn(pure)]
            pub fn is_unit_visible(board: &mut Board, p: Point, team: i32) -> bool {
                if team >= board.environment().config.max_player_count() as i32 {
                    return false;
                }
                let Some(team) = ClientPerspective::from_i16(team as i16) else {
                    return false
                };
                let Some(unit) = board.get_unit(p) else {
                    return false
                };
                crate::game::fog::is_unit_visible(&**board, &unit, p, team)
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
