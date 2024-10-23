use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use rhai_action_data::*;
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
use crate::units::hero::HeroInfluence;
use crate::units::movement::{Path, TBallast};
use crate::units::unit::Unit;

use super::executor::Executor;
use super::*;

pub type CustomAction = (Option<usize>, usize);

/*pub enum CustomActionDataType {
    Point,
    Direction,
    UnitType,
}*/

#[derive(Debug, Clone, PartialEq, Zippable)]
#[zippable(bits=5, support_ref = Environment)]
pub enum CustomActionData<D: Direction> {
    Point(Point),
    Direction(D),
    Unit(Unit<D>),
}

impl<D: Direction> CustomActionData<D> {
    fn into_dynamic(&self) -> Dynamic {
        match self {
            Self::Point(value) => Dynamic::from(*value),
            Self::Direction(value) => Dynamic::from(*value),
            Self::Unit(value) => Dynamic::from(value.clone()),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum CustomActionDataOptions<D: Direction> {
    Point(HashSet<Point>),
    Direction(Point, HashSet<D>),
    UnitShop(Vec<(Unit<D>, i32)>),
}

impl<D: Direction> CustomActionDataOptions<D> {
    pub fn contains(&self, data: &CustomActionData<D>) -> bool {
        match (self, data) {
            (Self::Point(options), CustomActionData::Point(option)) => options.contains(option),
            (Self::Direction(_visual_center, options), CustomActionData::Direction(option)) => options.contains(option),
            (Self::UnitShop(options), CustomActionData::Unit(option)) => options.iter().any(|o| o.0 == *option),
            _ => false
        }
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
    data: &[CustomActionData<D>],
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

pub fn run_commander_input_script<D: Direction>(
    script: usize,
    game: &Handle<Game<D>>,
    data: &[CustomActionData<D>],
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
    data: &[CustomActionData<D>],
) -> CustomActionTestResult<D> {
    let index = Arc::new(Mutex::new(0));
    let result = Arc::new(Mutex::new(None));
    let result_ = result.clone();
    let invalid_data = Arc::new(Mutex::new(false));
    let data = data.to_vec();
    let environment = game.environment();
    let mut engine = environment.get_engine(game);
    if D::is_hex() {
        ActionDataPackage6::new().register_into_engine(&mut engine);
    } else {
        ActionDataPackage4::new().register_into_engine(&mut engine);
    }
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
        if !options.contains(&data[i]) {
            *invalid_data.lock().unwrap() = true;
            return Err(format!("script {script} asks for ({i}) {options:?} but received {:?}", data[i]).into());
        }
        Ok(data[i].into_dynamic())
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
                println!("is_script_input_valid: {e:?}");
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
    data: &[CustomActionData<D>],
) -> bool {
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
        false
    }
}

pub fn is_terrain_script_input_valid<D: Direction>(
    script: usize,
    game: &Handle<Game<D>>,
    pos: Point,
    terrain: Terrain<D>,
    data: &[CustomActionData<D>],
) -> bool {
    let mut scope = Scope::new();
    scope.push_constant(CONST_NAME_POSITION, pos);
    scope.push_constant(CONST_NAME_TERRAIN, terrain);
    is_script_input_valid(script, game, scope, data)
}

pub fn is_commander_script_input_valid<D: Direction>(
    script: usize,
    game: &Handle<Game<D>>,
    data: &[CustomActionData<D>],
) -> bool {
    let mut scope = Scope::new();
    scope.push_constant(CONST_NAME_OWNER_ID, game.current_owner() as i32);
    scope.push_constant(CONST_NAME_TEAM, game.current_team().to_i16() as i32);
    is_script_input_valid(script, game, scope, data)
}

fn is_script_input_valid<D: Direction>(
    script: usize,
    game: &impl GameView<D>,
    scope: Scope<'static>,
    data: &[CustomActionData<D>],
) -> bool {
    let index = Arc::new(Mutex::new(0));
    let index_ = index.clone();
    let success = Arc::new(Mutex::new(false));
    let success_ = success.clone();
    let invalid_data = Arc::new(Mutex::new(false));
    let invalid_data_ = invalid_data.clone();
    let data_len = data.len();
    let data = data.to_vec();
    let environment = game.environment();
    let mut engine = environment.get_engine(game);
    if D::is_hex() {
        ActionDataPackage6::new().register_into_engine(&mut engine);
    } else {
        ActionDataPackage4::new().register_into_engine(&mut engine);
    }
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
        if !options.contains(&data[i]) {
            *invalid_data.lock().unwrap() = true;
            return Err(format!("script {script} asks for ({i}) {options:?} but received {:?}", data[i]).into());
        }
        Ok(data[i].into_dynamic())
    });
    let executor = Executor::new(engine, scope, environment);
    match executor.run(script, ()) {
        Ok(b) => b && *index_.lock().unwrap() == data_len,
        Err(e) => {
            if *success_.lock().unwrap() {
                // early success
                true
            } else if *invalid_data_.lock().unwrap() {
                // wrong data supplied
                // TODO: log error
                false
            } else {
                // script had an error
                // TODO: log error
                println!("is_script_input_valid: {e:?}");
                false
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
    data: Option<&[CustomActionData<D>]>,
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

pub fn execute_terrain_script<D: Direction>(
    script: usize,
    handler: &mut EventHandler<D>,
    pos: Point,
    terrain: Terrain<D>,
    data: &[CustomActionData<D>],
) {
    let mut scope = Scope::new();
    scope.push_constant(CONST_NAME_POSITION, pos);
    scope.push_constant(CONST_NAME_TERRAIN, terrain);
    execute_script(script, handler, scope, Some(data))
}

pub fn execute_commander_script<D: Direction>(
    script: usize,
    handler: &mut EventHandler<D>,
    data: Option<&[CustomActionData<D>]>,
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
    data: Option<&[CustomActionData<D>]>,
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
            println!("execute_unit_script: {e}");
            handler.effect_glitch();
        }
    }
}

/*#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CustomAction {
    None, // not parsed, since this isn't valid for normal units. it's used as default for hero powers
    UnexhaustWithoutMoving,
    SummonCrystal(HeroType),
    ActivateUnits,
    BuyUnit(bool),
    SwapUnitPositions,
    Repair(u8),
}

impl FromConfig for CustomAction {
    fn from_conf(s: &str) -> Result<(Self, &str), ConfigParseError> {
        let (base, mut arguments) = string_base(s);
        Ok((match base {
            "UnexhaustWithoutMoving" => Self::UnexhaustWithoutMoving,
            "SummonCrystal" => {
                let (hero_type, remainder) = parse_tuple1(arguments)?;
                arguments = remainder;
                Self::SummonCrystal(hero_type)
            },
            "ActivateUnits" => Self::ActivateUnits,
            "BuyUnit" => {
                let (exhaust, remainder) = parse_tuple1(arguments)?;
                arguments = remainder;
                Self::BuyUnit(exhaust)
            }
            "SwapUnitPositions" => Self::SwapUnitPositions,
            "Repair" => {
                let (hp, remainder) = parse_tuple1(arguments)?;
                arguments = remainder;
                Self::Repair(1.max(99.min(hp)))
            },
            invalid => return Err(ConfigParseError::UnknownEnumMember(format!("CustomAction::{}", invalid))),
        }, arguments))
    }
}

impl CustomAction {
    pub fn next_condition<D: Direction>(
        &self,
        game: &impl GameView<D>,
        funds: i32,
        unit: &Unit<D>,
        path: &Path<D>,
        destination: Point,
        transporter: Option<(&Unit<D>, usize)>,
        heroes: &[HeroInfluence<D>],
        ballast: &[TBallast<D>],
        data_so_far: &[CustomActionData<D>],
    ) -> CustomActionTestResult<D> {
        let transporter: Option<(&Unit<D>, Point)> = transporter.map(|(u, _)| (u, path.start));
        match self {
            Self::None => CustomActionTestResult::Success,
            Self::UnexhaustWithoutMoving => {
                if path.len() == 0 {
                    CustomActionTestResult::Success
                } else {
                    CustomActionTestResult::Failure
                }
            }
            Self::SummonCrystal(_) => {
                if data_so_far.len() == 0 {
                    let options = game.get_neighbors(destination, NeighborMode::FollowPipes).into_iter()
                    .map(|op| op.point)
                    .filter(|p| {
                        game.get_unit(*p).is_none()
                    }).collect();
                    CustomActionTestResult::Next(CustomActionDataOptions::Point(options))
                } else {
                    CustomActionTestResult::Success
                }
            }
            Self::ActivateUnits => CustomActionTestResult::Success,
            Self::BuyUnit(_) => {
                let build_inside = unit.has_attribute(AttributeKey::Transported);
                let team = unit.get_team();
                if data_so_far.len() == 0 {
                    if build_inside {
                        let mut free_space = unit.remaining_transport_capacity();
                        if let Some(drone_id) = unit.get_drone_station_id() {
                            let mut outside = 0;
                            for p in game.all_points() {
                                if let Some(u) = game.get_visible_unit(team, p) {
                                    if u.get_drone_id() == Some(drone_id) {
                                        outside += 1;
                                    }
                                }
                            }
                            free_space = free_space.max(outside) - outside;
                        }
                        if free_space <= 0 {
                            return CustomActionTestResult::Failure
                        }
                    }
                    let mut options = Vec::new();
                    for unit_type in unit.transportable_units() {
                        options.push(unit.unit_shop_option(game, destination, *unit_type, transporter, heroes, ballast));
                    }
                    CustomActionTestResult::Next(CustomActionDataOptions::UnitShop(options))
                } else if data_so_far.len() == 1 {
                    let unit = match data_so_far {
                        [CustomActionData::UnitType(unit_type)] => {
                            let (unit, cost) = unit.unit_shop_option(game, destination, *unit_type, transporter, heroes, ballast);
                            if cost > funds {
                                return CustomActionTestResult::Failure;
                            }
                            unit
                        }
                        _ => return CustomActionTestResult::Failure,
                    };
                    if !build_inside {
                        let options = D::list().into_iter()
                        .filter(|d| {
                            match game.get_neighbor(destination, *d) {
                                Some((p, _)) => {
                                    game.get_terrain(p).unwrap().movement_cost(unit.default_movement_type()).is_some()
                                    && game.get_visible_unit(team, p).is_none()
                                }
                                _ => false
                            }
                        })
                        .collect();
                        CustomActionTestResult::Next(CustomActionDataOptions::Direction(destination, options))
                    } else {
                        CustomActionTestResult::Success
                    }
                } else {
                    CustomActionTestResult::Success
                }
            }
            Self::SwapUnitPositions => {
                if data_so_far.len() == 0 {
                    let options = game.get_neighbors(destination, NeighborMode::FollowPipes).into_iter()
                    .map(|op| op.point)
                    .filter(|p| {
                        *p != destination && game.get_unit(*p).is_some()
                    }).collect();
                    CustomActionTestResult::Next(CustomActionDataOptions::Point(options))
                } else if data_so_far.len() == 1 {
                    let first_point = match data_so_far[0] {
                        CustomActionData::Point(p) => p,
                        _ => return CustomActionTestResult::Failure,
                    };
                    let mut options = HashSet::new();
                    for layer in game.range_in_layers(destination, 5) {
                        for p in layer {
                            if p != first_point && p != destination && game.get_unit(p).is_some() {
                                options.insert(p);
                            }
                        }
                    }
                    CustomActionTestResult::Next(CustomActionDataOptions::Point(options))
                } else {
                    CustomActionTestResult::Success
                }
            }
            Self::Repair(_) => {
                if funds * 100 >= unit.full_price(game, destination, None, heroes) {
                    CustomActionTestResult::Success
                } else {
                    CustomActionTestResult::Failure
                }
            }
        }
    }

    pub fn is_data_valid<D: Direction>(
        &self,
        game: &impl GameView<D>,
        unit: &Unit<D>,
        path: &Path<D>,
        destination: Point,
        transporter: Option<(&Unit<D>, usize)>,
        ballast: &[TBallast<D>],
        data: &[CustomActionData<D>],
    ) -> bool {
        let funds = game.get_owning_player(unit.get_owner_id()).unwrap().funds_after_path(game, path);
        let heroes = Hero::hero_influence_at(game, destination, unit.get_owner_id());
        for i in 0..data.len() {
            use CustomActionTestResult as R;
            match self.next_condition(game, funds, unit, path, destination, transporter, &heroes, ballast, &data[..i]) {
                R::Failure => return false,
                R::Success => return i == data.len(),
                R::Next(options) => {
                    if i >= data.len() || !options.contains(&data[i]) {
                        return false;
                    }
                }
                R::NextOrSuccess(options) => {
                    if i < data.len() && !options.contains(&data[i]) {
                        return false;
                    }
                }
            }
        }
        true
    }

    pub fn execute<D: Direction>(
        &self,
        handler: &mut EventHandler<D>,
        unit: &Unit<D>,
        path: &Path<D>,
        destination: Point,
        _transporter: Option<(&Unit<D>, usize)>,
        _heroes: &[HeroInfluence<D>],
        ballast: &[TBallast<D>],
        data: &[CustomActionData<D>],
    ) {
        match self {
            Self::None => (),
            Self::UnexhaustWithoutMoving => {
                handler.unit_status(destination, ActionStatus::Ready);
            }
            Self::SummonCrystal(hero_type) => {
                let crystal_pos = match data {
                    &[CustomActionData::Point(p)] => p,
                    _ => panic!("SummonCrystal Action Data is wrong: {:?}", data),
                };
                let builder = UnitType::HeroCrystal.instance(&handler.environment())
                .set_owner_id(unit.get_owner_id())
                .set_hero(Hero::new(*hero_type, None));
                handler.unit_creation(crystal_pos, builder.build_with_defaults());
            }
            Self::ActivateUnits => {
                for p in handler.get_map().get_neighbors(destination, NeighborMode::FollowPipes) {
                    if let Some(u) = handler.get_map().get_unit(p.point) {
                        if u.get_owner_id() == unit.get_owner_id() && u.is_exhausted() {
                            handler.unit_status(p.point, ActionStatus::Ready);
                        }
                    }
                }
            }
            Self::BuyUnit(exhaust) => {
                match data {
                    &[CustomActionData::UnitType(unit_type)] => {
                        buy_transported_unit(handler, path.start, destination, unit_type, ballast, *exhaust);
                    },
                    &[CustomActionData::UnitType(unit_type), CustomActionData::Direction(dir)] => {
                        buy_unit(handler, path.start, destination, unit_type, dir, ballast, *exhaust);
                    },
                    _ => panic!("BuyUnit Action Data is wrong: {:?}", data),
                };
            }
            Self::SwapUnitPositions => {
                let (p1, p2) = match data {
                    &[CustomActionData::Point(p1), CustomActionData::Point(p2)] => (p1, p2),
                    _ => panic!("SwapUnitPositions Action Data is wrong: {:?}", data),
                };
                let unit1 = handler.get_map().get_unit(p1).cloned().unwrap();
                let unit2 = handler.get_map().get_unit(p2).cloned().unwrap();
                handler.unit_replace(p1, unit2);
                handler.unit_replace(p2, unit1);
            }
            Self::Repair(heal) => {
                let heroes = Hero::hero_influence_at(handler.get_game(), destination, unit.get_owner_id());
                let full_price = unit.full_price(handler.get_game(), destination, None, &heroes).max(0) as u32;
                let mut heal = (*heal as u32)
                    .min(100 - unit.get_hp() as u32);
                if full_price > 0 {
                    heal = heal.min(*handler.get_game().current_player().funds as u32 * 100 / full_price);
                }
                if heal > 0 {
                    let cost = full_price * heal / 100;
                    handler.money_buy(unit.get_owner_id(), cost as i32);
                    handler.unit_repair(destination, heal as u8);
                    handler.unit_status(destination, ActionStatus::Repairing);
                }
            }
        }
    }
}*/

/*pub fn buy_transported_unit<D: Direction>(handler: &mut EventHandler<D>, path_start: Point, end: Point, unit_type: UnitType, ballast: &[TBallast<D>], exhaust: bool) {
    let transporter = handler.get_map().get_unit(path_start).filter(|_| path_start != end);
    let factory_unit = handler.get_map().get_unit(end).unwrap();
    let heroes = Hero::hero_influence_at(handler.get_game(), end, factory_unit.get_owner_id());
    let (mut unit, cost) = factory_unit.unit_shop_option(handler.get_game(), end, unit_type, transporter.map(|u| (u, path_start)), &heroes, ballast);
    if !exhaust {
        unit.set_status(ActionStatus::Ready);
    }
    if handler.environment().unit_attributes(unit_type, factory_unit.get_owner_id()).any(|a| *a == AttributeKey::DroneStationId) {
        unit.set_drone_station_id(handler.get_map().new_drone_id(handler.rng()));
    }
    handler.money_buy(handler.get_game().current_player().get_owner_id(), cost);
    handler.unit_add_transported(end, unit);
}

pub fn buy_unit<D: Direction>(handler: &mut EventHandler<D>, path_start: Point, end: Point, unit_type: UnitType, dir: D, ballast: &[TBallast<D>], exhaust: bool) -> bool {
    let (destination, _) = handler.get_map().get_neighbor(end, dir).unwrap();
    if handler.get_map().get_unit(destination).is_some() {
        handler.effect_fog_surprise(destination);
        false
    } else {
        let transporter = handler.get_map().get_unit(path_start).filter(|_| path_start != end);
        let factory_unit = handler.get_map().get_unit(end).unwrap();
        let heroes = Hero::hero_influence_at(handler.get_game(), end, factory_unit.get_owner_id());
        let (mut unit, cost) = factory_unit.unit_shop_option(handler.get_game(), end, unit_type, transporter.map(|u| (u, path_start)), &heroes, ballast);
        if !exhaust {
            unit.set_status(ActionStatus::Ready);
        }
        unit.set_direction(unit.get_direction().rotate_by(dir));
        if handler.environment().unit_attributes(unit_type, factory_unit.get_owner_id()).any(|a| *a == AttributeKey::DroneStationId) {
            unit.set_drone_station_id(handler.get_map().new_drone_id(handler.rng()));
        }
        let path = Path {
            start: end,
            steps: vec![PathStep::Dir(dir)].try_into().unwrap(),
        };
        handler.money_buy(handler.get_game().current_player().get_owner_id(), cost);
        let unit = handler.animate_unit_path(&unit, &path, false);
        handler.unit_creation(destination, unit);
        true
    }
}*/
