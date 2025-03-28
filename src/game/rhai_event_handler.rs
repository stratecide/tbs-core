use rhai::*;
use rhai::plugin::*;
use rustc_hash::FxHashMap;

use crate::dyn_opt;
use super::event_handler::EventHandler;
use crate::map::direction::*;
use crate::map::point::*;
use crate::units::unit::Unit;
use crate::units::movement::*;
use crate::units::hero::*;
use crate::terrain::terrain::Terrain;
use crate::tags::*;
use crate::tokens::token::Token;
use crate::game::event_fx::*;

macro_rules! event_handler_module {
    ($pack: ident, $name: ident, $d: ty) => {
        #[export_module]
        mod $name {
            pub type Handler = EventHandler<$d>;
            pub type UnitId = crate::units::UnitId<$d>;

            // comparison
            //============

            #[rhai_fn(name = "==")]
            pub fn eq_ui(a: UnitId, b: UnitId) -> bool {
                a.0 == b.0
            }

            #[rhai_fn(name = "!=")]
            pub fn neq_ui(a: UnitId, b: UnitId) -> bool {
                a.0 != b.0
            }

            #[rhai_fn(pure)]
            pub fn get_unit(handler: &mut Handler, id: UnitId) -> Dynamic {
                dyn_opt(handler.get_observed_unit_pos(id.0).and_then(|(p, unload_index)| handler.with_map(|map| {
                    let mut u = map.get_unit(p)?;
                    if let Some(i) = unload_index {
                        u = u.get_transported().get(i)?;
                    }
                    Some(u.clone())
                })))
            }

            #[rhai_fn(pure)]
            pub fn get_unit_distortion(handler: &mut Handler, id: UnitId) -> Dynamic {
                handler.get_observed_unit(id.0)
                .map(|(_, _, distortion)| Dynamic::from(distortion))
                .unwrap_or(().into())
            }

            #[rhai_fn(pure)]
            pub fn get_unit_position(handler: &mut Handler, id: UnitId) -> Dynamic {
                handler.get_observed_unit_pos(id.0)
                .map(|(p, _)| Dynamic::from(p))
                .unwrap_or(().into())
            }

            #[rhai_fn(pure)]
            pub fn get_unit_transport_index(handler: &mut Handler, id: UnitId) -> Dynamic {
                handler.get_observed_unit_pos(id.0)
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

            pub fn generate_unique_id(handler: Handler, tag: TagKey) -> Dynamic {
                let environment = handler.environment();
                super::UniqueId::new(&environment, tag.0, handler.rng())
                .map(Dynamic::from)
                .unwrap_or(().into())
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
                if let Some(value) = TagValue::from_dynamic(value, key.0, &handler.environment()) {
                    handler.set_unit_tag(position, key.0, value);
                };
            }
            pub fn remove_unit_tag(mut handler: Handler, position: Point, key: TagKey) {
                if handler.with_map(|map| map.get_unit(position).is_none()) {
                    return;
                }
                handler.remove_unit_tag(position, key.0);
            }

            #[rhai_fn(name = "remove")]
            pub fn remove_unit_id_flag(mut handler: Handler, id: UnitId, key: FlagKey) {
                match handler.get_observed_unit_pos(id.0) {
                    Some((p, None)) => handler.remove_unit_flag(p, key.0),
                    Some((p, Some(unload_index))) => handler.remove_unit_flag_boarded(p, unload_index, key.0),
                    None => (),
                };
            }
            #[rhai_fn(name = "set")]
            pub fn set_unit_id_flag(mut handler: Handler, id: UnitId, key: FlagKey) {
                match handler.get_observed_unit_pos(id.0) {
                    Some((p, None)) => handler.set_unit_flag(p, key.0),
                    Some((p, Some(unload_index))) => handler.set_unit_flag_boarded(p, unload_index, key.0),
                    None => (),
                };
            }

            #[rhai_fn(pure, name = "get")]
            pub fn get_unit_id_tag(handler: &mut Handler, id: UnitId, key: TagKey) -> Dynamic {
                if let Some((p, unload_index)) = handler.get_observed_unit_pos(id.0) {
                    handler.with_map(|map| {
                        let unit = map.get_unit(p).unwrap();
                        if let Some(index) = unload_index {
                            unit.get_transported()[index].get_tag(key.0)
                        } else {
                            unit.get_tag(key.0)
                        }
                    }).map(|value| value.into_dynamic())
                    .unwrap_or(().into())
                } else {
                    ().into()
                }
            }
            #[rhai_fn(name = "set")]
            pub fn set_unit_id_tag(mut handler: Handler, id: UnitId, key: TagKey, value: Dynamic) {
                if let Some(value) = TagValue::from_dynamic(value, key.0, &handler.environment()) {
                    match handler.get_observed_unit_pos(id.0) {
                        Some((p, None)) => handler.set_unit_tag(p, key.0, value),
                        Some((p, Some(unload_index))) => handler.set_unit_tag_boarded(p, unload_index, key.0, value),
                        None => (),
                    };
                }
            }

            pub fn set_hero(mut handler: Handler, position: Point, hero: Hero) {
                if handler.with_map(|map| map.get_unit(position).is_none()) {
                    return;
                }
                handler.unit_set_hero(position, hero);
            }

            pub fn add_hero_charge(mut handler: Handler, id: UnitId, delta: i32) {
                if let Some((p, unload_index)) = handler.get_observed_unit_pos(id.0) {
                    handler.add_hero_charge(p, unload_index, delta);
                }
            }
            pub fn set_hero_charge(mut handler: Handler, id: UnitId, new_charge: i32) {
                if let Some((p, unload_index)) = handler.get_observed_unit_pos(id.0) {
                    handler.set_hero_charge(p, unload_index, new_charge);
                }
            }

            /*#[rhai_fn(name = "sneak_attack")]
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

            pub fn replace_terrain(mut handler: Handler, position: Point, terrain: Terrain<$d>) {
                handler.terrain_replace(position, terrain);
            }

            pub fn set_terrain_flag(mut handler: Handler, position: Point, flag: FlagKey) {
                handler.set_terrain_flag(position, flag.0);
            }
            pub fn remove_terrain_flag(mut handler: Handler, position: Point, flag: FlagKey) {
                handler.remove_terrain_flag(position, flag.0);
            }

            pub fn set_terrain_tag(mut handler: Handler, position: Point, key: TagKey, value: Dynamic) {
                let Some(value) = TagValue::from_dynamic(value, key.0, &handler.environment()) else {
                    return;
                };
                handler.set_terrain_tag(position, key.0, value);
            }
            pub fn remove_terrain_tag(mut handler: Handler, position: Point, key: TagKey) {
                handler.remove_terrain_tag(position, key.0);
            }

            // tokens

            pub fn place_token(mut handler: Handler, position: Point, token: Token<$d>) {
                handler.token_add(position, token);
            }
            pub fn remove_token(mut handler: Handler, position: Point, name: &str, owner_id: i32) -> Dynamic {
                if let Some((i, token)) = handler.with_map(|map| {
                    map.get_tokens(position).iter()
                    .enumerate()
                    .find(|(_, token)| {
                        token.name() == name && token.get_owner_id() as i32 == owner_id
                    }).map(|(i, token)| (i, token.clone()))
                }) {
                    handler.token_remove(position, i);
                    Dynamic::from(token)
                } else {
                    ().into()
                }
            }
            #[rhai_fn(name = "remove_token")]
            pub fn remove_token2(handler: Handler, position: Point, token: Token<$d>) -> Dynamic {
                remove_token(handler, position, token.name(), token.get_owner_id() as i32)
            }

            #[rhai_fn(name = "effect")]
            pub fn effect_global(mut handler: Handler, effect: EffectWithoutPosition<$d>) {
                handler.effect(Effect::Global(effect));
            }
            #[rhai_fn(name = "effect")]
            pub fn effect_point(mut handler: Handler, p: Point, effect: EffectWithoutPosition<$d>) {
                handler.effect(Effect::Point(effect, p));
            }
            #[rhai_fn(name = "effect")]
            pub fn effect_path(mut handler: Handler, path: Path<$d>, effect: EffectWithoutPosition<$d>) {
                let effect = {
                    let board = handler.get_game();
                    Effect::Path(EffectPath::new(&*board, effect.typ, effect.data, path))
                };
                handler.effect(effect);
            }

            pub fn effect(mut handler: Handler, effect: Effect<$d>) {
                handler.effect(effect);
            }
            pub fn effects(mut handler: Handler, effects: Array) {
                let mut list = Vec::with_capacity(effects.len());
                for effect in effects {
                    let effect = match effect.try_cast_result::<Effect<$d>>() {
                        Ok(effect) => {
                            list.push(effect);
                            continue;
                        }
                        Err(effect) => effect,
                    };
                    let _effect = match effect.try_cast_result::<EffectWithoutPosition<$d>>() {
                        Ok(effect) => {
                            list.push(Effect::Global(effect));
                            continue;
                        }
                        Err(effect) => effect,
                    };
                    // TODO: log error, add glitch effect (at most one)
                }
                handler.effects(list);
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
