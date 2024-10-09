use rhai::*;
use rhai::plugin::*;

use crate::config::environment::Environment;
use crate::config::table_config::TableAxisKey;

#[export_module]
mod environment_module {
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

def_package! {
    pub EnvironmentPackage(module)
    {
        combine_with_exported_module!(module, "environment_module", environment_module);
    } |> |_engine| {
    }
}
