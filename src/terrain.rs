//pub mod point;

type Owner = Option<u8>;

pub enum Terrain {
    Grass,
    Street,
    Realty(Realty, Owner),
}
impl Terrain {
    fn end_turn(&self) {
        match self {
            Terrain::Realty(realty, owner) => realty.end_turn(owner),
            _ => {}, // do nothin by default
        }
    }
}


pub enum Realty {
    City,
}
impl Realty {
    fn end_turn(&self, _owner: &Owner) {

    }
}
