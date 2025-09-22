use rhai::*;
use rhai::plugin::*;

use crate::dyn_opt;
use crate::config::environment::Environment;
use crate::config::table_config::*;
use crate::units::unit_types::UnitType;

#[export_module]
mod environment_module {

    pub type Config = Environment;

    pub fn parse_int(s: ImmutableString) -> Dynamic {
        match s.parse::<i32>() {
            Ok(result) => Dynamic::from(result),
            _ => ().into()
        }
    }

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
    pub fn get_custom_value(environment: &mut Config, unit_type: UnitType, column_name: ImmutableString) -> Dynamic {
        environment.unit_custom_attribute(unit_type, column_name)
            .map(|result| result.into())
            .unwrap_or(().into())
    }

    #[rhai_fn(pure)]
    pub fn get_hero_type(environment: &mut Config, owner_id: i32) -> Dynamic {
        if owner_id < 0 || owner_id > i8::MAX as i32 {
            return ().into();
        }
        dyn_opt(environment.get_hero(owner_id as i8))
    }
}

def_package! {
    pub EnvironmentPackage(module)
    {
        combine_with_exported_module!(module, "environment_module", environment_module);
    } |> |_engine| {
    }
}
