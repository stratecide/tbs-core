use rhai::*;
use rhai::plugin::*;

use super::event_handler::EventHandler;
use crate::map::direction::*;
use crate::map::point::*;
use crate::units::unit::Unit;
use crate::units::attributes::{AttributeKey, ActionStatus};
use crate::terrain::terrain::Terrain;

macro_rules! event_handler_module {
    ($name: ident, $d: ty) => {
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
        }
    };
}

event_handler_module!(event_handler_module4, Direction4);
event_handler_module!(event_handler_module6, Direction6);

def_package! {
    pub EventHandlerPackage(module)
    {
        combine_with_exported_module!(module, "event_handler_module4", event_handler_module4);
        combine_with_exported_module!(module, "event_handler_module6", event_handler_module6);
    } |> |_engine| {
    }
}
