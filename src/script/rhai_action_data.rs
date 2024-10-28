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
            pub type ShopItem = crate::script::custom_action::ShopItem<$d>;

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

            #[rhai_fn(name = "add", name = "+=")]
            pub fn add_point(options: &mut ActionDataOptions, value: Point) {
                match options {
                    ActionDataOptions::Point(set) => {
                        set.insert(value);
                    }
                    _ => (),
                }
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

            #[rhai_fn(name = "add", name = "+=")]
            pub fn add_direction(options: &mut ActionDataOptions, value: $d) {
                match options {
                    ActionDataOptions::Direction(_, set) => {
                        set.insert(value);
                    }
                    _ => (),
                }
            }

            #[rhai_fn(name = "ShopItem")]
            pub fn new_shop_item(key: Dynamic) -> Dynamic {
                if let Some(key) = ShopItemKey::from_dynamic(key) {
                    Dynamic::from(ShopItem {
                        key,
                        enabled: true,
                        costs: Vec::new(),
                    })
                } else {
                    ().into()
                }
            }

            #[rhai_fn(pure, get = "key")]
            pub fn shop_item_get_key(item: &mut ShopItem) -> Dynamic {
                item.key.into_dynamic()
            }
            #[rhai_fn(set = "key")]
            pub fn shop_item_set_key(item: &mut ShopItem, key: Dynamic) {
                if let Some(key) = ShopItemKey::from_dynamic(key) {
                    item.key = key;
                }
            }

            #[rhai_fn(pure, get = "enabled")]
            pub fn shop_item_get_enabled(item: &mut ShopItem) -> bool {
                item.enabled
            }
            #[rhai_fn(set = "enabled")]
            pub fn shop_item_set_enabled(item: &mut ShopItem, enabled: bool) {
                item.enabled = enabled;
            }

            #[rhai_fn(pure, get = "costs")]
            pub fn shop_item_get_costs(item: &mut ShopItem) -> Array {
                item.costs.iter()
                .map(|cost| cost.map(|cost| Dynamic::from(cost)).unwrap_or(().into()))
                .collect()
            }
            #[rhai_fn(set = "costs")]
            pub fn shop_item_set_costs(item: &mut ShopItem, costs: Array) {
                item.costs = costs.into_iter()
                .map(|d| d.try_cast())
                .collect();
            }

            pub fn new_shop(name: &str) -> ActionDataOptions {
                ActionDataOptions::Shop(name.to_string(), Vec::new())
            }

            #[rhai_fn(pure, name = "len", get = "len")]
            pub fn len(options: &mut ActionDataOptions) -> i32 {
                match options {
                    ActionDataOptions::Point(set) => set.len() as i32,
                    ActionDataOptions::Direction(_, set) => set.len() as i32,
                    ActionDataOptions::Shop(_, list) => list.len() as i32,
                }
            }

            #[rhai_fn(name = "add")]
            pub fn add_shop_item(options: &mut ActionDataOptions, item: ShopItem) {
                match options {
                    ActionDataOptions::Shop(_, list) => {
                        if list.len() >= MAXIMUM_SHOP_SIZE {
                            return;
                        }
                        list.push(item);
                    },
                    _ => (),
                }
            }
            #[rhai_fn(name = "add")]
            pub fn add_shop_unit(options: &mut ActionDataOptions, unit: Unit<$d>, cost: i32) {
                match options {
                    ActionDataOptions::Shop(_, list) => {
                        if list.len() >= MAXIMUM_SHOP_SIZE {
                            return;
                        }
                        list.push(ShopItem {
                            key: ShopItemKey::Unit(unit),
                            enabled: true,
                            costs: vec![Some(cost)],
                        });
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
