
use rustc_hash::FxHashSet as HashSet;

use crate::commander::commander_type::CommanderType;
use crate::config::parse::*;
use crate::game::game_view::GameView;
use crate::map::direction::Direction;
use crate::map::point::Point;
use crate::script::executor::Executor;
use crate::tags::*;
use crate::tokens::token::Token;
use crate::tokens::token_types::TokenType;

use super::file_loader::FileLoader;
use super::{ConfigParseError, OwnershipPredicate};

#[derive(Debug, Clone)]
pub(crate) enum TokenFilter {
    Rhai(usize),
    Commander(CommanderType, Option<u8>),
    Type(HashSet<TokenType>),
    Ownable,
    Unowned,
    OwnerTurn,
    Flag(HashSet<FlagKey>),
    Not(Vec<Self>),
}

impl FromConfig for TokenFilter {
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
            "T" | "Type" => {
                let (list, r) = parse_inner_vec::<TokenType>(remainder, true, loader)?;
                remainder = r;
                Self::Type(list.into_iter().collect())
            }
            "Ownable" => Self::Ownable,
            "Unowned" => Self::Unowned,
            "OwnerTurn" => Self::OwnerTurn,
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
            _ => return Err(ConfigParseError::UnknownEnumMember(format!("TokenFilter::{s}")))
        }, remainder))
    }
}

impl TokenFilter {
    pub fn check<D: Direction>(
        &self,
        game: &impl GameView<D>,
        pos: Point,
        token: &Token<D>,
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
                let commander = token.get_commander(game);
                commander.typ() == *commander_type
                && (power.is_none() || power.clone().unwrap() as usize == commander.get_active_power())
            }
            Self::Type(t) => t.contains(&token.typ()),
            Self::Ownable => token.environment().config.token_ownership(token.typ()) != OwnershipPredicate::Never,
            Self::Unowned => token.get_owner_id() < 0,
            Self::OwnerTurn => token.get_owner_id() == game.current_owner(),
            Self::Flag(flags) => flags.iter().any(|flag| token.has_flag(flag.0)),
            Self::Not(negated) => {
                // returns true if at least one check returns false
                // if you need all checks to return false, put them into separate Self::Not wrappers instead
                negated.iter()
                .any(|negated| !negated.check(game, pos, token, executor))
            }
        }
    }
}
