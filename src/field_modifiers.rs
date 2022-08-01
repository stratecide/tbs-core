use crate::player::*;

#[derive(Debug, PartialEq, Clone)]
pub enum FieldModifier {
    Coins1,
    Coins2,
    Coins4,
    FactoryBubble(Owner),
}
impl FieldModifier {
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
