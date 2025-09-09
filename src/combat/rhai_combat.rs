use std::marker::PhantomData;
use std::ptr::with_exposed_provenance_mut;
use std::rc::Rc;
use std::cell::RefCell;

use num_rational::Rational32;
use rhai::*;
use rhai::plugin::*;

use crate::game::event_handler::EventHandler;
use crate::map::direction::*;
use crate::map::map::get_unit;
use crate::script::executor::Executor;
use crate::script::CONST_NAME_ATTACK_CONTEXT;
use crate::units::{UnitData, UnitId};
use crate::combat::*;

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
            pub type Attack = crate::combat::ScriptedAttack<$d>;
            pub type AttackContext = Rc<RefCell<Option<super::AttackContextPointer<$d>>>>;

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

            #[rhai_fn(name="Attack")]
            pub fn new_attack(attacker_id: UnitId<$d>, defender_id: UnitId<$d>) -> Attack {
                Attack {
                    attacker: AttackerPosition::Real(attacker_id),
                    defender_id,
                    priority: 0,
                }
            }

            pub fn with_priority(mut script: Attack, priority: i32) -> Attack {
                script.priority = priority.max(i8::MIN as i32).min(i8::MAX as i32);
                script
            }

            #[rhai_fn(return_raw, name="remember_unit")]
            pub fn remember_unit_transported(ctx: &mut AttackContext, p: Point, unload_index: i32) -> Result<Dynamic, Box<EvalAltResult>> {
                if let Some(ctx) = &mut *ctx.borrow_mut() {
                    Ok(ctx.as_mut().remember_unit(p, Some(unload_index)))
                } else {
                    Err("AttackContext isn't supported here".into())
                }
            }
            #[rhai_fn(return_raw, name="remember_unit")]
            pub fn remember_unit(ctx: &mut AttackContext, p: Point) -> Result<Dynamic, Box<EvalAltResult>> {
                if let Some(ctx) = &mut *ctx.borrow_mut() {
                    Ok(ctx.as_mut().remember_unit(p, None))
                } else {
                    Err("AttackContext isn't supported here".into())
                }
            }

            #[rhai_fn(return_raw)]
            pub fn attacker_bonus(ctx: &mut AttackContext, defender_id: UnitId<$d>, column_id: ImmutableString, base_value: Rational32) -> Result<Rational32, Box<EvalAltResult>> {
                if let Some(ctx) = &mut *ctx.borrow_mut() {
                    Ok(ctx.as_mut().attacker_bonus(defender_id, column_id, base_value))
                } else {
                    Err("AttackContext isn't supported here".into())
                }
            }

            #[rhai_fn(return_raw)]
            pub fn defender_bonus(ctx: &mut AttackContext, defender_id: UnitId<$d>, column_id: &str, base_value: Rational32) -> Result<Rational32, Box<EvalAltResult>> {
                if let Some(ctx) = &mut *ctx.borrow_mut() {
                    Ok(ctx.as_mut().defender_bonus(defender_id, column_id, base_value))
                } else {
                    Err("AttackContext isn't supported here".into())
                }
            }

            #[rhai_fn(return_raw)]
            pub fn attack_bonus(ctx: &mut AttackContext, column_id: &str, base_value: Rational32) -> Result<Rational32, Box<EvalAltResult>> {
                if let Some(ctx) = &mut *ctx.borrow_mut() {
                    Ok(ctx.as_mut().attack_bonus(column_id, base_value))
                } else {
                    Err("AttackContext isn't supported here".into())
                }
            }

            #[rhai_fn(return_raw)]
            pub fn add_script(ctx: &mut AttackContext, attack_script: AttackScript) -> Result<(), Box<EvalAltResult>> {
                if let Some(ctx) = &mut *ctx.borrow_mut() {
                    Ok(ctx.as_mut().add_script(attack_script))
                } else {
                    Err("AttackContext isn't supported here".into())
                }
            }

            #[rhai_fn(return_raw)]
            pub fn on_defend(ctx: &mut AttackContext, defend_script: OnDefendScript) -> Result<(), Box<EvalAltResult>> {
                if let Some(ctx) = &mut *ctx.borrow_mut() {
                    Ok(ctx.as_mut().on_defend(defend_script))
                } else {
                    Err("AttackContext isn't supported here".into())
                }
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

pub(super) struct AttackContext<'a, 'c: 'a, D: Direction> {
    pub(super) handler: &'a mut EventHandler<'c, D>,
    pub(super) attack: &'a ConfiguredAttack,
    pub(super) splash: &'a AttackInstance,
    pub(super) attacker: &'a Unit<D>,
    pub(super) attacker_pos: Point,
    pub(super) attacker_ballast: &'a [TBallast<D>],
    pub(super) heroes: &'a HeroMap<D>,
    pub(super) counter_state: &'a AttackCounterState<D>,
    pub(super) scripts: Vec<(Vec<Dynamic>, Option<Rational32>, Option<Rc<AST>>, ImmutableString)>,
    pointer: Rc<RefCell<Option<AttackContextPointer<D>>>>,
}

impl<'a, 'c: 'a, D: Direction> Drop for AttackContext<'a, 'c, D> {
    fn drop(&mut self) {
        *self.pointer.borrow_mut() = None;
    }
}

impl<'a, 'c, D: Direction> AttackContext<'a, 'c, D> {
    pub(super) fn new(
        handler: &'a mut EventHandler<'c, D>,
        attack: &'a ConfiguredAttack,
        splash: &'a AttackInstance,
        attacker: &'a Unit<D>,
        attacker_pos: Point,
        attacker_ballast: &'a [TBallast<D>],
        heroes: &'a HeroMap<D>,
        counter_state: &'a AttackCounterState<D>,
    ) -> Self {
        Self {
            handler,
            attack,
            splash,
            attacker,
            attacker_pos,
            attacker_ballast,
            heroes,
            counter_state,
            scripts: Vec::new(),
            pointer: Rc::default(),
        }
    }
    pub(super) fn executor<'b>(&'b mut self, mut scope: Scope<'b>) -> Executor<'b> {
        *self.pointer.borrow_mut() = Some(AttackContextPointer::from(self));
        scope.push(CONST_NAME_ATTACK_CONTEXT, self.pointer.clone());
        self.handler.get_board().executor(scope)
    }

    fn remember_unit(&mut self, p: Point, unload_index: Option<i32>) -> Dynamic {
        let unload_index = match (self.handler.get_game().get_unit(p), unload_index) {
            (Some(_), None) => None,
            (Some(unit), Some(i)) if i >= 0 && unit.get_transported().len() > i as usize => Some(i as usize),
            _ => return ().into(),
        };
        Dynamic::from(self.handler.observe_unit(p, unload_index))
    }

    fn attacker_bonus(&self, defender_id: UnitId<D>, column_id: ImmutableString, base_value: Rational32) -> Rational32 {
        let Some((pos, unload_index)) = self.handler.get_observed_unit_pos(defender_id.0) else {
            return base_value;
        };
        let defender = get_unit(self.handler.get_board(), pos, unload_index).unwrap();
        self.handler.environment().config.unit_attack_bonus(
            &column_id.to_string(),
            base_value,
            self.handler.get_board(),
            self.attack,
            self.splash,
            self.attacker,
            self.attacker_pos,
            UnitData {
                unit: defender,
                pos,
                unload_index,
                ballast: &[],  // TODO: could have a value if counter-attack
                original_transporter: None, // TODO: could have a value if counter-attack
            },
            self.heroes,
            self.attacker_ballast,
            self.counter_state.is_counter(),
        )
    }

    fn defender_bonus(&self, defender_id: UnitId<D>, column_id: &str, base_value: Rational32) -> Rational32 {
        let Some((pos, unload_index)) = self.handler.get_observed_unit_pos(defender_id.0) else {
            return base_value;
        };
        let defender = get_unit(self.handler.get_board(), pos, unload_index).unwrap();
        let result = self.handler.environment().config.unit_defense_bonus(
            &column_id.to_string(),
            base_value,
            self.handler.get_board(),
            self.attack,
            self.splash,
            defender,
            (pos, unload_index),
            UnitData {
                unit: self.attacker,
                pos: self.attacker_pos,
                unload_index: None,
                ballast: self.attacker_ballast,
                original_transporter: None, // TODO
            },
            self.heroes,
            &[], // TODO
            self.counter_state.is_counter(),
        );
        //crate::debug!("unit_defense_bonus {column_id} = {result}");
        result
    }

    fn attack_bonus(&self, column_id: &str, base_value: Rational32) -> Rational32 {
        let result = self.handler.environment().config.attack_bonus(
            &column_id.to_string(),
            base_value,
            self.handler.get_board(),
            self.attack,
            self.splash,
            self.attacker,
            self.attacker_pos,
            None, // TODO ?
            self.heroes,
            self.attacker_ballast,
            self.counter_state,
        );
        //crate::debug!("attack_bonus {column_id} = {result}");
        result
    }

    fn add_script(&mut self, attack_script: AttackScript) {
        self.scripts.push((
            attack_script.arguments,
            None,
            None,
            attack_script.function_name,
        ));
    }

    fn on_defend(&mut self, defend_script: OnDefendScript<D>) {
        let Some((pos, unload_index)) = self.handler.get_observed_unit_pos(defend_script.defender_id.0) else {
            return;
        };
        let defender = get_unit(self.handler.get_board(), pos, unload_index).unwrap();
        let environment = self.handler.environment();
        let scripts = environment.config.on_defend_scripts(
            &defend_script.column_name.to_string(),
            defend_script.arguments.len(),
            self.handler.get_board(),
            self.attack,
            self.splash,
            self.attacker,
            self.attacker_pos,
            UnitData {
                unit: defender,
                pos,
                unload_index,
                ballast: &[], // TODO,
                original_transporter: None, // TODO
            },
            self.heroes,
            self.attacker_ballast,
            self.counter_state,
        );
        //crate::debug!("on_defend {} scripts", scripts.len());
        if scripts.len() == 0 {
            return;
        }
        for (function_index, priority) in scripts {
            let (ast, function_name) = environment.get_rhai_function(function_index);
            self.scripts.push((
                defend_script.arguments.clone(),
                priority,
                Some(ast.clone()),
                function_name.into(),
            ));
        }
    }
}

/// Newtype wrapping a reference (pointer) cast into 'usize'
#[derive(Debug, Eq, PartialEq, Clone, Hash)]
pub(super) struct AttackContextPointer<D: Direction> {
    ptr: usize,
    _pd: PhantomData<D>,
}

impl<D: Direction> AttackContextPointer<D> {
    pub(super) fn from(value: *mut AttackContext<D>) -> Self {
        let ptr = value.expose_provenance();
        Self {
            ptr,
            _pd: PhantomData,
        }
    }

    fn as_mut<'a>(&'a mut self) -> &'a mut AttackContext<'a, 'a, D> {
        let ptr: *mut AttackContext<'a, 'a, D> = with_exposed_provenance_mut(self.ptr);
        unsafe {ptr.as_mut()}
            .unwrap()
    }
}
