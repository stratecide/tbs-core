use rhai::*;
use rhai::plugin::*;

use crate::map::direction::*;
use crate::units::UnitId;

#[export_module]
mod combat_module {

    pub type AttackScript = crate::combat::AttackScript;

    #[rhai_fn(name="Script")]
    pub fn new_script(function_name: ImmutableString, arguments: Array) -> AttackScript {
        AttackScript {
            function_name,
            arguments,
        }
    }
}

macro_rules! combat_module {
    ($pack: ident, $name: ident, $d: ty) => {
        #[export_module]
        mod $name {
            pub type OnDefendScript = crate::combat::OnDefendScript<$d>;

            #[rhai_fn(name="OnDefendScript")]
            pub fn new_defend_script(column_name: ImmutableString, defender_id: UnitId<$d>) -> OnDefendScript {
                OnDefendScript {
                    column_name,
                    defender_id,
                    arguments: Vec::new(),
                }
            }

            pub fn with_arguments(mut script: OnDefendScript, arguments: Array) -> OnDefendScript {
                script.arguments = arguments;
                script
            }
        }

        def_package! {
            pub $pack(module)
            {
                combine_with_exported_module!(module, "combat_module", combat_module);
                combine_with_exported_module!(module, stringify!($name), $name);
            } |> |_engine| {
            }
        }
    };
}

combat_module!(CombatPackage4, combat_module4, Direction4);
combat_module!(CombatPackage6, combat_module6, Direction6);
