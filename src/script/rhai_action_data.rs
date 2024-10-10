use std::collections::HashSet;
use rhai::*;
use rhai::plugin::*;

use crate::map::direction::*;
use crate::map::point::*;
use super::custom_action::*;
use crate::units::unit::Unit;

macro_rules! action_data_module {
    ($pack: ident, $name: ident, $d: ty) => {
        #[export_module]
        mod $name {
            pub type ActionDataOptions = CustomActionDataOptions<$d>;

            #[rhai_fn()]
            pub fn success() -> bool {
                true
            }
            #[rhai_fn()]
            pub fn failure() -> bool {
                false
            }

            #[rhai_fn(name = "new_point_selection")]
            pub fn new_point_selection() -> ActionDataOptions {
                ActionDataOptions::Point(HashSet::new())
            }
            #[rhai_fn(name = "new_point_selection")]
            pub fn new_point_selection_with_points(points: Array) -> ActionDataOptions {
                let mut set = HashSet::new();
                for p in points {
                    if let Some(p) = p.try_cast::<Point>() {
                        set.insert(p);
                    }
                }
                ActionDataOptions::Point(set)
            }

            #[rhai_fn(name = "new_direction_selection")]
            pub fn new_direction_selection(center: Point) -> ActionDataOptions {
                ActionDataOptions::Direction(center, HashSet::new())
            }
            #[rhai_fn(name = "new_direction_selection")]
            pub fn new_direction_selection_with_directions(center: Point, directions: Array) -> ActionDataOptions {
                let mut set = HashSet::new();
                for d in directions {
                    if let Some(d) = d.try_cast::<$d>() {
                        set.insert(d);
                    }
                }
                ActionDataOptions::Direction(center, set)
            }

            pub fn new_unit_shop() -> ActionDataOptions {
                ActionDataOptions::UnitShop(Vec::new())
            }

            #[rhai_fn(pure, name = "len", get = "len")]
            pub fn len(options: &mut ActionDataOptions) -> i32 {
                match options {
                    ActionDataOptions::Point(set) => set.len() as i32,
                    ActionDataOptions::Direction(_, set) => set.len() as i32,
                    ActionDataOptions::UnitShop(list) => list.len() as i32,
                }
            }

            #[rhai_fn(name = "add", name = "+=")]
            pub fn add_point(options: &mut ActionDataOptions, value: Point) {
                match options {
                    ActionDataOptions::Point(set) => {
                        set.insert(value);
                    }
                    _ => (),
                }
            }

            #[rhai_fn(name = "add", name = "+=")]
            pub fn add_direction(options: &mut ActionDataOptions, value: $d) {
                match options {
                    ActionDataOptions::Direction(_, set) => {
                        set.insert(value);
                    }
                    _ => (),
                }
            }

            #[rhai_fn(name = "add")]
            pub fn add_unit_shop(options: &mut ActionDataOptions, unit: Unit<$d>, cost: i32) {
                match options {
                    ActionDataOptions::UnitShop(list) => {
                        list.push((unit, cost));
                    },
                    _ => (),
                }
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

action_data_module!(ActionDataPackage4, action_data_module4, Direction4);
action_data_module!(ActionDataPackage6, action_data_module6, Direction6);
