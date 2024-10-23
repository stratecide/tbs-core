use rhai::*;
use rhai::packages::*;

use crate::config::environment::Environment;
use crate::game::game_view::GameView;
use crate::game::rhai_board::SharedGameView;
use crate::map::direction::Direction;

pub mod executor;
pub mod custom_action;
mod rhai_environment;
mod rhai_action_data;
mod rhai_fraction;
//pub mod custom_power;
/*pub mod defend;
pub mod unit;
pub mod player;
pub mod attack;
pub mod kill;
pub mod terrain;
pub mod death;*/

pub const CONST_NAME_CONFIG: &'static str = "CONFIG";
pub const CONST_NAME_BOARD: &'static str = "BOARD";
pub const CONST_NAME_EVENT_HANDLER: &'static str = "EVENT_HANDLER";
pub const CONST_NAME_OWNER_ID: &'static str = "OWNER_ID";
pub const CONST_NAME_TEAM: &'static str = "TEAM";
pub const CONST_NAME_POSITION: &'static str = "POSITION";
pub const CONST_NAME_OTHER_POSITION: &'static str = "OTHER_POSITION";
pub const CONST_NAME_TERRAIN: &'static str = "TERRAIN";
pub const CONST_NAME_IS_BUBBLE: &'static str = "IS_BUBBLE";
pub const CONST_NAME_UNIT: &'static str = "UNIT";
pub const CONST_NAME_UNIT_ID: &'static str = "UNIT_ID";
pub const CONST_NAME_OTHER_UNIT: &'static str = "OTHER_UNIT";
pub const CONST_NAME_OTHER_UNIT_ID: &'static str = "OTHER_UNIT_ID";
pub const CONST_NAME_PATH: &'static str = "PATH";
pub const CONST_NAME_TRANSPORTER: &'static str = "TRANSPORTER";
pub const CONST_NAME_TRANSPORT_INDEX: &'static str = "TRANSPORT_INDEX";
pub const CONST_NAME_TRANSPORTER_POSITION: &'static str = "TRANSPORTER_POSITION";
pub const CONST_NAME_IS_COUNTER: &'static str = "IS_COUNTER";
pub const CONST_NAME_DAMAGE: &'static str = "DAMAGE";
pub const CONST_NAME_INTERRUPTED: &'static str = "INTERRUPTED";

pub const FUNCTION_NAME_INPUT_CHOICE: &'static str = "user_selection";
pub const FUNCTION_NAME_BLAST_DIRECTION: &'static str = "get_blast_direction";

pub fn create_base_engine() -> Engine {
    let mut engine = Engine::new_raw();
    // add built-in packages
    CorePackage::new().register_into_engine(&mut engine);
    LogicPackage::new().register_into_engine(&mut engine);
    BasicArrayPackage::new().register_into_engine(&mut engine);
    BasicMapPackage::new().register_into_engine(&mut engine);
    // maybe add MoreStringPackage or BitFieldPackage
    // my packages
    rhai_fraction::FractionPackage::new().register_into_engine(&mut engine);
    // https://rhai.rs/book/safety/max-stmt-depth.html
    // ran into problems with the debug-build defaults 32, 16
    engine.set_max_expr_depths(64, 32);
    // https://rhai.rs/book/safety/max-call-stack.html
    // 8 should be enough
    engine.set_max_call_levels(8);
    engine
}

def_package! {
    pub MyPackage4(module):
        CorePackage,
        LogicPackage,
        BasicArrayPackage,
        BasicMapPackage,
        rhai_environment::EnvironmentPackage,
        rhai_fraction::FractionPackage,
        crate::terrain::rhai_terrain::TerrainPackage,
        crate::map::rhai_point::PositionPackage4,
        crate::map::rhai_direction::DirectionPackage4,
        crate::game::rhai_board::BoardPackage4,
        crate::units::rhai_unit::UnitPackage4,
        crate::units::rhai_combat::CombatPackage4,
        crate::units::rhai_movement::MovementPackage4,
        crate::details::rhai_details::DetailPackage4,
        crate::game::rhai_event_handler::EventHandlerPackage4 {}
}

def_package! {
    pub MyPackage6(module):
        CorePackage,
        LogicPackage,
        BasicArrayPackage,
        BasicMapPackage,
        rhai_environment::EnvironmentPackage,
        rhai_fraction::FractionPackage,
        crate::terrain::rhai_terrain::TerrainPackage,
        crate::map::rhai_point::PositionPackage6,
        crate::map::rhai_direction::DirectionPackage6,
        crate::game::rhai_board::BoardPackage6,
        crate::units::rhai_unit::UnitPackage6,
        crate::units::rhai_combat::CombatPackage6,
        crate::units::rhai_movement::MovementPackage6,
        crate::details::rhai_details::DetailPackage6,
        crate::game::rhai_event_handler::EventHandlerPackage6 {}
}

pub fn with_board<D: Direction, R>(context: NativeCallContext, f: impl FnOnce(&Shared<dyn GameView<D>>) -> R) -> R {
    let board: SharedGameView<D> = context.engine().eval_expression(CONST_NAME_BOARD).expect("BOARD should be in context!");
    f(&board.0)
}

pub fn get_environment(context: NativeCallContext) -> Environment {
    context.engine().eval_expression::<Environment>(CONST_NAME_CONFIG).expect("CONFIG should be in context!")
}
