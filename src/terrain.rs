use crate::player::Owner;

#[derive(Debug, PartialEq, Clone)]
pub enum Terrain {
    Grass,
    Street,
    Realty(Realty, Option<Owner>),
    Fountain,
}
impl Terrain {
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
