use rhai::*;
use rhai::plugin::*;

use crate::config::environment::Environment;
use crate::config::table_config::TableAxisKey;
use crate::config::table_config::TableValue;

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
            .map(TableValue::into_dynamic)
            .unwrap_or(().into())
    }

    #[rhai_fn(pure)]
    pub fn table_row(environment: &mut Config, name: &str, y: Dynamic, value: Dynamic) -> Array {
        let Some(y) = TableAxisKey::from_dynamic(y) else {
            return Array::new();
        };
        let Some(value) = TableValue::from_dynamic(value) else {
            return Array::new();
        };
        environment.table_row(name, y, value)
            .into_iter().map(TableAxisKey::into_dynamic)
            .collect::<Vec<_>>()
    }
    #[rhai_fn(pure)]
    pub fn table_column(environment: &mut Config, name: &str, x: Dynamic, value: Dynamic) -> Array {
        let Some(x) = TableAxisKey::from_dynamic(x) else {
            return Array::new();
        };
        let Some(value) = TableValue::from_dynamic(value) else {
            return Array::new();
        };
        environment.table_column(name, x, value)
            .into_iter().map(TableAxisKey::into_dynamic)
            .collect::<Vec<_>>()
    }

    #[rhai_fn(pure)]
    pub fn income_factor(environment: &mut Environment, owner_id: i32) -> i32 {
        environment.settings.as_ref().and_then(|settings| {
            settings.players.iter().find(|player| player.get_owner_id() as i32 == owner_id)
            .map(|player| player.get_income())
        }).unwrap_or(0)
    }
}

def_package! {
    pub EnvironmentPackage(module)
    {
        combine_with_exported_module!(module, "environment_module", environment_module);
    } |> |_engine| {
    }
}
