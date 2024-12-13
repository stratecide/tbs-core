use rhai::*;
use rhai::plugin::*;

use crate::map::direction::*;
use crate::map::point::*;
use crate::units::unit_types::UnitType;
use crate::units::hero::Hero;
use crate::script::{with_board, get_environment};
use crate::units::movement::MovementType;
use crate::units::UnitVisibility;
use crate::tags::*;
use crate::tokens::token::Token;

#[export_module]
mod unit_type_module {

    pub type UnitType = crate::units::unit_types::UnitType;
    pub type MovementType = crate::units::movement::MovementType;
    pub type UnitVisibility = crate::units::UnitVisibility;

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

    #[rhai_fn(pure, name = "==")]
    pub fn uv_eq(u1: &mut UnitVisibility, u2: UnitVisibility) -> bool {
        *u1 == u2
    }
    #[rhai_fn(pure, name = "!=")]
    pub fn uv_neq(u1: &mut UnitVisibility, u2: UnitVisibility) -> bool {
        *u1 != u2
    }
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

            pub fn copy_from(unit: &mut Unit, other: Unit) {
                unit.copy_from(other.get_tag_bag());
            }
            #[rhai_fn(name = "copy_from")]
            pub fn copy_from2(unit: &mut Unit, other: Token<$d>) {
                unit.copy_from(&other.get_tag_bag());
            }

            #[rhai_fn(pure, name = "get_custom")]
            pub fn get_custom(unit: &mut Unit, column_name: ImmutableString) -> Dynamic {
                unit.environment().unit_custom_attribute(unit.typ(), column_name)
                    .map(|result| result.into())
                    .unwrap_or(().into())
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

            #[rhai_fn(pure, get = "movement_types")]
            pub fn get_movement_types(unit: &mut Unit) -> Array {
                unit.environment().config.sub_movement_types(unit.environment().config.base_movement_type(unit.typ())).iter()
                .map(|mt| Dynamic::from(*mt))
                .collect()
            }
            #[rhai_fn(pure, get = "movement_type")]
            pub fn get_movement_type(unit: &mut Unit) -> MovementType {
                unit.sub_movement_type()
            }
            #[rhai_fn(set = "movement_type")]
            pub fn set_movement_type(unit: &mut Unit, movement_type: MovementType) {
                unit.set_sub_movement_type(movement_type)
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

            #[rhai_fn(pure, name = "visibility")]
            pub fn get_visibility(context: NativeCallContext, unit: &mut Unit, position: Point) -> UnitVisibility {
                with_board(context, |game| unit.visibility(game, position))
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
