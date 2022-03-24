pub type Owner = u8;

#[derive(Debug, PartialEq)]
pub struct Player {
    color_id: u8,
}

impl Player {
    pub fn color_id(&self) -> u8 {
        self.color_id
    }
}

