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
            pub fn unit_status(mut handler: Handler, position: Point, status: ActionStatus) {
                if handler.with_map(|map| map.get_unit(position).is_some()) {
                    handler.unit_status(position, status);
                }
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
