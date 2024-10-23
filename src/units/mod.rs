pub mod commands;
pub mod movement;
pub mod rhai_movement;
pub mod combat;
pub mod rhai_combat;
//pub mod attributes;
pub mod hero;
pub mod unit_types;
pub mod unit;
pub mod rhai_unit;
#[cfg(test)]
pub(crate) mod test;

use zipper::*;

pub type Hp = U<100>;

crate::listable_enum! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum UnitVisibility {
        Stealth,
        Normal,
        AlwaysVisible,
    }
}
