
use crate::player::*;
use crate::map::direction::Direction;

use super::*;

#[derive(Debug, PartialEq, Clone)]
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
    pub fn is_exhausted(&self) -> bool {
        match self {
            Self::Normal(unit) => unit.exhausted,
            Self::Mercenary(merc) => merc.unit.exhausted,
        }
    }
}
