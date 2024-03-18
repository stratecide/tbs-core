pub mod commands;
pub mod movement;
pub mod combat;
pub mod attributes;
pub mod hero;
pub mod unit_types;
pub mod unit;

use zipper::*;

pub type Hp = U<100>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum UnitVisibility {
    Stealth,
    Normal,
    AlwaysVisible,
}
