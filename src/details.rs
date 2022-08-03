use crate::player::*;

pub const MAX_STACK_SIZE: usize = 4;

#[derive(Debug, PartialEq, Clone)]
pub enum Detail {
    Coins1,
    Coins2,
    Coins4,
    FactoryBubble(Owner),
}
impl Detail {
    pub fn fog_replacement(&self) -> Option<Self> {
        match self {
            Self::Coins1 | Self::Coins2 | Self::Coins4 => {
                Some(self.clone())
            }
            Self::FactoryBubble(_) => {
                Some(self.clone())
            }
        }
    }
}
