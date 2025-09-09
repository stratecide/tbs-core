use std::any::{type_name, Any};
use std::cell::RefCell;

use rhai::*;

use crate::config::environment::Environment;
use crate::map::direction::Direction;
use crate::script::CONST_NAME_CONFIG;

pub struct Executor<'a> {
    scope: RefCell<Scope<'a>>,
    environment: Environment,
}

impl<'a> Executor<'a> {
    pub fn new(mut scope: Scope<'a>, environment: Environment) -> Self {
        scope.push_constant(CONST_NAME_CONFIG, environment.clone());
        Self {
            scope: RefCell::new(scope),
            environment,
        }
    }

    pub fn environment(&self) -> &Environment {
        &self.environment
    }

    pub fn run<D: Direction, T: Any>(&self, function_index: usize, args: impl FuncArgs) -> Result<T, Box<EvalAltResult>> {
        let (ast, name) = self.environment.get_rhai_function(function_index);
        self.run_ast::<D, T>(ast, name, args)
    }

    pub fn run_ast<D: Direction, T: Any>(&self, ast: &AST, function: impl AsRef<str>, args: impl FuncArgs) -> Result<T, Box<EvalAltResult>> {
        let engine = self.environment.config.engine::<D>();
        let mut scope = self.scope.borrow_mut();
        let options = CallFnOptions::new().eval_ast(false).rewind_scope(true);
        let result: Dynamic = engine.call_fn_with_options(options, &mut *scope, ast, function, args)?;
        result.try_cast_result().map_err(|r| {
            let result_type = engine.map_type_name(r.type_name());
            let cast_type = match type_name::<T>() {
                typ if typ.contains("::") => engine.map_type_name(typ),
                typ => typ,
            };
            EvalAltResult::ErrorMismatchOutputType(cast_type.into(), result_type.into(), Position::NONE)
                .into()
        })
    }
}
