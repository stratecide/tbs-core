pub type Owner = u8;

#[derive(Debug, PartialEq)]
pub struct Player {
    pub color_id: u8,
    pub owner_id: Owner,
}

impl Player {
    pub fn color_id(&self) -> u8 {
        self.color_id
    }
}

