use crate::map::direction::Direction;
use crate::player::Owner;

use super::ArmorType;
use super::Hp;

use zipper::*;
use zipper::zipper_derive::*;




#[derive(Debug, PartialEq, Clone, Zippable)]
pub struct Structure<D: Direction> {
    pub typ: Structures::<D>,
    pub owner: Option::<Owner>,
    pub hp: Hp,
    pub exhausted: bool,
}

#[derive(Debug, PartialEq, Clone, Zippable)]
#[zippable(bits = 4)]
pub enum Structures<D: Direction> {
    Cannon(D),
}
impl<D: Direction> Structures<D> {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Cannon(_) => "Cannon",
        }
    }
    pub fn get_armor(&self) -> (ArmorType, f32) {
        match self {
            Self::Cannon(_) => (ArmorType::Heavy, 2.5),
        }
    }
    pub fn value(&self) -> u16 {
        match self {
            Self::Cannon(_) => 500,
        }
    }
}

