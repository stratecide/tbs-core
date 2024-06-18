use std::collections::{HashMap, HashSet};

use interfaces::game_interface::ClientPerspective;
use zipper_derive::Zippable;
use zipper::{bits_needed_for_max_value, Exportable, SupportedZippable, U};

use crate::config::config::Config;
use crate::config::environment::Environment;
use crate::game::fog::FogIntensity;
use crate::game::game::Game;
use crate::game::game_view::GameView;
use crate::map::direction::Direction;
use crate::map::map::Map;
use crate::map::map_view::MapView;
use crate::map::point::Point;
use crate::map::wrapping_map::Distortion;
use crate::player::Owner;
use crate::terrain::TerrainType;
use crate::units::attributes::Attribute;
use crate::units::hero::HeroType;
use crate::units::unit::{Unit, UnitBuilder};
use crate::units::unit_types::UnitType;

pub const MAX_STACK_SIZE: u32 = 31;

crate::listable_enum! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum DetailType {
        Pipe,
        Coins1,
        Coins2,
        Coins3,
        Bubble,
        Skull,
        SludgeToken,
    }
}


#[derive(Debug, Clone, PartialEq, Eq, Zippable)]
#[zippable(bits = 4, support_ref = Environment)]
pub enum Detail<D: Direction> {
    Pipe(PipeState<D>),
    Coins1,
    Coins2,
    Coins3,
    Bubble(Owner, TerrainType),
    Skull(SkullData<D>),
    SludgeToken(SludgeToken),
}

impl<D: Direction> Detail<D> {
    pub fn typ(&self) -> DetailType {
        match self {
            Self::Pipe(_) => DetailType::Pipe,
            Self::Coins1 => DetailType::Coins1,
            Self::Coins2 => DetailType::Coins2,
            Self::Coins3 => DetailType::Coins3,
            Self::Bubble(_, _) => DetailType::Bubble,
            Self::Skull(_) => DetailType::Skull,
            Self::SludgeToken(_) => DetailType::SludgeToken,
        }
    }

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
        let max_sludge = details.iter()
        .filter_map(|d| match d {
            Self::SludgeToken(token) => token.remaining_turns(environment),
            _ => None,
        }).max()
        .unwrap_or(0);
        let details: Vec<Self> = details.into_iter().rev().filter(|detail| {
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
                    remove = bubble || pipe_directions.len() > 0 || !environment.config.terrain_can_build_base(*typ);
                    if !remove {
                        bubble = true;
                    }
                }
                Self::Coins1 | Self::Coins2 | Self::Coins3 => {
                    remove = coin || pipe_directions.len() > 0;
                    if !remove {
                        coin = true;
                    }
                }
                Self::Skull(_) => {
                    remove = skull || pipe_directions.len() > 0;
                    if !remove {
                        skull = true;
                    }
                }
                Self::SludgeToken(token) => {
                    remove = max_sludge != token.remaining_turns(environment).unwrap_or(0) || pipe_directions.len() > 0;
                }
            }
            !remove
        }).take(MAX_STACK_SIZE as usize).collect();
        details
    }

    pub fn fix_self(&mut self, map: &Map<D>, pos: Point) {
        match self {
            Self::Pipe(connection) => {
                for (i, d) in connection.directions.iter().cloned().enumerate() {
                    if let Some((point, distortion)) = map.wrapping_logic().get_neighbor(pos, d) {
                        // ends don't matter if there's no neighbor
                        connection.ends[i] = true;
                        for det in map.get_details(point) {
                            match det {
                                Self::Pipe(connection2) => {
                                    if connection2.distortion(distortion.update_direction(d)).is_some() {
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkullData<D: Direction> {
    owner: Owner,
    unit_type: UnitType,
    attributes: Vec<Attribute<D>>,
}

impl<D: Direction> SupportedZippable<&Environment> for SkullData<D> {
    fn export(&self, zipper: &mut zipper::Zipper, support: &Environment) {
        self.owner.export(zipper, support);
        self.unit_type.export(zipper, support);
        for attr in &self.attributes {
            attr.export(support, zipper, self.unit_type, false, self.owner.0, HeroType::None);
        }
    }

    fn import(unzipper: &mut zipper::Unzipper, support: &Environment) -> Result<Self, zipper::ZipperError> {
        let owner = Owner::import(unzipper, support)?;
        let unit_type = UnitType::import(unzipper, support)?;
        let mut attributes = Vec::new();
        for key in support.unit_attributes(unit_type, owner.0) {
            if key.is_skull_data(&support.config) {
                attributes.push(Attribute::import(unzipper, support, *key, unit_type, false, owner.0, HeroType::None)?);
            }
        }
        Ok(Self {
            owner,
            unit_type,
            attributes,
        })
    }
}

impl<D: Direction> SkullData<D> {
    pub fn new(unit: &Unit<D>, owner: i8) -> Self {
        let unit_type = unit.typ();
        let mut attributes = Vec::new();
        for key in unit.environment().unit_attributes(unit_type, owner) {
            if key.is_skull_data(&unit.environment().config) {
                attributes.push(unit.get_attributes().get(key).cloned().unwrap_or(key.default()));
            }
        }
        Self {
            owner: Owner(owner),
            unit_type,
            attributes,
        }
    }

    pub fn get_owner_id(&self) -> i8 {
        self.owner.0
    }

    pub fn get_unit_type(&self) -> UnitType {
        self.unit_type
    }

    pub fn unit(&self, environment: &Environment, hp: u8) -> Unit<D> {
        let mut builder = UnitBuilder::new(environment, self.unit_type)
        .set_owner_id(self.owner.0)
        .set_hp(hp)
        .set_zombified(true);
        for attr in &self.attributes {
            builder = builder.set_attribute(attr);
        }
        builder.build_with_defaults()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Zippable)]
pub struct PipeState<D: Direction> {
    directions: [D; 2],
    ends: [bool; 2],
}

impl<D: Direction> PipeState<D> {
    pub fn new(d1: D, d2: D) -> Option<Self> {
        if d1 == d2 {
            return None;
        }
        Some(Self {
            directions: [d1, d2],
            ends: [true; 2],
        })
    }

    pub fn directions(&self) -> [(D, bool); 2] {
        [
            (self.directions[0], self.ends[0]),
            (self.directions[1], self.ends[1]),
        ]
    }

    /**
     * @d: direction that leads into this PipeState
     * return: if d is a valid entry, returns Distortion to apply. None otherwise
     */
    pub fn distortion(&self, entry: D) -> Option<Distortion<D>> {
        let entry = entry.opposite_direction();
        for (i, dir) in self.directions.iter().enumerate() {
            if *dir == entry {
                return Some(Distortion::new(false, self.directions[1 - i].rotate_by(entry.opposite_direction().mirror_vertically())));
            }
        }
        None
    }
    
    pub fn is_open(&self, d: D) -> bool {
        for i in 0..self.directions.len() {
            if d == self.directions[i] {
                return self.ends[i];
            }
        }
        false
    }
}

impl<D: Direction> Default for PipeState<D> {
    fn default() -> Self {
        Self {
            directions: [D::angle_0(), D::angle_0().opposite_direction()],
            ends: [true; 2],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SludgeToken {
    owner: Owner,
    counter: u8,
}

impl SupportedZippable<&Environment> for SludgeToken {
    fn export(&self, zipper: &mut zipper::Zipper, support: &Environment) {
        self.owner.export(zipper, support);
        zipper.write_u8(self.counter, bits_needed_for_max_value(support.config.max_sludge() as u32));
    }

    fn import(unzipper: &mut zipper::Unzipper, support: &Environment) -> Result<Self, zipper::ZipperError> {
        let owner = Owner::import(unzipper, support)?;
        let max_sludge = support.config.max_sludge();
        let counter = unzipper.read_u8(bits_needed_for_max_value(max_sludge as u32))?;
        Ok(Self {
            owner,
            counter: counter.min(max_sludge),
        })
    }
}

impl SludgeToken {
    pub fn new(config: &Config, owner: i8, counter: u8) -> Self {
        Self {
            owner: owner.into(),
            counter: counter.min(config.max_sludge()),
        }
    }

    pub fn remaining_turns(&self, environment: &Environment) -> Option<usize> {
        let settings = environment.settings.as_ref()?;
        let player_index = settings.players.iter()
        .position(|p| p.get_owner_id() == self.get_owner_id())?;
        Some(player_index + settings.players.len() * self.counter.max(0) as usize)
    }

    pub fn get_owner_id(&self) -> i8 {
        self.owner.0
    }

    pub fn get_counter(&self) -> u8 {
        self.counter
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::config::config::Config;
    use crate::details::Detail;
    use crate::map::direction::*;
    use crate::map::map::*;
    use crate::map::map_view::MapView;
    use crate::map::point::*;
    use crate::map::point_map::PointMap;
    use crate::map::wrapping_map::*;
    use crate::terrain::TerrainType;

    use super::PipeState;


    #[test]
    fn pipe_state() {
        let pipe = PipeState::new(Direction4::D180, Direction4::D90).unwrap();
        assert_eq!(pipe.distortion(Direction4::D0), Some(Distortion::new(false, Direction4::D90)));
        assert_eq!(pipe.distortion(Direction4::D0).unwrap().update_direction(Direction4::D0), Direction4::D90);
        assert_eq!(pipe.distortion(Direction4::D270).unwrap().update_direction(Direction4::D270), Direction4::D180);
    }

    #[test]
    fn straight_line() {
        let config = Arc::new(Config::test_config());
        let map = PointMap::new(8, 5, false);
        let map = WMBuilder::<Direction4>::with_transformations(map, vec![Transformation::new(Distortion::new(false, Direction4::D90), Direction4::D0.translation(6))]).unwrap();
        let mut map = Map::new(map.build(), &config);
        let map_env = map.environment().clone();
        for x in 0..8 {
            for y in 0..5 {
                map.set_terrain(Point::new(x, y), TerrainType::ChessTile.instance(&map_env).build_with_defaults());
            }
        }
        map.add_detail(Point::new(7, 3), Detail::Pipe(PipeState::new(Direction4::D0, Direction4::D90).unwrap()));
        map.add_detail(Point::new(2, 4), Detail::Pipe(PipeState::new(Direction4::D180, Direction4::D90).unwrap()));
        assert_eq!(
            map.get_neighbor(Point::new(3, 0), Direction4::D90),
            Some((Point::new(7, 2), Distortion::new(false, Direction4::D0)))
        );
        assert_eq!(
            map.get_line(Point::new(3, 1), Direction4::D90, 4, NeighborMode::FollowPipes),
            vec![
                OrientedPoint::new(Point::new(3, 1), false, Direction4::D90),
                OrientedPoint::new(Point::new(3, 0), false, Direction4::D90),
                OrientedPoint::new(Point::new(7, 2), false, Direction4::D90),
                OrientedPoint::new(Point::new(7, 1), false, Direction4::D90),
            ]
        );
        assert_eq!(
            map.get_neighbor(Point::new(1, 4), Direction4::D0),
            Some((Point::new(2, 3), Distortion::new(false, Direction4::D90)))
        );
        assert_eq!(
            map.get_line(Point::new(2, 2), Direction4::D270, 4, NeighborMode::FollowPipes),
            vec![
                OrientedPoint::new(Point::new(2, 2), false, Direction4::D270),
                OrientedPoint::new(Point::new(2, 3), false, Direction4::D270),
                OrientedPoint::new(Point::new(1, 4), false, Direction4::D180),
                OrientedPoint::new(Point::new(0, 4), false, Direction4::D180),
            ]
        );
    }
}
