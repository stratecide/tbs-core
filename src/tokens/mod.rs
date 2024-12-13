pub mod token_types;
pub use token_types::TokenType;
pub mod token;
pub use token::Token;
pub mod rhai_token;
#[cfg(test)]
pub(crate) mod test;

pub const MAX_STACK_SIZE: u32 = 31;
