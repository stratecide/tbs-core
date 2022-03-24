use crate::player::Owner;

#[derive(Debug, PartialEq, Clone)]
pub enum UnitType {
    Normal(NormalUnit),
}

#[derive(Debug, PartialEq, Clone)]
pub enum NormalUnits {
    Hovercraft,
    TransportHeli(Vec<NormalUnit>),
}

#[derive(Debug, PartialEq, Clone)]
pub struct NormalUnit {
    pub typ: NormalUnits,
    pub owner: Owner,
    pub hp: u8,
    pub exhausted: bool,
}
impl NormalUnit {
    pub fn new_instance(from: NormalUnits, color_id: u8) -> UnitType {
        UnitType::Normal(NormalUnit {
            typ: from,
            owner: color_id,
            hp: 100,
            exhausted: false,
        })
    }
}
