use zipper::*;

use crate::config::parse::FromConfig;
use crate::map::direction::Direction;
use crate::config::environment::Environment;

use super::token::Token;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TokenType(pub usize);

impl FromConfig for TokenType {
    fn from_conf<'a>(s: &'a str, loader: &mut crate::config::file_loader::FileLoader) -> Result<(Self, &'a str), crate::config::ConfigParseError> {
        let (base, s) = crate::config::parse::string_base(s);
        match loader.token_types.iter().position(|name| name.as_str() == base) {
            Some(i) => Ok((Self(i), s)),
            None => Err(crate::config::ConfigParseError::MissingToken(base.to_string()))
        }
    }
}

impl SupportedZippable<&Environment> for TokenType {
    fn export(&self, zipper: &mut Zipper, environment: &Environment) {
        let bits = bits_needed_for_max_value(environment.config.token_count() as u32 - 1);
        zipper.write_u32(self.0 as u32, bits);
    }
    fn import(unzipper: &mut Unzipper, environment: &Environment) -> Result<Self, ZipperError> {
        let bits = bits_needed_for_max_value(environment.config.token_count() as u32 - 1);
        let index = unzipper.read_u32(bits)? as usize;
        if index >= environment.config.token_count() {
            return Err(ZipperError::EnumOutOfBounds(format!("TokenType index {}", index)))
        }
        Ok(Self(index))
    }
}

impl TokenType {
    pub fn instance<D: Direction>(&self, environment: &Environment) -> Token<D> {
        Token::new(environment.clone(), *self)
    }
}
