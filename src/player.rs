pub type Owner = u8;

pub type Team = u8;

pub type Perspective = Option<Team>;

#[derive(Debug, Clone, PartialEq)]
pub struct Player {
    pub color_id: u8,
    pub owner_id: Owner,
    pub team: Team,
}

impl Player {
    pub fn color_id(&self) -> u8 {
        self.color_id
    }
    pub fn team(&self) -> u8 {
        self.team
    }
}

