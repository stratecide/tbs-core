use std::any::{type_name, Any};
use std::marker::PhantomData;

use rhai::*;

use crate::config::environment::Environment;
use crate::map::direction::Direction;
use crate::script::CONST_NAME_CONFIG;

pub struct Executor<'a> {
    first_argument: Map,
    environment: Environment,
    phantom: PhantomData<&'a ()>
}

impl<'a> Executor<'a> {
    pub fn new(mut first_argument: Map, environment: Environment) -> Self {
        first_argument.insert(CONST_NAME_CONFIG.into(), Dynamic::from(environment.clone()));
        Self {
            first_argument: first_argument,
            environment,
            phantom: PhantomData
        }
    }

    pub fn environment(&self) -> &Environment {
        &self.environment
    }

    pub fn run<D: Direction, T: Any>(&self, function_index: usize, additional_arguments: impl FuncArgs) -> Result<T, Box<EvalAltResult>> {
        let (ast, name) = self.environment.get_rhai_function(function_index);
        self.run_ast::<D, T>(ast, name, additional_arguments)
    }

    pub fn run_ast<D: Direction, T: Any>(&self, ast: &AST, function: impl AsRef<str>, additional_arguments: impl FuncArgs) -> Result<T, Box<EvalAltResult>> {
        let engine = self.environment.config.engine::<D>();
        let mut scope = Scope::new();
        let options = CallFnOptions::new().eval_ast(false).rewind_scope(true);
        let mut args = vec![Dynamic::from(self.first_argument.clone())];
        additional_arguments.parse(&mut args);
        let result: Dynamic = engine.call_fn_with_options(options, &mut scope, ast, function, args)?;
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
