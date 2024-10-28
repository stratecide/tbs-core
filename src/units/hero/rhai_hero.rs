use rhai::*;
use rhai::plugin::*;

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
}

def_package! {
    pub HeroPackage(module)
    {
        combine_with_exported_module!(module, "hero_type_module", hero_type_module);
    } |> |_engine| {
    }
}
