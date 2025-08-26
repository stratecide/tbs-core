use std::collections::HashSet;
use std::error::Error;
use executor::Executor;
use rhai::Scope;
use rustc_hash::FxHashMap as HashMap;

use crate::commander::commander_type::CommanderType;
use crate::config::parse::*;
use crate::game::event_handler::EventHandler;
use crate::game::game::Game;
use crate::game::game_view::GameView;
use crate::handle::Handle;
use crate::map::direction::Direction;
use crate::map::point::Point;
use crate::script::*;
use crate::tags::FlagKey;
use crate::units::hero::HeroMap;
use crate::units::UnitData;

use super::file_loader::{FileLoader, TableLine};
use super::terrain_powered::TerrainFilter;
use super::token_filter::TokenFilter;
use super::unit_filter::UnitFilter;
use super::ConfigParseError;

#[derive(Debug)]
pub struct GlobalEventConfig {
    pub(crate) typ: GlobalEventType,
    pub(crate) on_start_turn: Option<usize>,
    pub(crate) on_end_turn: Option<usize>,
}

impl TableLine for GlobalEventConfig {
    type Header = GlobalEventConfigHeader;
    fn parse(data: &HashMap<Self::Header, &str>, loader: &mut FileLoader) -> Result<Self, Box<dyn Error>> {
        use GlobalEventConfigHeader as H;
        use ConfigParseError as E;
        let get = |key| {
            data.get(&key).ok_or(E::MissingColumn(format!("{key:?}")))
        };
        let typ = match get(H::Type)?.trim().to_lowercase().as_str() {
            "global" => GlobalEventType::Global(parse_vec_def(data, H::Filter, Vec::new(), loader)?),
            "unit" => GlobalEventType::Unit(parse_vec_def(data, H::Filter, Vec::new(), loader)?),
            "token" => GlobalEventType::Token(parse_vec_def(data, H::Filter, Vec::new(), loader)?),
            "terrain" => GlobalEventType::Terrain(parse_vec_def(data, H::Filter, Vec::new(), loader)?),
            e => return Err(E::UnknownEnumMember(format!("GlobalEventType::{e}")).into())
        };
        Ok(Self {
            typ,
            on_start_turn: match data.get(&H::StartTurn) {
                Some(s) if s.len() > 0 => Some(loader.rhai_function(s, 0..=0)?.index),
                _ => None,
            },
            on_end_turn: match data.get(&H::EndTurn) {
                Some(s) if s.len() > 0 => Some(loader.rhai_function(s, 0..=0)?.index),
                _ => None,
            },
        })
    }

    fn simple_validation(&self) -> Result<(), Box<dyn Error>> {
        Ok(())
    }
}

crate::listable_enum! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum GlobalEventConfigHeader {
        Type,
        Filter,
        StartTurn,
        EndTurn,
    }
}

#[derive(Debug)]
pub(crate) enum GlobalEventType {
    Global(Vec<GlobalFilter>),
    Terrain(Vec<TerrainFilter>),
    Token(Vec<TokenFilter>),
    Unit(Vec<UnitFilter>),
}

impl GlobalEventType {
    pub fn test_global<D: Direction>(&self, game: &Handle<Game<D>>) -> Option<Scope<'static>> {
        match self {
            Self::Global(filter) => {
                let environment = game.environment();
                let engine = environment.get_engine_board(game);
                let mut scope = Scope::new();
                scope.push_constant(CONST_NAME_OWNER_ID, game.with(|g| g.current_player().get_owner_id()) as i32);
                let executor = Executor::new(engine, scope.clone(), environment);
                if game.with(|g| filter.iter().all(|filter| filter.check(g, &executor))) {
                    return Some(scope);
                }
            }
            _ => ()
        }
        None
    }

    pub fn test_local<D: Direction>(&self, handler: &mut EventHandler<D>, pos: Point, heroes: &HeroMap<D>) -> Vec<Scope<'static>> {
        let current_owner_id = handler.get_game().current_owner() as i32;
        let mut result = Vec::new();
        match self {
            Self::Global(_) => (),
            Self::Terrain(filter) => {
                let game = &*handler.get_game();
                let terrain = game.get_terrain(pos).unwrap();
                let heroes = heroes.get(pos, terrain.get_owner_id());
                let environment = game.environment();
                let engine = environment.get_engine_board(game);
                let mut scope = Scope::new();
                scope.push_constant(CONST_NAME_POSITION, pos);
                scope.push_constant(CONST_NAME_TERRAIN, terrain.clone());
                scope.push_constant(CONST_NAME_OWNER_ID, current_owner_id);
                let executor = Executor::new(engine, scope.clone(), environment);
                if filter.iter().all(|filter| filter.check(game, pos, &terrain, heroes, &executor)) {
                    result.push(scope)
                }
            }
            Self::Token(filter) => {
                let game = &*handler.get_game();
                for token in game.get_tokens(pos) {
                    let environment = game.environment();
                    let engine = environment.get_engine_board(game);
                    let mut scope = Scope::new();
                    scope.push_constant(CONST_NAME_POSITION, pos);
                    scope.push_constant(CONST_NAME_TOKEN, token.clone());
                    scope.push_constant(CONST_NAME_OWNER_ID, current_owner_id);
                    let executor = Executor::new(engine, scope.clone(), environment);
                    if filter.iter().all(|filter| filter.check(game, pos, &token, &executor)) {
                        result.push(scope)
                    }
                }
            }
            Self::Unit(filter) => {
                let Some(unit) = handler.get_game().get_unit(pos) else {
                    return result
                };
                if filter.iter().all(|filter| filter.check(&*handler.get_game(), UnitData {
                    unit: &unit,
                    pos,
                    unload_index: None,
                    ballast: &[],
                    original_transporter: None,
                }, None, heroes, false)) {
                    let mut scope = Scope::new();
                    scope.push_constant(CONST_NAME_UNIT, unit.clone());
                    scope.push_constant(CONST_NAME_UNIT_ID, handler.observe_unit(pos, None));
                    scope.push_constant(CONST_NAME_POSITION, pos);
                    scope.push_constant(CONST_NAME_TRANSPORT_INDEX, ());
                    scope.push_constant(CONST_NAME_TRANSPORTER, ());
                    scope.push_constant(CONST_NAME_OWNER_ID, current_owner_id);
                    result.push(scope)
                }
                for (i, u) in unit.get_transported().iter().enumerate() {
                    if filter.iter().all(|filter| filter.check(&*handler.get_game(), UnitData {
                        unit: u,
                        pos,
                        unload_index: Some(i),
                        ballast: &[],
                        original_transporter: Some((&unit, pos)),
                    }, None, heroes, false)) {
                        let mut scope = Scope::new();
                        scope.push_constant(CONST_NAME_UNIT, u.clone());
                        scope.push_constant(CONST_NAME_UNIT_ID, handler.observe_unit(pos, Some(i)));
                        scope.push_constant(CONST_NAME_POSITION, pos);
                        scope.push_constant(CONST_NAME_TRANSPORT_INDEX, i as i32);
                        scope.push_constant(CONST_NAME_TRANSPORTER, unit.clone());
                        scope.push_constant(CONST_NAME_OWNER_ID, current_owner_id);
                        result.push(scope)
                    }
                }
            }
        }
        result
    }
}


#[derive(Debug, Clone)]
pub(crate) enum GlobalFilter {
    Rhai(usize),
    Commander(CommanderType, Option<u8>),
    Alive,
    Round(Vec<u32>),
    Flag(HashSet<FlagKey>),
    Not(Vec<Self>),
}

impl FromConfig for GlobalFilter {
    fn from_conf<'a>(s: &'a str, loader: &mut FileLoader) -> Result<(Self, &'a str), ConfigParseError> {
        let (base, mut remainder) = string_base(s);
        Ok((match base {
            "Rhai" | "Script" => {
                let (name, r) = parse_tuple1::<String>(remainder, loader)?;
                remainder = r;
                Self::Rhai(loader.rhai_function(&name, 0..=0)?.index)
            }
            "Commander" | "Co" => {
                if let Ok((commander, power, r)) = parse_tuple2(remainder, loader) {
                    remainder = r;
                    Self::Commander(commander, Some(power))
                } else {
                    let (commander, r) = parse_tuple1(remainder, loader)?;
                    remainder = r;
                    Self::Commander(commander, None)
                }
            }
            "Alive" => Self::Alive,
            "Round" => {
                let (list, r) = parse_inner_vec::<u32>(remainder, true, loader)?;
                remainder = r;
                Self::Round(list.into_iter().collect())
            }
            "Flag" | "F" => {
                let (list, r) = parse_inner_vec::<FlagKey>(remainder, true, loader)?;
                remainder = r;
                Self::Flag(list.into_iter().collect())
            }
            "Not" => {
                let (list, r) = parse_inner_vec::<Self>(remainder, true, loader)?;
                remainder = r;
                Self::Not(list)
            }
            _ => return Err(ConfigParseError::UnknownEnumMember(format!("GlobalFilter::{s}")))
        }, remainder))
    }
}

impl GlobalFilter {
    pub fn check<D: Direction>(
        &self,
        game: &Game<D>,
        executor: &Executor,
    ) -> bool {
        match self {
            Self::Rhai(function_index) => {
                match executor.run(*function_index, ()) {
                    Ok(result) => result,
                    Err(_e) => {
                        // TODO: log error
                        false
                    }
                }
            }
            Self::Commander(commander_type, power) => {
                let commander = &game.current_player().commander;
                commander.typ() == *commander_type
                && (power.is_none() || power.clone().unwrap() as usize == commander.get_active_power())
            }
            Self::Alive => !game.current_player().dead,
            Self::Round(rounds) => rounds.contains(&(game.current_turn() as u32 / game.players.len() as u32)),
            Self::Flag(flags) => {
                let player = game.current_player();
                flags.iter().any(|flag| player.has_flag(flag.0))
            }
            Self::Not(negated) => {
                // returns true if at least one check returns false
                // if you need all checks to return false, put them into separate Self::Not wrappers instead
                negated.iter()
                .any(|negated| !negated.check(game, executor))
            }
        }
    }
}
