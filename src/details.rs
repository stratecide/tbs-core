use crate::player::*;
use crate::units::normal_units::NormalUnits;

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
    Skull(Owner, NormalUnits),
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
            Self::Skull(_, _) => {
                Some(self.clone())
            }
        }
    }
    
    // remove Detail from value that conflict with other Detail
    // starting from the back, so add_detail can be used by the editor to overwrite previous data
    pub fn correct_stack(details: Vec<Self>) -> Vec<Self> {
        let mut bubble = false;
        let mut coin = false;
        let mut skull = false;
        let details: Vec<Self> = details.into_iter().rev().filter(|detail| {
            let remove;
            match detail {
                Self::Skull(_, _) => {
                    remove = skull;
                    skull = true;
                }
                Self::FactoryBubble(_) => {
                    remove = bubble;
                    bubble = true;
                }
                Self::Coins1 | Self::Coins2 | Self::Coins4 => {
                    remove = coin;
                    coin = true;
                }
            }
            !remove
        }).take(MAX_STACK_SIZE as usize).collect();
        details.into_iter().rev().collect()
    }
}

pub fn details_fog_replacement<const S: u32>(dets: &LVec<Detail, S>) -> LVec<Detail, S> {
    let dets: Vec<Detail> = dets.into_iter().flat_map(|det| {
        det.fog_replacement()
    }).collect();
    LVec::try_from(dets).unwrap()
}
