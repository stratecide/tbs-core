use std::any::{type_name, Any};
use std::cell::RefCell;

use rhai::*;

use crate::config::environment::Environment;

pub struct Executor {
    engine: Engine,
    scope: RefCell<Scope<'static>>,
    environment: Environment,
}

impl Executor {
    pub fn new(engine: Engine, scope: Scope<'static>, environment: Environment) -> Self {
        Self {
            engine,
            scope: RefCell::new(scope),
            environment,
        }
    }

    pub fn run<T: Any>(&self, function_index: usize, args: impl FuncArgs) -> Result<T, Box<EvalAltResult>> {
        let mut scope = self.scope.borrow_mut();
        Self::execute(&self.environment, &self.engine, &mut scope, function_index, args)
    }

    pub fn execute<T: Any>(environment: &Environment, engine: &Engine, scope: &mut Scope, function_index: usize, args: impl FuncArgs) -> Result<T, Box<EvalAltResult>> {
        let (ast, name) = environment.rhai_function_name(engine, function_index);
        let options = CallFnOptions::new().eval_ast(false).rewind_scope(true);
        let result: Dynamic = engine.call_fn_with_options(options, scope, &ast, name, args)?;
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
