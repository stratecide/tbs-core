use crate::map::direction::Direction;
use crate::player::Owner;

use super::ArmorType;




#[derive(Debug, PartialEq, Clone)]
pub struct Structure<D: Direction> {
    pub typ: Structures<D>,
    pub owner: Option<Owner>,
    pub hp: u8,
    pub exhausted: bool,
}

#[derive(Debug, PartialEq, Clone)]
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
}

