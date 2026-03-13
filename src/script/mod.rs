use rhai::packages::*;
use rhai::*;

pub mod custom_action;
pub mod executor;
mod rhai_action_data;
mod rhai_environment;
mod rhai_fraction;

pub const CONST_NAME_CONFIG: &'static str = "config";
pub const CONST_NAME_BOARD: &'static str = "board";
pub const CONST_NAME_EVENT_HANDLER: &'static str = "event_handler";
pub const CONST_NAME_ATTACK_CONTEXT: &'static str = "attack";
pub const CONST_NAME_OWNER_ID: &'static str = "owner_id";
pub const CONST_NAME_TEAM: &'static str = "team";
pub const CONST_NAME_STARTING_POSITION: &'static str = "starting_position";
pub const CONST_NAME_POSITION: &'static str = "position";
pub const CONST_NAME_OTHER_POSITION: &'static str = "other_position";
pub const CONST_NAME_HEROES: &'static str = "heroes";
pub const CONST_NAME_TERRAIN: &'static str = "terrain";
pub const CONST_NAME_TOKEN: &'static str = "token";
pub const CONST_NAME_UNIT: &'static str = "unit";
pub const CONST_NAME_UNIT_ID: &'static str = "unit_id";
pub const CONST_NAME_OTHER_UNIT: &'static str = "other_unit";
pub const CONST_NAME_OTHER_UNIT_ID: &'static str = "other_unit_id";
pub const CONST_NAME_ATTACKER: &'static str = "attacker";
pub const CONST_NAME_ATTACKER_ID: &'static str = "attacker_id";
pub const CONST_NAME_ATTACKER_POSITION: &'static str = "attacker_position";
pub const CONST_NAME_ATTACK_DIRECTION: &'static str = "attacker_direction";
pub const CONST_NAME_DEFENDER: &'static str = "defender";
pub const CONST_NAME_DEFENDER_ID: &'static str = "defender_id";
pub const CONST_NAME_DEFENDER_POSITION: &'static str = "defender_position";
pub const CONST_NAME_DEFENDER_POSITIONS: &'static str = "defender_positions";
pub const CONST_NAME_PATH: &'static str = "path";
pub const CONST_NAME_TRANSPORTER: &'static str = "transporter";
pub const CONST_NAME_TRANSPORT_INDEX: &'static str = "transporter_index";
pub const CONST_NAME_TRANSPORTER_POSITION: &'static str = "transporter_position";
pub const CONST_NAME_IS_COUNTER: &'static str = "is_counter";
pub const CONST_NAME_MIRRORED: &'static str = "mirrored";
pub const CONST_NAME_DAMAGE: &'static str = "damage";
pub const CONST_NAME_INTERRUPTED: &'static str = "interrupted";
pub const CONST_NAME_TARGETS: &'static str = "targets";
pub const CONST_NAME_SPLASH_DISTANCE: &'static str = "splash_distance";
pub const CONST_NAME_ATTACK_PRIORITY: &'static str = "attack_priority";

pub const CONST_NAME_PLAYER: &'static str = "player";
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
    crate::tags::TagPackage::new().register_into_engine(&mut engine);
    // https://rhai.rs/book/safety/max-stmt-depth.html
    // ran into problems with the debug-build defaults 32, 16
    engine.set_max_expr_depths(64, 32);
    // https://rhai.rs/book/safety/max-call-stack.html
    // 8 should be enough
    engine.set_max_call_levels(8);
    engine.on_print(|s| {
        crate::debug!("RHAI-PRINT '{s}'");
    });
    engine.on_debug(|s, src, pos| {
        crate::debug!("RHAI-DEBUG of {} at {pos:?}: '{s}'", src.unwrap_or("-"));
    });
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
        rhai_action_data::ActionDataPackage4,
        crate::terrain::rhai_terrain::TerrainPackage4,
        crate::map::rhai_point::PositionPackage4,
        crate::map::rhai_direction::DirectionPackage4,
        crate::map::rhai_board::BoardPackage4,
        crate::units::rhai_unit::UnitPackage4,
        crate::units::rhai_movement::MovementPackage4,
        crate::commander::rhai_commander::CommanderPackage,
        crate::units::hero::rhai_hero::HeroPackage4,
        crate::tokens::rhai_token::TokenPackage4,
        crate::combat::rhai_combat::CombatPackage4,
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
        rhai_action_data::ActionDataPackage6,
        crate::terrain::rhai_terrain::TerrainPackage6,
        crate::map::rhai_point::PositionPackage6,
        crate::map::rhai_direction::DirectionPackage6,
        crate::map::rhai_board::BoardPackage6,
        crate::units::rhai_unit::UnitPackage6,
        crate::units::rhai_movement::MovementPackage6,
        crate::commander::rhai_commander::CommanderPackage,
        crate::units::hero::rhai_hero::HeroPackage6,
        crate::tokens::rhai_token::TokenPackage6,
        crate::combat::rhai_combat::CombatPackage6,
        crate::game::rhai_event_handler::EventHandlerPackage6 {}
}
