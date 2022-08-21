use crate::player::*;

use zipper::*;
use zipper::zipper_derive::*;

pub const MAX_STACK_SIZE: u32 = 4;

#[derive(Debug, PartialEq, Clone, Zippable)]
#[zippable(bits = 8)]
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

pub fn details_fog_replacement<const S: u32>(dets: &LVec<Detail, S>) -> LVec<Detail, S> {
    let dets: Vec<Detail> = dets.into_iter().flat_map(|det| {
        det.fog_replacement()
    }).collect();
    LVec::try_from(dets).unwrap()
}
