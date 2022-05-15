use crate::{player::Owner, map::direction::Direction, units::MovementType};

#[derive(Debug, PartialEq, Clone)]
pub enum Terrain<D: Direction> {
    Grass,
    Street,
    Realty(Realty, Option<Owner>),
    Fountain,
    Pipe(D::P),
}
impl<D: Direction> Terrain<D> {
    pub fn movement_cost(&self, movement_type: &MovementType) -> Option<u8> {
        match self {
            Self::Grass => match movement_type {
                MovementType::Foot | MovementType::Treads => Some(6),
                MovementType::Heli => Some(6),
                MovementType::Hover => Some(6),
                MovementType::Wheel => Some(9),
            }
            Self::Street => Some(6),
            Self::Realty(_, _) => Some(6),
            Self::Fountain => match movement_type {
                MovementType::Heli => Some(6),
                MovementType::Hover => Some(6),
                _ => None,
            }
            Self::Pipe(_) => None,
        }
    }
    fn end_turn(&self) {
        match self {
            Terrain::Realty(realty, owner) => realty.end_turn(owner),
            _ => {}, // do nothin by default
        }
    }
}


#[derive(Debug, PartialEq, Clone)]
pub enum Realty {
    City,
    Hq,
}
impl Realty {
    fn end_turn(&self, _owner: &Option<Owner>) {

    }
}
