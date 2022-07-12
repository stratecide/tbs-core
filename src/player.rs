use std::collections::HashMap;

pub type Owner = u8;

pub type Team = u8;

pub type Perspective = Option<Team>;

#[derive(Debug, Clone, PartialEq)]
pub struct Player {
    pub color_id: u8,
    pub owner_id: Owner,
    pub team: Team,
    pub income: i16,
    pub funds: i32, // may not be consistent during fog
}
impl Player {
    pub fn new(id: u8, team: Team, income: i16, funds: i32) -> Self {
        Self {
            color_id: id,
            owner_id: id,
            team,
            income,
            funds,
        }
    }
}

impl Player {
}

