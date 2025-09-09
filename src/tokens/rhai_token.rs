use rhai::*;
use rhai::plugin::*;

use crate::config::environment::Environment;
use crate::map::direction::*;
use crate::tags::*;
use crate::units::unit::*;

use super::*;
use super::token_types::TokenType;
#[export_module]
mod token_type_module {

    pub type TokenType = crate::tokens::token_types::TokenType;

    #[rhai_fn(pure, name = "TokenType")]
    pub fn new_token_type(environment: &mut Environment, name: &str) -> Dynamic {
        environment.config.find_token_by_name(name)
        .map(Dynamic::from)
        .unwrap_or(().into())
    }

    #[rhai_fn(pure, name = "==")]
    pub fn eq(u1: &mut TokenType, u2: TokenType) -> bool {
        *u1 == u2
    }
    #[rhai_fn(pure, name = "!=")]
    pub fn neq(u1: &mut TokenType, u2: TokenType) -> bool {
        *u1 != u2
    }
}

macro_rules! token_module {
    ($pack: ident, $name: ident, $d: ty) => {
        #[export_module]
        mod $name {
            pub type Token = super::token::Token<$d>;

            #[rhai_fn(pure, name = "Token")]
            pub fn new_token(environment: &mut Environment, typ: TokenType) -> Token {
                Token::new(environment.clone(), typ)
            }

            #[rhai_fn(pure, get = "type")]
            pub fn get_type(token: &mut Token) -> TokenType {
                token.typ()
            }

            #[rhai_fn(pure, get="owner_id")]
            pub fn get_owner_id(token: &mut Token) -> i32 {
                token.get_owner_id() as i32
            }
            #[rhai_fn(set = "owner_id")]
            pub fn set_owner_id(token: &mut Token, owner_id: i32) {
                token.set_owner_id(owner_id.max(-1).min(token.environment().config.max_player_count() as i32) as i8)
            }

            #[rhai_fn(pure, get = "team")]
            pub fn get_team(token: &mut Token) -> i32 {
                token.get_team().to_i16() as i32
            }

            pub fn copy_from(token: &mut Token, other: Token) {
                token.copy_from(&other.get_tag_bag());
            }
            #[rhai_fn(name = "copy_from")]
            pub fn copy_from2(token: &mut Token, other: Unit<$d>) {
                token.copy_from(&other.get_tag_bag());
            }

            #[rhai_fn(pure, name = "has")]
            pub fn has_flag(token: &mut Token, flag: FlagKey) -> bool {
                token.has_flag(flag.0)
            }
            #[rhai_fn(name = "set")]
            pub fn set_flag(token: &mut Token, flag: FlagKey) {
                token.set_flag(flag.0)
            }
            #[rhai_fn(name = "remove")]
            pub fn remove_flag(token: &mut Token, flag: FlagKey) {
                token.remove_flag(flag.0)
            }

            #[rhai_fn(pure, name = "has")]
            pub fn has_tag(token: &mut Token, tag: TagKey) -> bool {
                token.get_tag(tag.0).is_some()
            }
            #[rhai_fn(pure, name = "get")]
            pub fn get_tag(token: &mut Token, key: TagKey) -> Dynamic {
                token.get_tag(key.0).map(|v| v.into_dynamic()).unwrap_or(().into())
            }
            #[rhai_fn(name = "set")]
            pub fn set_tag(token: &mut Token, key: TagKey, value: Dynamic) {
                if let Some(value) = TagValue::from_dynamic(value, key.0, token.environment()) {
                    token.set_tag(key.0, value);
                }
            }
            #[rhai_fn(name = "remove")]
            pub fn remove_tag(terrain: &mut Token, tag: TagKey) {
                terrain.remove_tag(tag.0)
            }
        }

        def_package! {
            pub $pack(module)
            {
                combine_with_exported_module!(module, "token_type_module", token_type_module);
                combine_with_exported_module!(module, stringify!($name), $name);
            } |> |_engine| {
            }
        }
    };
}

token_module!(TokenPackage4, token_module4, Direction4);
token_module!(TokenPackage6, token_module6, Direction6);
