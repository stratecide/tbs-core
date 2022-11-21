
use crate::player::*;
use crate::map::direction::Direction;

use zipper::*;
use zipper::zipper_derive::*;

use super::*;

#[derive(Debug, PartialEq, Clone, Zippable)]
#[zippable(bits = 1)]
pub enum TransportableTypes {
    Normal(NormalUnit),
    Mercenary(Mercenary),
}
impl TransportableTypes {
    pub fn as_unit<D: Direction>(self) -> UnitType<D> {
        match self {
            Self::Normal(u) => UnitType::Normal(u),
            Self::Mercenary(u) => UnitType::Mercenary(u),
        }
    }
    pub fn as_trait<D: Direction>(&self) -> &dyn NormalUnitTrait<D> {
        match self {
            Self::Normal(u) => u.as_trait(),
            Self::Mercenary(u) => u.as_trait(),
        }
    }
    pub fn get_owner(&self) -> &Owner {
        match self {
            Self::Normal(u) => &u.owner,
            Self::Mercenary(m) => &m.unit.owner,
        }
    }
    pub fn get_hp(&self) -> u8 {
        *match self {
            Self::Normal(unit) => unit.hp,
            Self::Mercenary(unit) => unit.unit.hp,
        }
    }
    pub fn is_exhausted(&self) -> bool {
        match self {
            Self::Normal(unit) => unit.exhausted,
            Self::Mercenary(merc) => merc.unit.exhausted,
        }
    }
    pub fn remove_available_mercs(&self, mercs: &mut Vec<MercenaryOption>) {
        match self {
            Self::Mercenary(merc) => {
                if let Some(index) = mercs.iter().position(|m| m == &merc.typ.build_option()) {
                    mercs.remove(index);
                }
            }
            _ => {}
        }
    }
}
