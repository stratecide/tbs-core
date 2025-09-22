use rhai::*;
use rhai::plugin::*;

#[export_module]
mod commander_type_module {

    pub type CommanderType = super::super::commander_type::CommanderType;

    #[rhai_fn(pure, name = "==")]
    pub fn ht_eq(u1: &mut CommanderType, u2: CommanderType) -> bool {
        *u1 == u2
    }
    #[rhai_fn(pure, name = "!=")]
    pub fn ht_neq(u1: &mut CommanderType, u2: CommanderType) -> bool {
        *u1 != u2
    }

}

def_package! {
    pub CommanderPackage(module)
    {
        combine_with_exported_module!(module, "commander_type_module", commander_type_module);
    } |> |_engine| {
    }
}
