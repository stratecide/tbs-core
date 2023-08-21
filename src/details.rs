use std::collections::HashMap;

use crate::game::fog::FogIntensity;
use crate::game::game::Game;
use crate::map::direction::Direction;
use crate::map::point::Point;
use crate::player::*;
use crate::units::normal_units::NormalUnits;

use zipper::zipper_derive::*;

pub const MAX_STACK_SIZE: u32 = 4;

#[derive(Debug, PartialEq, Clone, Zippable)]
#[zippable(bits = 8)]
pub enum Detail {
    Coins1,
    Coins2,
    Coins4,
    AirportBubble(Owner),
    FactoryBubble(Owner),
    PortBubble(Owner),
    Skull(Owner, NormalUnits),
}
impl Detail {
    pub fn get_vision<D: Direction>(&self, game: &Game<D>, pos: Point, team: Perspective) -> HashMap<Point, FogIntensity> {
        let mut result = HashMap::new();
        match self {
            Self::AirportBubble(owner) |
            Self::FactoryBubble(owner) |
            Self::PortBubble(owner) => {
                if let Some(player) = game.get_owning_player(*owner) {
                    if Some(player.team) == team {
                        result.insert(pos, FogIntensity::TrueSight);
                    }
                }
            }
            _ => ()
        }
        result
    }

    pub fn fog_replacement(&self, intensity: FogIntensity) -> Option<Self> {
        match intensity {
            FogIntensity::NormalVision |
            FogIntensity::TrueSight => {
                Some(self.clone())
            }
            FogIntensity::Light |
            FogIntensity::Dark => {
                match self {
                    _ => Some(self.clone())
                }
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
                Self::AirportBubble(_) |
                Self::PortBubble(_) |
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

