use rhai::*;
use rhai::plugin::*;
use rustc_hash::FxHashMap;
use num_rational::Rational32;

use super::event_handler::EventHandler;
use crate::map::direction::*;
use crate::map::point::*;
use crate::units::unit::Unit;
use crate::units::combat::*;
use crate::units::movement::*;
use crate::terrain::terrain::Terrain;
use crate::tags::*;

macro_rules! event_handler_module {
    ($pack: ident, $name: ident, $d: ty) => {
        #[export_module]
        mod $name {
            pub type Handler = EventHandler<$d>;

            #[rhai_fn(pure)]
            pub fn get_unit_position(handler: &mut Handler, id: usize) -> Dynamic {
                handler.get_observed_unit_pos(id)
                .map(|(p, _)| Dynamic::from(p))
                .unwrap_or(().into())
            }

            #[rhai_fn(pure)]
            pub fn get_unit_transport_index(handler: &mut Handler, id: usize) -> Dynamic {
                handler.get_observed_unit_pos(id)
                .and_then(|(_, index)| index)
                .map(|index| Dynamic::from(index as i32))
                .unwrap_or(().into())
            }

            pub fn make_player_lose(mut handler: Handler, owner_id: i32) {
                if owner_id < 0 || owner_id > i8::MAX as i32 {
                    return;
                }
                handler.player_dies(owner_id as i8)
            }

            pub fn gain_money(mut handler: Handler, owner_id: i32, amount: i32) {
                if owner_id < 0 || owner_id > i8::MAX as i32 || amount <= 0 {
                    return;
                }
                let owner_id = owner_id as i8;
                if handler.with_game(|game| game.get_owning_player(owner_id).map(|player| !player.dead).unwrap_or(false)) {
                    handler.money_income(owner_id, amount)
                }
            }

            pub fn spend_money(mut handler: Handler, owner_id: i32, amount: i32) {
                if owner_id < 0 || owner_id > i8::MAX as i32 {
                    return;
                }
                let owner_id = owner_id as i8;
                if handler.with_game(|game| game.get_owning_player(owner_id).is_some()) {
                    handler.money_buy(owner_id, amount);
                }
            }

            pub fn set_unit_flag(mut handler: Handler, position: Point, flag: FlagKey) {
                if handler.with_map(|map| map.get_unit(position).is_none()) {
                    return;
                }
                handler.set_unit_flag(position, flag.0);
            }
            pub fn remove_unit_flag(mut handler: Handler, position: Point, flag: FlagKey) {
                if handler.with_map(|map| map.get_unit(position).is_none()) {
                    return;
                }
                handler.remove_unit_flag(position, flag.0);
            }

            pub fn set_unit_tag(mut handler: Handler, position: Point, key: TagKey, value: Dynamic) {
                if handler.with_map(|map| map.get_unit(position).is_none()) {
                    return;
                }
                let Some(value) = TagValue::from_dynamic(value, key.0, &handler.environment()) else {
                    return;
                };
                handler.set_unit_tag(position, key.0, value);
            }

            /*pub fn heal_unit(mut handler: Handler, position: Point, amount: i32) {
                if amount > 0 && handler.with_map(|map| map.get_unit(position).is_some()) {
                    handler.unit_heal(position, amount.min(100) as u8);
                }
            }

            pub fn damage_unit(mut handler: Handler, position: Point, amount: i32) {
                if amount > 0 && handler.with_map(|map| map.get_unit(position).is_some()) {
                    handler.unit_damage(position, amount.min(999) as u16);
                }
            }

            pub fn damage_units(mut handler: Handler, map: FxHashMap<Point, i32>) {
                let map = map.into_iter()
                    .filter(|(_, damage)| *damage > 0)
                    .map(|(p, damage)| (p, damage.min(999) as u16))
                    .collect();
                handler.unit_mass_damage(&map);
            }
            pub fn heal_units(mut handler: Handler, map: FxHashMap<Point, i32>) {
                let map = map.into_iter()
                    .filter(|(_, heal)| *heal > 0)
                    .map(|(p, heal)| (p, heal.min(100) as u8))
                    .collect();
                handler.unit_mass_heal(map);
            }*/

            #[rhai_fn(name = "sneak_attack")]
            pub fn sneak_attack(mut handler: Handler, vector: AttackVector<$d>, p: Point, attacker: Unit<$d>, factor: Rational32, attacker_id: usize) {
                vector.execute(
                    &mut handler,
                    p,
                    attacker,
                    Some(attacker_id),
                    None,
                    false,
                    false,
                    true,
                    factor,
                    Counter::NoCounter,
                );
            }
            #[rhai_fn(name = "sneak_attack")]
            pub fn sneak_attack2(handler: Handler, vector: AttackVector<$d>, p: Point, attacker: Unit<$d>, factor: i32, attacker_id: usize) {
                sneak_attack(handler, vector, p, attacker, Rational32::from_integer(factor), attacker_id)
            }
            #[rhai_fn(name = "sneak_attack")]
            pub fn sneak_attack3(mut handler: Handler, vector: AttackVector<$d>, p: Point, attacker: Unit<$d>, factor: Rational32) {
                vector.execute(
                    &mut handler,
                    p,
                    attacker,
                    None,
                    None,
                    false,
                    false,
                    true,
                    factor,
                    Counter::NoCounter,
                );
            }
            #[rhai_fn(name = "sneak_attack")]
            pub fn sneak_attack4(handler: Handler, vector: AttackVector<$d>, p: Point, attacker: Unit<$d>, factor: i32) {
                sneak_attack3(handler, vector, p, attacker, Rational32::from_integer(factor))
            }

            /*pub fn set_unit_status(mut handler: Handler, position: Point, status: ActionStatus) {
                if handler.with_map(|map| map.get_unit(position).is_some()) {
                    handler.unit_status(position, status);
                }
            }*/

            pub fn place_unit(mut handler: Handler, position: Point, unit: Unit<$d>) {
                if handler.with_map(|map| map.get_unit(position).is_none()) {
                    handler.unit_creation(position, unit);
                }
            }

            pub fn take_unit(mut handler: Handler, position: Point) -> Dynamic {
                if let Some(unit) = handler.with_map(|map| map.get_unit(position).cloned()) {
                    handler.unit_remove(position);
                    Dynamic::from(unit)
                } else {
                    ().into()
                }
            }

            pub fn transport_unit(mut handler: Handler, position: Point, unit: Unit<$d>) {
                if handler.with_map(|map| map.get_unit(position)
                .map(|transporter| transporter.can_transport(&unit))
                .unwrap_or(false)) {
                    handler.unit_add_transported(position, unit);
                }
            }

            #[rhai_fn(name = "move_unit")]
            pub fn move_unit(mut handler: Handler, path: Path<$d>, involuntary: bool) {
                if handler.with_map(|map| map.get_unit(path.start).is_none()) {
                    return;
                }
                let Ok(end) = path.end(&*handler.get_game()) else {
                    return;
                };
                if handler.with_map(|map| map.get_unit(end.0).is_some()) {
                    return;
                }
                handler.unit_path(None, &path, false, involuntary);
            }
            #[rhai_fn(name = "move_unit")]
            pub fn move_unit2(handler: Handler, path: Path<$d>) {
                move_unit(handler, path, false);
            }

            /*pub fn set_unit_level(mut handler: Handler, position: Point, level: i32) {
                let level = level.max(0).min(handler.environment().config.max_unit_level() as i32) as u8;
                let has_attribute = handler.with_map(|map| {
                    map.get_unit(position)
                    .map(|unit| unit.has_attribute(AttributeKey::Level))
                    .unwrap_or(false)
                });
                if has_attribute {
                    handler.unit_level(position, level);
                }
            }

            pub fn set_terrain_anger(mut handler: Handler, position: Point, anger: i32) {
                let anger = anger.max(0).min(handler.environment().config.terrain_max_anger() as i32) as u8;
                let has_attribute = handler.with_map(|map| {
                    map.get_terrain(position).expect(&format!("Missing terrain at {:?}", position))
                    .has_attribute(TerrainAttributeKey::Anger)
                });
                if has_attribute {
                    handler.terrain_anger(position, anger);
                }
            }*/

            pub fn replace_terrain(mut handler: Handler, position: Point, terrain: Terrain<$d>) {
                handler.terrain_replace(position, terrain);
            }

            /*pub fn place_skull(mut handler: Handler, position: Point, of_unit: Unit<$d>, owner_id: i32) {
                let environment = handler.environment();
                if owner_id < 0 || owner_id >= environment.config.max_player_count() as i32 {
                    return;
                }
                let owner_id = owner_id as i8;
                if environment.unit_attributes(of_unit.typ(), owner_id).any(|a| *a == AttributeKey::Zombified) {
                    handler.detail_add(position, Detail::Skull(SkullData::new(&of_unit, owner_id)));
                }
            }
            pub fn remove_skull(mut handler: Handler, position: Point, owner_id: i32) {
                if let Some(i) = handler.with_map(|map| {
                    map.get_details(position).iter()
                    .enumerate()
                    .find(|(_, detail)| {
                        matches!(detail, Detail::Skull(skull) if skull.get_owner_id() as i32 == owner_id)
                    }).map(|(i, _)| i)
                }) {
                    handler.detail_remove(position, i);
                }
            }

            pub fn place_sludge(mut handler: Handler, position: Point, owner_id: i32, counter: i32) {
                let environment = handler.environment();
                if owner_id < 0 || owner_id >= environment.config.max_player_count() as i32 {
                    return;
                }
                let owner_id = owner_id as i8;
                let counter = counter.max(0).min(environment.config.max_sludge() as i32) as u8;
                handler.detail_add(position, Detail::SludgeToken(SludgeToken::new(&environment.config, owner_id, counter)));
            }
            pub fn remove_sludge(mut handler: Handler, position: Point, owner_id: i32) {
                if let Some(i) = handler.with_map(|map| {
                    map.get_details(position).iter()
                    .enumerate()
                    .find(|(_, detail)| {
                        matches!(detail, Detail::SludgeToken(token) if token.get_owner_id() as i32 == owner_id)
                    }).map(|(i, _)| i)
                }) {
                    handler.detail_remove(position, i);
                }
            }*/
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
    #[rhai_fn()]
    pub fn new_mass_heal() -> FxHashMap<Point, i32> {
        FxHashMap::default()
    }

    #[rhai_fn(pure)]
    pub fn len(map: &mut FxHashMap<Point, i32>) -> i32 {
        map.len() as i32
    }

    #[rhai_fn(pure)]
    pub fn contains(map: &mut FxHashMap<Point, i32>, p: Point) -> bool {
        map.contains_key(&p)
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
