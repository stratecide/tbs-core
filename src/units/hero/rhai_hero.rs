use rhai::*;
use rhai::plugin::*;

use crate::map::direction::*;
use crate::map::point::*;
use crate::script::get_environment;
use super::UnitId;

#[export_module]
mod hero_type_module {

    pub type HeroType = super::super::HeroType;
    pub type Hero = super::super::Hero;

    #[rhai_fn(pure, name = "==")]
    pub fn ht_eq(u1: &mut HeroType, u2: HeroType) -> bool {
        *u1 == u2
    }
    #[rhai_fn(pure, name = "!=")]
    pub fn ht_neq(u1: &mut HeroType, u2: HeroType) -> bool {
        *u1 != u2
    }

    #[rhai_fn(pure, name = "==")]
    pub fn h_eq(u1: &mut Hero, u2: Hero) -> bool {
        *u1 == u2
    }
    #[rhai_fn(pure, name = "!=")]
    pub fn h_neq(u1: &mut Hero, u2: Hero) -> bool {
        *u1 != u2
    }

    #[rhai_fn(name = "Hero")]
    pub fn new_hero(hero_type: HeroType) -> Hero {
        Hero::new(hero_type)
    }
    #[rhai_fn(name = "Hero")]
    pub fn new_hero_str(context: NativeCallContext, typ: &str) -> Dynamic {
        let environment = get_environment(context);
        if let Some(typ) = environment.config.find_hero_by_name(typ) {
            Dynamic::from(Hero::new(typ))
        } else {
            ().into()
        }
    }
}

macro_rules! hero_module {
    ($pack: ident, $name: ident, $d: ty) => {
        #[export_module]
        mod $name {
            pub type HeroInfluence = super::super::HeroInfluenceWithId<$d>;
            pub type HeroMap = super::super::HeroMapWithId<$d>;

            #[rhai_fn(pure, name = "get")]
            pub fn hero_map_pos_owner(map: &mut HeroMap, pos: Point, owner: i32) -> Array {
                if owner < -1 || owner > i8::MAX as i32 {
                    return Vec::new();
                }
                map.get(pos, owner as i8).iter()
                    .cloned()
                    .map(Dynamic::from)
                    .collect()
            }
            
            #[rhai_fn(pure, get = "unit_id")]
            pub fn unit_id(influence: &mut HeroInfluence) -> UnitId<$d> {
                influence.0
            }
        }

        def_package! {
            pub $pack(module)
            {
                combine_with_exported_module!(module, "hero_type_module", hero_type_module);
                combine_with_exported_module!(module, stringify!($name), $name);
            } |> |_engine| {
            }
        }
    };
}

hero_module!(HeroPackage4, hero_module4, Direction4);
hero_module!(HeroPackage6, hero_module6, Direction6);
