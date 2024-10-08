use rhai::*;
use rhai::plugin::*;

use crate::config::environment::Environment;
use crate::config::table_config::TableAxisKey;

macro_rules! environment_module {
    ($name: ident, $d: ty) => {
        #[export_module]
        mod $name {
            pub type Config = Environment;

            #[rhai_fn(pure)]
            pub fn table_entry(environment: &mut Config, name: &str, x: Dynamic, y: Dynamic) -> Dynamic {
                let Some(x) = TableAxisKey::from_dynamic(x) else {
                    return ().into();
                };
                let Some(y) = TableAxisKey::from_dynamic(y) else {
                    return ().into();
                };
                environment.table_entry(name, x, y)
                    .map(|value| value.into())
                    .unwrap_or(().into())
            }
        }
    };
}

environment_module!(environment_module4, Direction4);
environment_module!(environment_module6, Direction6);

def_package! {
    pub EnvironmentPackage(module)
    {
        combine_with_exported_module!(module, "environment_module4", environment_module4);
        combine_with_exported_module!(module, "environment_module6", environment_module6);
    } |> |_engine| {
    }
}
