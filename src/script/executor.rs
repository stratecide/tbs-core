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
        let (ast, name) = self.environment.rhai_function_name(&self.engine, function_index);
        let options = CallFnOptions::new().eval_ast(false).rewind_scope(true);
        let mut scope = self.scope.borrow_mut();
        let result: Dynamic = self.engine.call_fn_with_options(options, &mut scope, &ast, name, args)?;
        result.try_cast_result().map_err(|r| {
            let result_type = self.engine.map_type_name(r.type_name());
            let cast_type = match type_name::<T>() {
                typ if typ.contains("::") => self.engine.map_type_name(typ),
                typ => typ,
            };
            EvalAltResult::ErrorMismatchOutputType(cast_type.into(), result_type.into(), Position::NONE)
                .into()
        })
    }
}
