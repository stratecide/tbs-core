use rhai::*;
use rhai::plugin::*;

use crate::config::environment::Environment;
use crate::map::direction::*;
use crate::map::point::*;
use crate::units::unit_types::UnitType;
use crate::units::hero::*;
use crate::script::with_board;
use super::attributes::*;
use crate::units::movement::MovementType;

#[export_module]
mod unit_type_module {

    pub type UnitType = crate::units::unit_types::UnitType;
    pub type MovementType = crate::units::movement::MovementType;

    #[rhai_fn(pure, name = "==")]
    pub fn eq(u1: &mut UnitType, u2: UnitType) -> bool {
        *u1 == u2
    }
    #[rhai_fn(pure, name = "!=")]
    pub fn neq(u1: &mut UnitType, u2: UnitType) -> bool {
        *u1 != u2
    }

    #[rhai_fn(pure, name = "==")]
    pub fn mt_eq(u1: &mut MovementType, u2: MovementType) -> bool {
        *u1 == u2
    }
    #[rhai_fn(pure, name = "!=")]
    pub fn mt_neq(u1: &mut MovementType, u2: MovementType) -> bool {
        *u1 != u2
    }

    pub fn status_repairing() -> ActionStatus {
        ActionStatus::Repairing
    }
    pub fn status_exhausted() -> ActionStatus {
        ActionStatus::Exhausted
    }
    pub fn status_ready() -> ActionStatus {
        ActionStatus::Ready
    }

    #[rhai_fn(pure, name = "==")]
    pub fn as_eq(u1: &mut ActionStatus, u2: ActionStatus) -> bool {
        *u1 == u2
    }
    #[rhai_fn(pure, name = "!=")]
    pub fn as_neq(u1: &mut ActionStatus, u2: ActionStatus) -> bool {
        *u1 != u2
    }
}

macro_rules! board_module {
    ($pack: ident, $name: ident, $d: ty) => {
        #[export_module]
        mod $name {
            pub type Unit = crate::units::unit::Unit<$d>;
            pub type UnitBuilder = crate::units::unit::UnitBuilder<$d>;

            #[rhai_fn(pure, name = "==")]
            pub fn eq(p1: &mut Unit, p2: Unit) -> bool {
                *p1 == p2
            }
            #[rhai_fn(pure, name = "!=")]
            pub fn neq(u1: &mut Unit, u2: Unit) -> bool {
                *u1 != u2
            }
        
            #[rhai_fn(pure, get = "type")]
            pub fn get_type(unit: &mut Unit) -> UnitType {
                unit.typ()
            }

            #[rhai_fn(pure, get = "owner_id")]
            pub fn get_owner_id(unit: &mut Unit) -> i32 {
                unit.get_owner_id() as i32
            }

            #[rhai_fn(pure, get = "hp")]
            pub fn get_hp(unit: &mut Unit) -> i32 {
                unit.get_hp() as i32
            }

            #[rhai_fn(pure, get = "status")]
            pub fn get_status(unit: &mut Unit) -> ActionStatus {
                unit.get_status()
            }
            #[rhai_fn(set = "status")]
            pub fn set_status(unit: &mut Unit, status: ActionStatus) {
                unit.set_status(status)
            }

            #[rhai_fn(pure, name = "full_price")]
            pub fn full_price1(context: NativeCallContext, unit: &mut Unit, position: Point) -> i32 {
                with_board(context, |game| unit.full_price(game, position, None, &[]))
            }
            #[rhai_fn(pure, name = "full_price")]
            pub fn full_price2(context: NativeCallContext, unit: &mut Unit, position: Point, factory: Unit) -> i32 {
                with_board(context, |game| unit.full_price(game, position, Some(&factory), &[]))
            }

            #[rhai_fn(pure, get = "transported")]
            pub fn get_transported(unit: &mut Unit) -> Dynamic {
                if unit.has_attribute(AttributeKey::Transported) {
                    unit.get_transported().to_vec().into()
                } else {
                    ().into()
                }
            }
            #[rhai_fn(pure, get = "transported_len")]
            pub fn get_transported_len(unit: &mut Unit) -> i32 {
                unit.get_transported().len() as i32
            }
            #[rhai_fn(pure, get = "transport_capacity")]
            pub fn get_transport_capacity(unit: &mut Unit) -> i32 {
                unit.transport_capacity() as i32
            }

            #[rhai_fn(pure, get = "movement_type")]
            pub fn get_movement_type(unit: &mut Unit) -> MovementType {
                unit.default_movement_type()
            }

            #[rhai_fn(pure, name = "build_unit")]
            pub fn build_unit_name(environment: &mut Environment, name: &str) -> Dynamic {
                if let Some(unit_type) = environment.find_unit_by_name(name) {
                    build_unit(environment, unit_type)
                } else {
                    ().into()
                }
            }

            #[rhai_fn(pure)]
            pub fn build_unit(environment: &mut Environment, unit_type: UnitType) -> Dynamic {
                Dynamic::from(unit_type.instance::<$d>(environment))
            }

            #[rhai_fn(name = "copy_from")]
            pub fn builder_copy_from(builder: UnitBuilder, unit: Unit) -> UnitBuilder {
                builder.copy_from(&unit)
            }

            #[rhai_fn(name = "owner_id")]
            pub fn builder_owner_id(builder: UnitBuilder, owner_id: i32) -> UnitBuilder {
                builder.set_owner_id(owner_id.max(-1).min(i8::MAX as i32) as i8)
            }

            #[rhai_fn(name = "hero")]
            pub fn builder_hero_type(builder: UnitBuilder, name: &str) -> UnitBuilder {
                if let Some(hero_type) = builder.environment().find_hero_by_name(name) {
                    builder.set_hero(Hero::new(hero_type, None))
                } else {
                    builder
                }
            }

            #[rhai_fn(name = "hp")]
            pub fn builder_hp(builder: UnitBuilder, hp: i32) -> UnitBuilder {
                builder.set_hp(hp.max(0).min(100) as u8)
            }

            #[rhai_fn(name = "build")]
            pub fn builder_build(builder: UnitBuilder) -> Unit {
                builder.build_with_defaults()
            }
        }

        def_package! {
            pub $pack(module)
            {
                combine_with_exported_module!(module, "unit_type_module", unit_type_module);
                combine_with_exported_module!(module, stringify!($name), $name);
            } |> |_engine| {
            }
        }
    };
}

board_module!(UnitPackage4, unit_module4, Direction4);
board_module!(UnitPackage6, unit_module6, Direction6);
