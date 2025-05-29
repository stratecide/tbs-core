use std::collections::HashSet;
use std::fmt::Debug;
use std::sync::{Arc, Mutex};

use zipper_derive::Zippable;
use zipper::*;

use crate::config::environment::Environment;
use crate::game::event_handler::EventHandler;
use crate::game::game::Game;
use crate::game::game_view::GameView;
use crate::game::modified_view::UnitMovementView;
use crate::handle::Handle;
use crate::map::direction::Direction;
use crate::map::point::Point;
use crate::terrain::terrain::Terrain;
use crate::tokens::token::Token;
use crate::units::hero::{HeroInfluence, HeroType};
use crate::units::movement::{Path, TBallast};
use crate::units::unit::Unit;

use super::executor::Executor;
use super::*;

// Shop windows can have at most this many entries.
pub const MAXIMUM_SHOP_SIZE: usize = 50;

pub type CustomAction = (Option<usize>, usize);

#[derive(Debug, Clone, PartialEq)]
pub struct ShopItemIndex(pub usize);

impl From<usize> for ShopItemIndex {
    fn from(value: usize) -> Self {
        Self(value)
    }
}

impl Zippable for ShopItemIndex {
    fn zip(&self, zipper: &mut Zipper) {
        let max_value = MAXIMUM_SHOP_SIZE as u32 - 1;
        let bits = bits_needed_for_max_value(max_value);
        zipper.write_u32(self.0 as u32, bits);
    }
    fn unzip(unzipper: &mut Unzipper) -> Result<Self, ZipperError> {
        let max_value = MAXIMUM_SHOP_SIZE as u32 - 1;
        let bits = bits_needed_for_max_value(max_value);
        let inner = unzipper.read_u32(bits)?;
        if inner > max_value {
            return Err(ZipperError::EnumOutOfBounds(format!("ShopItemIndex({inner})")));
        }
        Ok(Self(inner as usize))
    }
}

#[derive(Debug, Clone, PartialEq, Zippable)]
#[zippable(bits=4, support_ref = Environment)]
pub enum CustomActionInput<D: Direction> {
    Point(Point),
    Direction(D),
    ShopItem(ShopItemIndex),
}

#[derive(Debug, Clone, PartialEq)]
pub enum ShopItemKey<D: Direction> {
    String(String),
    Unit(Unit<D>),
    HeroType(HeroType),
}

impl<D: Direction> ShopItemKey<D> {
    pub fn into_dynamic(&self) -> Dynamic {
        match self {
            Self::String(key) => Dynamic::from(ImmutableString::from(key)),
            Self::Unit(unit) => Dynamic::from(unit.clone()),
            Self::HeroType(key) => Dynamic::from(*key),
        }
    }

    pub fn from_dynamic(value: Dynamic) -> Option<Self> {
        match value.type_name().split("::").last().unwrap() {
            "string" => Some(Self::String(value.cast::<ImmutableString>().into_owned())),
            "Unit" => Some(Self::Unit(value.cast())),
            "HeroType" => Some(Self::HeroType(value.cast())),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ShopItem<D: Direction> {
    pub key: ShopItemKey<D>,
    pub enabled: bool,
    pub costs: Vec<Option<i32>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CustomActionData<D: Direction> {
    Point(Point),
    Direction(D),
    ShopItem(ShopItem<D>),
}

impl<D: Direction> CustomActionData<D> {
    fn into_dynamic(&self) -> Dynamic {
        match self {
            Self::Point(value) => Dynamic::from(*value),
            Self::Direction(value) => Dynamic::from(*value),
            Self::ShopItem(value) => Dynamic::from(value.clone()),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum CustomActionDataOptions<D: Direction> {
    Point(HashSet<Point>),
    Direction(Point, HashSet<D>),
    Shop(String, Vec<ShopItem<D>>),
}

impl<D: Direction> CustomActionDataOptions<D> {
    pub fn contains(&self, data: &CustomActionInput<D>) -> Option<CustomActionData<D>> {
        match (self, data) {
            (Self::Point(options), CustomActionInput::Point(option)) => {
                if options.contains(option) {
                    return Some(CustomActionData::Point(*option));
                }
            }
            (Self::Direction(_visual_center, options), CustomActionInput::Direction(option)) => {
                if options.contains(option) {
                    return Some(CustomActionData::Direction(*option));
                }
            }
            (Self::Shop(_, options), CustomActionInput::ShopItem(index)) => {
                return options.get(index.0)
                .filter(|item| item.enabled)
                .map(|item| CustomActionData::ShopItem(item.clone()));
            }
            _ => ()
        }
        None
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum CustomActionTestResult<D: Direction> {
    Success,
    // ideally only returned in the first iteration or if server received invalid data.
    // but if the script returns an error, that's also a failure (and should be logged somewhere)
    Failure,
    Next(CustomActionDataOptions<D>),
    NextOrSuccess(CustomActionDataOptions<D>),
}


pub fn run_unit_input_script<D: Direction>(
    script: usize,
    game: &impl GameView<D>,
    path: &Path<D>,
    transport_index: Option<usize>,
    data: &[CustomActionInput<D>],
) -> CustomActionTestResult<D> {
    let mut game = UnitMovementView::new(game);
    if let Some((unit_pos, unit)) = game.unit_path_without_placing(transport_index, path) {
        game.put_unit(unit_pos, unit.clone());
        let mut scope = Scope::new();
        scope.push_constant(CONST_NAME_TRANSPORTER, game.get_unit(path.start).map(|u| Dynamic::from(u)).unwrap_or(().into()));
        scope.push_constant(CONST_NAME_TRANSPORTER_POSITION, path.start);
        scope.push_constant(CONST_NAME_TRANSPORT_INDEX, transport_index.map(|i| Dynamic::from(i as i32)).unwrap_or(().into()));
        scope.push_constant(CONST_NAME_PATH, path.clone());
        scope.push_constant(CONST_NAME_UNIT, unit);
        scope.push_constant(CONST_NAME_POSITION, unit_pos);
        run_input_script(script, &game, scope, data)
    } else {
        CustomActionTestResult::Failure
    }
}

pub fn run_token_input_script<D: Direction>(
    script: usize,
    game: &Handle<Game<D>>,
    pos: Point,
    token: Token<D>,
    data: &[CustomActionInput<D>],
) -> CustomActionTestResult<D> {
    let mut scope = Scope::new();
    scope.push_constant(CONST_NAME_POSITION, pos);
    scope.push_constant(CONST_NAME_TOKEN, token);
    run_input_script(script, game, scope, data)
}

pub fn run_terrain_input_script<D: Direction>(
    script: usize,
    game: &Handle<Game<D>>,
    pos: Point,
    terrain: Terrain<D>,
    data: &[CustomActionInput<D>],
) -> CustomActionTestResult<D> {
    let mut scope = Scope::new();
    scope.push_constant(CONST_NAME_POSITION, pos);
    scope.push_constant(CONST_NAME_TERRAIN, terrain);
    run_input_script(script, game, scope, data)
}

pub fn run_commander_input_script<D: Direction>(
    script: usize,
    game: &Handle<Game<D>>,
    data: &[CustomActionInput<D>],
) -> CustomActionTestResult<D> {
    let mut scope = Scope::new();
    scope.push_constant(CONST_NAME_OWNER_ID, game.current_owner() as i32);
    scope.push_constant(CONST_NAME_TEAM, game.current_team().to_i16() as i32);
    run_input_script(script, game, scope, data)
}

fn run_input_script<D: Direction>(
    script: usize,
    game: &impl GameView<D>,
    scope: Scope<'static>,
    data: &[CustomActionInput<D>],
) -> CustomActionTestResult<D> {
    let index = Arc::new(Mutex::new(0));
    let result = Arc::new(Mutex::new(None));
    let result_ = result.clone();
    let invalid_data = Arc::new(Mutex::new(false));
    let data = data.to_vec();
    let environment = game.environment();
    let mut engine = environment.get_engine_board(game);
    engine.register_fn(FUNCTION_NAME_INPUT_CHOICE, move |options: &mut CustomActionDataOptions<D>, or_succeed: bool| -> Result<Dynamic, Box<EvalAltResult>> {
        let mut index = index.lock().unwrap();
        let i = *index;
        *index += 1;
        drop(index);
        if i >= data.len() {
            if or_succeed {
                *result.lock().unwrap() = Some(CustomActionTestResult::NextOrSuccess(options.clone()));
            } else {
                *result.lock().unwrap() = Some(CustomActionTestResult::Next(options.clone()));
            }
            return Err("Script requests Input".into());
        }
        let Some(data) = options.contains(&data[i]) else {
            *invalid_data.lock().unwrap() = true;
            return Err(format!("script {script} asks for ({i}) {options:?} but received {:?}", data[i]).into());
        };
        Ok(data.into_dynamic())
    });
    let executor = Executor::new(engine, scope, environment);
    match executor.run(script, ()) {
        Ok(true) => CustomActionTestResult::Success,
        Ok(false) => CustomActionTestResult::Failure,
        Err(e) => {
            if let Some(result) = result_.lock().unwrap().take() {
                result
            } else {
                // script had an error
                // TODO: log error
                println!("run_input_script: {e:?}");
                CustomActionTestResult::Failure
            }
        }
    }
}

pub fn is_unit_script_input_valid<D: Direction>(
    script: usize,
    game: &Handle<Game<D>>,
    path: &Path<D>,
    transport_index: Option<usize>,
    data: &[CustomActionInput<D>],
) -> Option<Vec<CustomActionData<D>>> {
    let mut game = UnitMovementView::new(game);
    if let Some((unit_pos, unit)) = game.unit_path_without_placing(transport_index, path) {
        game.put_unit(unit_pos, unit.clone());
        let mut scope = Scope::new();
        scope.push_constant(CONST_NAME_TRANSPORTER, game.get_unit(path.start).map(|u| Dynamic::from(u)).unwrap_or(().into()));
        scope.push_constant(CONST_NAME_TRANSPORTER_POSITION, path.start);
        scope.push_constant(CONST_NAME_TRANSPORT_INDEX, transport_index.map(|i| Dynamic::from(i as i32)).unwrap_or(().into()));
        scope.push_constant(CONST_NAME_PATH, path.clone());
        scope.push_constant(CONST_NAME_UNIT, unit);
        scope.push_constant(CONST_NAME_POSITION, unit_pos);
        is_script_input_valid(script, &game, scope, data)
    } else {
        None
    }
}

pub fn is_token_script_input_valid<D: Direction>(
    script: usize,
    game: &Handle<Game<D>>,
    pos: Point,
    token: Token<D>,
    data: &[CustomActionInput<D>],
) -> Option<Vec<CustomActionData<D>>> {
    let mut scope = Scope::new();
    scope.push_constant(CONST_NAME_POSITION, pos);
    scope.push_constant(CONST_NAME_TOKEN, token);
    is_script_input_valid(script, game, scope, data)
}

pub fn is_terrain_script_input_valid<D: Direction>(
    script: usize,
    game: &Handle<Game<D>>,
    pos: Point,
    terrain: Terrain<D>,
    data: &[CustomActionInput<D>],
) -> Option<Vec<CustomActionData<D>>> {
    let mut scope = Scope::new();
    scope.push_constant(CONST_NAME_POSITION, pos);
    scope.push_constant(CONST_NAME_TERRAIN, terrain);
    is_script_input_valid(script, game, scope, data)
}

pub fn is_commander_script_input_valid<D: Direction>(
    script: usize,
    game: &Handle<Game<D>>,
    data: &[CustomActionInput<D>],
) -> Option<Vec<CustomActionData<D>>> {
    let mut scope = Scope::new();
    scope.push_constant(CONST_NAME_OWNER_ID, game.current_owner() as i32);
    scope.push_constant(CONST_NAME_TEAM, game.current_team().to_i16() as i32);
    is_script_input_valid(script, game, scope, data)
}

fn is_script_input_valid<D: Direction>(
    script: usize,
    game: &impl GameView<D>,
    scope: Scope<'static>,
    data: &[CustomActionInput<D>],
) -> Option<Vec<CustomActionData<D>>> {
    let index = Arc::new(Mutex::new(0));
    let index_ = index.clone();
    let success = Arc::new(Mutex::new(false));
    let success_ = success.clone();
    let invalid_data = Arc::new(Mutex::new(false));
    let invalid_data_ = invalid_data.clone();
    let result = Arc::new(Mutex::new(Vec::new()));
    let result_ = result.clone();
    let data_len = data.len();
    let data = data.to_vec();
    let environment = game.environment();
    let mut engine = environment.get_engine_board(game);
    engine.register_fn(FUNCTION_NAME_INPUT_CHOICE, move |options: &mut CustomActionDataOptions<D>, or_succeed: bool| -> Result<Dynamic, Box<EvalAltResult>> {
        let mut index = index.lock().unwrap();
        let i = *index;
        *index += 1;
        drop(index);
        if i >= data.len() {
            if or_succeed {
                // early success
                *success.lock().unwrap() = true;
            } else {
                *invalid_data.lock().unwrap() = true;
            }
            return Err(format!("not enough data ({}) for script {script}", data.len()).into());
        }
        let Some(data) = options.contains(&data[i]) else {
            *invalid_data.lock().unwrap() = true;
            return Err(format!("script {script} asks for ({i}) {options:?} but received {:?}", data[i]).into());
        };
        result.lock().unwrap().push(data.clone());
        Ok(data.into_dynamic())
    });
    let executor = Executor::new(engine, scope, environment);
    match executor.run(script, ()) {
        Ok(b) => {
            if b && *index_.lock().unwrap() == data_len {
                // success: input script returned success and input data was used up
                Some(result_.lock().unwrap().clone())
            } else {
                // superfluous input data is an error
                None
            }
        }
        Err(e) => {
            if *success_.lock().unwrap() {
                // early success
                Some(result_.lock().unwrap().clone())
            } else if *invalid_data_.lock().unwrap() {
                // wrong data supplied
                // TODO: log error
                println!("is_script_input_valid: {e:?}");
                None
            } else {
                // script had an error
                // TODO: log error
                println!("is_script_input_valid: {e:?}");
                None
            }
        }
    }
}

pub fn execute_unit_script<D: Direction>(
    script: usize,
    handler: &mut EventHandler<D>,
    unit: &Unit<D>,
    path: &Path<D>,
    unit_pos: Point,
    transporter: Option<(&Unit<D>, usize)>,
    _heroes: &[HeroInfluence<D>],
    _ballast: &[TBallast<D>],
    data: Option<Vec<CustomActionData<D>>>,
) {
    let mut scope = Scope::new();
    scope.push_constant(CONST_NAME_TRANSPORTER, transporter.map(|(t, _)| t.clone()));
    scope.push_constant(CONST_NAME_TRANSPORTER_POSITION, path.start);
    scope.push_constant(CONST_NAME_TRANSPORT_INDEX, transporter.map(|(_, i)| i));
    scope.push_constant(CONST_NAME_PATH, path.clone());
    scope.push_constant(CONST_NAME_UNIT, unit.clone());
    scope.push_constant(CONST_NAME_POSITION, unit_pos);
    execute_script(script, handler, scope, data)
}

pub fn execute_token_script<D: Direction>(
    script: usize,
    handler: &mut EventHandler<D>,
    pos: Point,
    token: Token<D>,
    data: Vec<CustomActionData<D>>,
) {
    let mut scope = Scope::new();
    scope.push_constant(CONST_NAME_POSITION, pos);
    scope.push_constant(CONST_NAME_TOKEN, token);
    execute_script(script, handler, scope, Some(data))
}

pub fn execute_terrain_script<D: Direction>(
    script: usize,
    handler: &mut EventHandler<D>,
    pos: Point,
    terrain: Terrain<D>,
    data: Vec<CustomActionData<D>>,
) {
    let mut scope = Scope::new();
    scope.push_constant(CONST_NAME_POSITION, pos);
    scope.push_constant(CONST_NAME_TERRAIN, terrain);
    execute_script(script, handler, scope, Some(data))
}

pub fn execute_commander_script<D: Direction>(
    script: usize,
    handler: &mut EventHandler<D>,
    data: Option<Vec<CustomActionData<D>>>,
) {
    let mut scope = Scope::new();
    scope.push_constant(CONST_NAME_OWNER_ID, handler.get_game().current_owner() as i32);
    scope.push_constant(CONST_NAME_TEAM, handler.get_game().current_team().to_i16() as i32);
    execute_script(script, handler, scope, data)
}

fn execute_script<D: Direction>(
    script: usize,
    handler: &mut EventHandler<D>,
    scope: Scope<'static>,
    data: Option<Vec<CustomActionData<D>>>,
) {
    let environment = handler.get_game().environment();
    let engine = environment.get_engine_handler(handler);
    let executor = Executor::new(engine, scope, environment);
    let result: Result<(), Box<EvalAltResult>> = if let Some(data) = data {
        let data = data.iter()
        .map(CustomActionData::into_dynamic)
        .collect::<Array>();
        executor.run(script, (data, ))
    } else {
        executor.run(script, ())
    };
    match result {
        Ok(_) => (),
        Err(e) => {
            // TODO: log error
            println!("execute_script: {e}");
            handler.effect_glitch();
        }
    }
}

#[cfg(test)]
pub(crate) mod test {
    pub const CA_UNIT_BUY_HERO: usize = 0;
    pub const CA_UNIT_BUILD_UNIT: usize = 1;
    pub const CA_UNIT_CAPTURE: usize = 2;
    pub const CA_UNIT_REPAIR: usize = 3;
}
