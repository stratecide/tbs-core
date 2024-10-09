use rhai::*;
use rhai::plugin::*;
use rustc_hash::{FxHashMap, FxHashSet};

use super::event_handler::EventHandler;
use crate::game::game_view::GameView;
use crate::map::direction::*;
use crate::map::point::*;
use crate::units::unit::Unit;
use crate::units::attributes::{AttributeKey, ActionStatus};
use crate::terrain::terrain::Terrain;
use crate::terrain::attributes::TerrainAttributeKey;

macro_rules! event_handler_module {
    ($pack: ident, $name: ident, $d: ty) => {
        #[export_module]
        mod $name {
            pub type Handler = EventHandler<$d>;

            #[rhai_fn()]
            pub fn spend_money(mut handler: Handler, owner_id: i32, amount: i32) {
                if owner_id < 0 || owner_id > i8::MAX as i32 {
                    return;
                }
                let owner_id = owner_id as i8;
                if handler.with_game(|game| game.get_owning_player(owner_id).is_some()) {
                    handler.money_buy(owner_id, amount);
                }
            }

            #[rhai_fn()]
            pub fn heal_unit(mut handler: Handler, position: Point, amount: i32) {
                if amount > 0 && handler.with_map(|map| map.get_unit(position).is_some()) {
                    handler.unit_heal(position, amount.min(100) as u8);
                }
            }

            #[rhai_fn()]
            pub fn damage_unit(mut handler: Handler, position: Point, amount: i32) {
                if amount > 0 && handler.with_map(|map| map.get_unit(position).is_some()) {
                    handler.unit_damage(position, amount.min(999) as u16);
                }
            }

            #[rhai_fn()]
            pub fn damage_units(mut handler: Handler, map: FxHashMap<Point, i32>) {
                let map = map.into_iter()
                    .filter(|(_, damage)| *damage > 0)
                    .map(|(p, damage)| (p, damage.min(999) as u16))
                    .collect();
                handler.unit_mass_damage(&map);
            }

            #[rhai_fn()]
            pub fn set_unit_status(mut handler: Handler, position: Point, status: ActionStatus) {
                if handler.with_map(|map| map.get_unit(position).is_some()) {
                    handler.unit_status(position, status);
                }
            }

            #[rhai_fn()]
            pub fn make_player_lose(mut handler: Handler, owner_id: i32) {
                if owner_id < 0 || owner_id > i8::MAX as i32 {
                    return;
                }
                handler.player_dies(owner_id as i8)
            }

            #[rhai_fn()]
            pub fn place_unit(mut handler: Handler, position: Point, unit: Unit<$d>) {
                handler.unit_creation(position, unit);
            }

            #[rhai_fn()]
            pub fn take_unit(mut handler: Handler, position: Point) -> Dynamic {
                if let Some(unit) = handler.with_map(|map| map.get_unit(position).cloned()) {
                    handler.unit_remove(position);
                    Dynamic::from(unit)
                } else {
                    ().into()
                }
            }

            #[rhai_fn()]
            pub fn set_terrain_anger(mut handler: Handler, position: Point, anger: i32) {
                let anger = anger.max(0).min(handler.environment().config.terrain_max_anger() as i32) as u8;
                let has_attribute = handler.with_map(|map| {
                    map.get_terrain(position).expect(&format!("Missing terrain at {:?}", position))
                    .has_attribute(TerrainAttributeKey::Anger)
                });
                if has_attribute {
                    handler.terrain_anger(position, anger);
                }
            }
        }

        def_package! {
            pub $pack(module)
            {
                combine_with_exported_module!(module, stringify!($name), $name);
                combine_with_exported_module!(module, "mass_damage", mass_damage);
            } |> |_engine| {
            }
        }
    };
}

event_handler_module!(EventHandlerPackage4, event_handler_module4, Direction4);
event_handler_module!(EventHandlerPackage6, event_handler_module6, Direction6);

#[export_module]
mod mass_damage{

    #[rhai_fn()]
    pub fn new_mass_damage() -> FxHashMap<Point, i32> {
        FxHashMap::default()
    }

    #[rhai_fn(pure)]
    pub fn len(map: &mut FxHashMap<Point, i32>) -> i32 {
        map.len() as i32
    }

    #[rhai_fn(name = "add")]
    pub fn add(map: &mut FxHashMap<Point, i32>, p: Point, amount: i32) {
        let damage = map.remove(&p).unwrap_or(0) + amount;
        map.insert(p, damage);
    }

    #[rhai_fn(name = "remove")]
    pub fn remove(map: &mut FxHashMap<Point, i32>, p: Point) -> Dynamic {
        if let Some(damage) = map.remove(&p) {
            damage.into()
        } else {
            ().into()
        }
    }
}
