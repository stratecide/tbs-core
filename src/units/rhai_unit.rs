use rhai::*;
use rhai::plugin::*;

use crate::map::direction::*;
use crate::map::point::*;
use crate::units::unit_types::UnitType;
use crate::units::hero::Hero;
use crate::script::{with_board, get_environment};
use crate::units::movement::MovementType;
use crate::tags::*;

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

    /*pub fn status_repairing() -> ActionStatus {
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
    }*/
}

macro_rules! board_module {
    ($pack: ident, $name: ident, $d: ty) => {
        #[export_module]
        mod $name {
            pub type Unit = crate::units::unit::Unit<$d>;

            #[rhai_fn(pure, name = "==")]
            pub fn eq(p1: &mut Unit, p2: Unit) -> bool {
                *p1 == p2
            }
            #[rhai_fn(pure, name = "!=")]
            pub fn neq(u1: &mut Unit, u2: Unit) -> bool {
                *u1 != u2
            }
        
            #[rhai_fn(name = "Unit")]
            pub fn new_unit(context: NativeCallContext, typ: UnitType) -> Unit {
                let environment = get_environment(context);
                Unit::new(environment, typ)
            }
            #[rhai_fn(name = "Unit")]
            pub fn new_unit_str(context: NativeCallContext, typ: &str) -> Dynamic {
                let environment = get_environment(context);
                if let Some(typ) = environment.config.find_unit_by_name(typ) {
                    Dynamic::from(Unit::new(environment, typ))
                } else {
                    ().into()
                }
            }

            #[rhai_fn(pure, get = "type")]
            pub fn get_type(unit: &mut Unit) -> UnitType {
                unit.typ()
            }

            #[rhai_fn(pure, get = "owner_id")]
            pub fn get_owner_id(unit: &mut Unit) -> i32 {
                unit.get_owner_id() as i32
            }
            #[rhai_fn(set = "owner_id")]
            pub fn set_owner_id(unit: &mut Unit, owner_id: i32) {
                unit.set_owner_id(owner_id.max(-1).min(unit.environment().config.max_player_count() as i32) as i8)
            }

            #[rhai_fn(pure, get = "team")]
            pub fn get_team(unit: &mut Unit) -> i32 {
                unit.get_team().to_i16() as i32
            }

            /*#[rhai_fn(pure, get = "hp")]
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

            #[rhai_fn(pure, get = "level")]
            pub fn get_level(unit: &mut Unit) -> Dynamic {
                if unit.has_attribute(AttributeKey::Level) {
                    (unit.get_level() as i32).into()
                } else {
                    ().into()
                }
            }

            #[rhai_fn(pure, get = "drone_station_id")]
            pub fn get_drone_station_id(unit: &mut Unit) -> Dynamic {
                unit.get_drone_station_id()
                .map(|id| (id as i32).into())
                .unwrap_or(().into())
            }

            #[rhai_fn(pure, get = "drone_id")]
            pub fn get_drone_id(unit: &mut Unit) -> Dynamic {
                unit.get_drone_id()
                .map(|id| (id as i32).into())
                .unwrap_or(().into())
            }*/

            pub fn copy_from(unit: &mut Unit, other: Unit) {
                unit.copy_from(&other);
            }

            #[rhai_fn(pure, name = "has")]
            pub fn has_flag(unit: &mut Unit, flag: FlagKey) -> bool {
                unit.has_flag(flag.0)
            }
            #[rhai_fn(name = "set")]
            pub fn set_flag(unit: &mut Unit, flag: FlagKey) {
                unit.set_flag(flag.0)
            }
            #[rhai_fn(name = "remove")]
            pub fn remove_flag(unit: &mut Unit, flag: FlagKey) {
                unit.remove_flag(flag.0)
            }

            #[rhai_fn(pure, name = "has")]
            pub fn has_tag(unit: &mut Unit, tag: TagKey) -> bool {
                unit.get_tag(tag.0).is_some()
            }
            #[rhai_fn(pure, name = "get")]
            pub fn get_tag(unit: &mut Unit, key: TagKey) -> Dynamic {
                unit.get_tag(key.0).map(|v| v.into_dynamic()).unwrap_or(().into())
            }
            #[rhai_fn(name = "set")]
            pub fn set_tag(unit: &mut Unit, key: TagKey, value: Dynamic) {
                if let Some(value) = TagValue::from_dynamic(value, key.0, unit.environment()) {
                    unit.set_tag(key.0, value);
                }
            }
            #[rhai_fn(name = "remove")]
            pub fn remove_tag(terrain: &mut Unit, tag: TagKey) {
                terrain.remove_tag(tag.0)
            }

            #[rhai_fn(pure, name = "value")]
            pub fn value1(context: NativeCallContext, unit: &mut Unit, position: Point) -> i32 {
                with_board(context, |game| unit.value(game, position, None, &[]))
            }
            #[rhai_fn(pure, name = "value")]
            pub fn value2(context: NativeCallContext, unit: &mut Unit, position: Point, factory: Unit) -> i32 {
                with_board(context, |game| unit.value(game, position, Some(&factory), &[]))
            }

            #[rhai_fn(pure, get = "transported")]
            pub fn get_transported(unit: &mut Unit) -> Array {
                unit.get_transported().iter()
                .map(|u| Dynamic::from(u.clone()))
                .collect()
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
                unit.sub_movement_type()
            }

            #[rhai_fn(pure, get = "hero")]
            pub fn get_hero(unit: &mut Unit) -> Dynamic {
                unit.get_hero()
                .map(|hero| Dynamic::from(hero.clone()))
                .unwrap_or(().into())
            }
            #[rhai_fn(set = "hero")]
            pub fn set_hero(unit: &mut Unit, hero: Hero) {
                unit.set_hero(hero)
            }
            #[rhai_fn(set = "hero")]
            pub fn remove_hero(unit: &mut Unit, _: ()) {
                unit.remove_hero()
            }
            #[rhai_fn(name = "remove_hero")]
            pub fn remove_hero2(unit: &mut Unit) {
                unit.remove_hero()
            }

            /*#[rhai_fn(pure)]
            pub fn build_unit(environment: &mut Environment, unit_type: UnitType) -> UnitBuilder {
                unit_type.instance::<$d>(environment)
            }
            #[rhai_fn(return_raw, pure, name = "build_unit")]
            pub fn build_unit_name(environment: &mut Environment, name: &str) -> Result<UnitBuilder, Box<EvalAltResult>> {
                if let Some(unit_type) = environment.config.find_unit_by_name(name) {
                    Ok(build_unit(environment, unit_type))
                } else {
                    Err(format!("Unknown unit type '{name}'").into())
                }
            }

            #[rhai_fn(name = "copy_from")]
            pub fn builder_copy_from(builder: UnitBuilder, unit: Unit) -> UnitBuilder {
                builder.copy_from(&unit)
            }

            #[rhai_fn(name = "owner_id")]
            pub fn builder_owner_id(builder: UnitBuilder, owner_id: i32) -> UnitBuilder {
                builder.set_owner_id(owner_id.max(-1).min(i8::MAX as i32) as i8)
            }

            #[rhai_fn(return_raw, name = "hero")]
            pub fn builder_hero_type(builder: UnitBuilder, name: &str) -> Result<UnitBuilder, Box<EvalAltResult>> {
                if let Some(hero_type) = builder.environment().config.find_hero_by_name(name) {
                    Ok(builder.set_hero(Hero::new(hero_type)))
                } else {
                    Err(format!("Unknown hero type '{name}'").into())
                }
            }

            #[rhai_fn(name = "set")]
            pub fn builder_flag(builder: UnitBuilder, flag: FlagKey) -> UnitBuilder {
                builder.set_flag(flag.0)
            }

            #[rhai_fn(name = "set")]
            pub fn builder_tag_i32(builder: UnitBuilder, key: TagKey, value: i32) -> UnitBuilder {
                builder.set_tag(key.0, value.into())
            }

            /*#[rhai_fn(name = "hp")]
            pub fn builder_hp(builder: UnitBuilder, hp: i32) -> UnitBuilder {
                builder.set_hp(hp.max(0).min(100) as u8)
            }

            #[rhai_fn(name = "zombified")]
            pub fn builder_zombified(builder: UnitBuilder, zombified: bool) -> UnitBuilder {
                builder.set_zombified(zombified)
            }
            #[rhai_fn(name = "zombified")]
            pub fn builder_zombified2(builder: UnitBuilder) -> UnitBuilder {
                builder_zombified(builder, true)
            }*/

            #[rhai_fn(name = "build")]
            pub fn builder_build(builder: UnitBuilder) -> Unit {
                builder.build_with_defaults()
            }*/
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
