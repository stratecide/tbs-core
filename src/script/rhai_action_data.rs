use std::collections::HashSet;
use rhai::*;
use rhai::plugin::*;

use crate::map::direction::*;
use crate::map::point::*;
use super::custom_action::*;

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

            #[rhai_fn(pure, name = "len", get = "len")]
            pub fn len(options: &mut ActionDataOptions) -> i32 {
                match options {
                    ActionDataOptions::Point(set) => set.len() as i32,
                    ActionDataOptions::Direction(_, set) => set.len() as i32,
                    ActionDataOptions::UnitShop(list) => list.len() as i32,
                }
            }

            #[rhai_fn(name = "add", name = "+=")]
            pub fn add(options: &mut ActionDataOptions, value: Dynamic) {
                match options {
                    ActionDataOptions::Point(set) => {
                        if let Some(p) = value.try_cast::<Point>() {
                            set.insert(p);
                        }
                    }
                    ActionDataOptions::Direction(_, set) => {
                        if let Some(d) = value.try_cast::<$d>() {
                            set.insert(d);
                        }
                    }
                    ActionDataOptions::UnitShop(_list) => {
                        // TODO
                    },
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
