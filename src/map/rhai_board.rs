use rustc_hash::FxHashSet;

use rhai::*;
use rhai::plugin::*;
use num_rational::Rational32;

use interfaces::ClientPerspective;

use crate::config::environment::Environment;
use crate::tags::*;
use crate::map::direction::*;
use crate::map::map::{self, *};
use crate::map::point::*;
use crate::map::wrapping_map::OrientedPoint;
use crate::terrain::terrain::Terrain;
use crate::units::unit::Unit;
use super::board::*;

#[derive(Debug, Clone)]
struct UnitWithPosition<D: Direction> {
    unit: Unit<D>,
    position: Point,
    transport_index: Option<usize>,
}

macro_rules! board_module {
    ($name: ident, $d: ty, $board: ty) => {
        #[export_module]
        mod $name {
            pub type UnitWithPosition = super::UnitWithPosition<$d>;

            #[rhai_fn(get = "config")]
            pub fn config(board: $board) -> Environment {
                board.as_ref().environment().clone()
            }

            pub fn all_positions(board: $board) -> Array {
                map::valid_points(board.as_ref()).into_iter()
                .map(Dynamic::from)
                .collect()
            }

            pub fn get_neighbor(board: $board, p: Point, d: $d) -> Dynamic {
                map::get_neighbor(board.as_ref(), p, d).map(|(p, _)| Dynamic::from(p))
                .unwrap_or(Dynamic::UNIT)
            }

            pub fn get_neighbors(board: $board, p: Point) -> Array {
                map::get_neighbors(board.as_ref(), p, NeighborMode::FollowPipes).into_iter()
                .map(|p| Dynamic::from(p.point))
                .collect()
            }

            pub fn get_neighbors_with_direction(board: $board, p: Point) -> Array {
                <$d>::list().into_iter()
                .filter_map(|d| map::get_neighbor(board.as_ref(), p, d).map(|(p, _)| Dynamic::from(OrientedPoint::simple(p, d))))
                .collect()
            }

            pub fn all_units(board: $board) -> Array {
                let mut result = Vec::new();
                for p in valid_points(board.as_ref()) {
                    if let Some(unit) = board.as_ref().get_unit(p) {
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
                uwp.transport_index.map(Dynamic::from).unwrap_or(Dynamic::UNIT)
            }

            pub fn positions_in_range(board: $board, p: Point, range: i32) -> Array {
                if range < 0 {
                    return Vec::new();
                }
                let mut set: FxHashSet<Point> = get_neighbors_layers(board.as_ref(), p, range as usize).into_iter()
                    .flatten()
                    .collect();
                set.insert(p);
                set.into_iter().map(Dynamic::from).collect()
            }

            pub fn positions_in_range_layers(board: $board, p: Point, range: i32) -> Array {
                if range < 0 {
                    return Vec::new();
                }
                get_neighbors_layers(board.as_ref(), p, range as usize).into_iter()
                .map(Dynamic::from)
                .collect()
            }

            #[rhai_fn(return_raw, pure)]
            pub fn spread_search(context: NativeCallContext, board: &mut $board, start: Point, callback: FnPtr) -> Result<Array, Box<EvalAltResult>> {
                let mut error = None;
                let result = width_search(board.as_ref(), start, Box::new(&mut |p| {
                    if !error.is_none() {
                        return false;
                    }
                    match callback.call_within_context(&context, (board.clone(), p, )) {
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

            pub fn get_unit(board: $board, p: Point) -> Dynamic {
                board.as_ref().get_unit(p)
                .cloned()
                .map(Dynamic::from)
                .unwrap_or(Dynamic::UNIT)
            }

            pub fn get_tokens(board: $board, p: Point) -> Array {
                board.as_ref().get_tokens(p).into_iter()
                .cloned()
                .map(Dynamic::from)
                .collect()
            }

            pub fn get_terrain(board: $board, p: Point) -> Terrain<$d> {
                board.as_ref().get_terrain(p).cloned().expect("script requested terrain at {p:?}, but that point is invalid")
            }

            #[rhai_fn(name = "has")]
            pub fn has_flag(board: $board, owner_id: i32, flag: FlagKey) -> bool {
                if owner_id < 0 || owner_id > i8::MAX as i32 {
                    return false;
                }
                board.as_ref().get_owning_player(owner_id as i8).map(|p| p.has_flag(flag.0))
                .unwrap_or(false)
            }

            #[rhai_fn(name = "has")]
            pub fn has_tag(board: $board, owner_id: i32, tag: TagKey) -> bool {
                if owner_id < 0 || owner_id > i8::MAX as i32 {
                    return false;
                }
                board.as_ref().get_owning_player(owner_id as i8).map(|p| p.get_tag(tag.0).is_some())
                .unwrap_or(false)
            }
            #[rhai_fn(name = "get")]
            pub fn get_tag(board: $board, owner_id: i32, key: TagKey) -> Dynamic {
                if owner_id < 0 || owner_id > i8::MAX as i32 {
                    return Dynamic::UNIT;
                }
                board.as_ref().get_owning_player(owner_id as i8).and_then(|p| p.get_tag(key.0))
                .map(|v| v.into_dynamic())
                .unwrap_or(Dynamic::UNIT)
            }

            pub fn player_income(board: $board, owner_id: i32) -> Rational32 {
                if owner_id < 0 || owner_id > i8::MAX as i32 {
                    return Rational32::from_integer(0);
                }
                get_income_factor(board.as_ref(), owner_id as i8)
            }

            pub fn is_unit_visible(board: $board, p: Point, team: i32) -> bool {
                let board = board.as_ref();
                if team >= board.environment().config.max_player_count() as i32 {
                    return false;
                }
                let Some(team) = ClientPerspective::from_i16(team as i16) else {
                    return false
                };
                let Some(unit) = board.get_unit(p) else {
                    return false
                };
                crate::game::fog::is_unit_visible(board, &unit, p, team)
            }

        }
    };
}

board_module!(board_module4, Direction4, BoardPointer<Direction4>);
board_module!(board_module6, Direction6, BoardPointer<Direction6>);

def_package! {
    pub BoardPackage4(module)
    {
        combine_with_exported_module!(module, "board_module4", board_module4);
    } |> |_engine| {
    }
}

def_package! {
    pub BoardPackage6(module)
    {
        combine_with_exported_module!(module, "board_module6", board_module6);
    } |> |_engine| {
    }
}
