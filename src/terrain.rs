use crate::{player::Owner, map::direction::Direction};

#[derive(Debug, PartialEq, Clone)]
pub enum Terrain<D: Direction> {
    Grass,
    Street,
    Realty(Realty, Option<Owner>),
    Fountain,
    Pipe(D::P),
}
impl<D: Direction> Terrain<D> {
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
