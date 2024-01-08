use std::collections::{HashMap, HashSet};

use interfaces::game_interface::ClientPerspective;
use zipper_derive::Zippable;
use zipper::Exportable;

use crate::config::environment::Environment;
use crate::game::fog::FogIntensity;
use crate::game::game::Game;
use crate::map::direction::Direction;
use crate::map::map::Map;
use crate::map::point::Point;
use crate::player::Owner;
use crate::terrain::TerrainType;
use crate::units::unit_types::UnitType;

pub const MAX_STACK_SIZE: u32 = 4;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Zippable)]
#[zippable(bits = 4, support_ref = Environment)]
pub enum Detail<D: Direction> {
    Pipe(PipeState<D>),
    Coins1,
    Coins2,
    Coins4,
    Bubble(Owner, TerrainType),
    Skull(Owner, UnitType),
}
impl<D: Direction> Detail<D> {
    pub fn get_vision(&self, game: &Game<D>, pos: Point, team: ClientPerspective) -> HashMap<Point, FogIntensity> {
        let mut result = HashMap::new();
        match self {
            Self::Bubble(owner, _) => {
                if let Some(player) = game.get_owning_player(owner.0) {
                    if player.get_team() == team {
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
    pub fn correct_stack(details: Vec<Self>, environment: &Environment) -> Vec<Self> {
        let mut bubble = false;
        let mut coin = false;
        let mut skull = false;
        let mut pipe_directions = HashSet::new();
        let mut details: Vec<Self> = details.into_iter().rev().filter(|detail| {
            let remove;
            match detail {
                Self::Pipe(connection) => {
                    remove = bubble || coin || skull
                    || connection.directions[0] == connection.directions[1]
                    || connection.directions.iter().any(|d| pipe_directions.contains(d));
                    if !remove {
                        for d in connection.directions {
                            pipe_directions.insert(d);
                        }
                    }
                }
                Self::Bubble(_, typ) => {
                    remove = bubble || pipe_directions.len() > 0 || !environment.config.terrain_can_build(*typ);
                    if !remove {
                        bubble = true;
                    }
                }
                Self::Coins1 | Self::Coins2 | Self::Coins4 => {
                    remove = coin || pipe_directions.len() > 0;
                    if !remove {
                        coin = true;
                    }
                }
                Self::Skull(_, _) => {
                    remove = skull || pipe_directions.len() > 0;
                    if !remove {
                        skull = true;
                    }
                }
            }
            !remove
        }).take(MAX_STACK_SIZE as usize).collect();
        //details.sort();
        details
    }

    pub fn fix_self(&mut self, map: &Map<D>, pos: Point) {
        match self {
            Self::Pipe(connection) => {
                for (i, d) in connection.directions.iter().cloned().enumerate() {
                    if let Some(dp) = map.wrapping_logic().get_neighbor(pos, d) {
                        // ends don't matter if there's no neighbor
                        connection.ends[i] = true;
                        for det in map.get_details(dp.point) {
                            match det {
                                Self::Pipe(connection2) => {
                                    if connection2.transform_direction(dp.direction).is_some() {
                                        connection.ends[i] = false;
                                    }
                                }
                                _ => ()
                            }
                        }
                    }
                }
            }
            _ => ()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Zippable)]
pub struct PipeState<D: Direction> {
    directions: [D; 2],
    ends: [bool; 2],
}

impl<D: Direction> PipeState<D> {
    /**
     * @d: direction that leads into this PipeState
     * return: if d is a valid entry, returns Direction that leads out of this PipeState. None otherwise
     */
    pub fn transform_direction(&self, entry: D) -> Option<D> {
        let entry = entry.opposite_direction();
        for (i, dir) in self.directions.iter().enumerate() {
            if *dir == entry {
                return Some(self.directions[1 - i]);
            }
        }
        None
    }
    /*fn is_enterable(&self) -> bool;
    fn enterable_from(&self, d: D) -> bool;
    fn connections(&self) -> Vec<D>;
    fn connects_towards(&self, d: D) -> bool;
    fn connect_to(&mut self, d: D); // TODO: maybe return result depending on whether it was able to connect?
    fn next_dir(&self, entered_from: D) -> D;*/
}
