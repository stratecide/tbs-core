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
//pub mod custom_power;
/*pub mod defend;
pub mod unit;
pub mod player;
pub mod attack;
pub mod kill;
pub mod terrain;
pub mod death;*/

pub const CONST_NAME_EVENT_HANDLER: &'static str = "EVENT_HANDLER";
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

pub const FUNCTION_NAME_CONFIG: &'static str = "CONFIG";
pub const FUNCTION_NAME_BOARD: &'static str = "BOARD";
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
    rhai_environment::EnvironmentPackage::new().register_into_engine(&mut engine);
    crate::terrain::rhai_terrain::TerrainPackage::new().register_into_engine(&mut engine);
    // https://rhai.rs/book/safety/max-stmt-depth.html
    // ran into problems with the debug-build defaults 32, 16
    engine.set_max_expr_depths(64, 32);
    // https://rhai.rs/book/safety/max-call-stack.html
    // 8 should be enough
    engine.set_max_call_levels(8);
    engine
}

pub fn create_d_engine<D: Direction>() -> Engine {
    let mut engine = create_base_engine();
    if D::is_hex() {
        crate::map::rhai_point::PositionPackage6::new().register_into_engine(&mut engine);
        crate::map::rhai_direction::DirectionPackage6::new().register_into_engine(&mut engine);
        crate::game::rhai_board::BoardPackage6::new().register_into_engine(&mut engine);
        crate::units::rhai_unit::UnitPackage6::new().register_into_engine(&mut engine);
    } else {
        crate::map::rhai_point::PositionPackage4::new().register_into_engine(&mut engine);
        crate::map::rhai_direction::DirectionPackage4::new().register_into_engine(&mut engine);
        crate::game::rhai_board::BoardPackage4::new().register_into_engine(&mut engine);
        crate::units::rhai_unit::UnitPackage4::new().register_into_engine(&mut engine);
    }
    engine
}

pub fn with_board<D: Direction, R>(context: NativeCallContext, f: impl FnOnce(&Shared<dyn GameView<D>>) -> R) -> R {
    let board = context.call_native_fn::<SharedGameView<D>>(FUNCTION_NAME_BOARD, ()).expect("BOARD should be in context!");
    f(&board.0)
}

pub fn get_environment(context: NativeCallContext) -> Environment {
    context.call_native_fn::<Environment>(FUNCTION_NAME_CONFIG, ()).expect("CONFIG should be in context!")
}
