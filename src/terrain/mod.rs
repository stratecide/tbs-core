pub mod attributes;
pub mod terrain;
mod test;

use std::str::FromStr;
use zipper::*;

use crate::config::environment::Environment;
use crate::config::ConfigParseError;

use self::terrain::TerrainBuilder;

pub const KRAKEN_ATTACK_RANGE: usize = 3;
pub const KRAKEN_MAX_ANGER: usize = 8;


crate::enum_with_custom! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum TerrainType {
        Airport,
        Beach,
        Bridge,
        ChessPawnTile,
        ChessTile,
        City,
        ConstructionSite,
        Factory,
        Flame,
        Forest,
        Fountain,
        Grass,
        Hill,
        Hq,
        Icebergs,
        Kraken,
        Lillypads,
        Mountain,
        OilPlatform,
        //Pipe,
        Reef,
        Ruins,
        Sea,
        ShallowSea,
        Street,
        Memorial,
        //TrashIsland,
        //Crater,
        TentacleDepths,
        Port,
    }
}

impl SupportedZippable<&Environment> for TerrainType {
    fn export(&self, zipper: &mut Zipper, support: &Environment) {
        let index = support.config.terrain_types().iter().position(|t| t == self).unwrap();
        let bits = bits_needed_for_max_value(support.config.terrain_count() as u32 - 1);
        zipper.write_u32(index as u32, bits);
    }
    fn import(unzipper: &mut Unzipper, support: &Environment) -> Result<Self, ZipperError> {
        let bits = bits_needed_for_max_value(support.config.terrain_count() as u32 - 1);
        let index = unzipper.read_u32(bits)? as usize;
        if index < support.config.terrain_count() {
            Ok(support.config.terrain_types()[index])
        } else {
            Err(ZipperError::EnumOutOfBounds(format!("TerrainType index {}", index)))
        }
    }
}

impl TerrainType {
    pub fn instance(&self, environment: &Environment) -> TerrainBuilder {
        TerrainBuilder::new(environment, *self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtraMovementOptions {
    None,
    Jump,
    PawnStart,
}

impl FromStr for ExtraMovementOptions {
    type Err = ConfigParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut it = s.split(&['(', ',', '-', ')'])
        .map(str::trim);
        Ok(match it.next().unwrap() {
            "None" => Self::None,
            "Jump" => Self::Jump,
            "PawnStart" => Self::PawnStart,
            invalid => return Err(ConfigParseError::UnknownEnumMember(invalid.to_string())),
        })
    }
}

/*macro_rules! land_units {
    () => {
        MovementType::Foot |
        MovementType::Wheel |
        MovementType::Hover(HoverMode::Land) |
        MovementType::Treads |
        MovementType::Chess
    };
}

macro_rules! sea_units {
    () => {
        MovementType::Hover(HoverMode::Sea) |
        MovementType::Boat |
        MovementType::Ship
    };
}

macro_rules! air_units {
    () => {
        MovementType::Heli |
        MovementType::Plane
    };
}


#[derive(Debug, PartialEq, Clone, Zippable)]
#[zippable(bits = 8)]
pub enum Terrain<D: Direction> {
    Beach,
    Bridge,
    ChessTile,
    Flame,
    Forest,
    Fountain,
    Grass,
    Hill,
    Icebergs,
    Lillypads,
    Mountain,
    Pipe(D::P),
    Realty(Realty, Option<Owner>, CaptureProgress), 
    Reef,
    Ruins,
    Sea,
    ShallowSea,
    Street,
    Tavern,
    ChessPawnTile,
    //TrashIsland,
    //Crater,
    TentacleDepths,
    Kraken(U<8>),
}
impl<D: Direction> Terrain<D> {
    pub fn movement_cost(&self, movement_type: MovementType) -> Option<MovementPoints> {
        match (self, movement_type) {
            (Self::Beach, MovementType::Chess) => None,
            (Self::Beach, MovementType::Ship) => None,
            (Self::Beach, _) => Some(MovementPoints::from(1.)),

            (Self::Bridge, MovementType::Chess) => None,
            (Self::Bridge, _) => Some(MovementPoints::from(1.)),

            (Self::Grass, sea_units!()) => None,
            (Self::Grass, MovementType::Hover(_)) => Some(MovementPoints::from(1.5)),
            (Self::Grass, MovementType::Wheel) => Some(MovementPoints::from(1.5)),
            (Self::Grass, land_units!()) => Some(MovementPoints::from(1.)),
            (Self::Grass, air_units!()) => Some(MovementPoints::from(1.)),
            
            (Self::Forest, sea_units!()) => None,
            (Self::Forest, MovementType::Hover(_)) => Some(MovementPoints::from(2.)),
            (Self::Forest, MovementType::Wheel) => Some(MovementPoints::from(2.)),
            (Self::Forest, MovementType::Foot) => Some(MovementPoints::from(1.)),
            (Self::Forest, land_units!()) => Some(MovementPoints::from(1.5)),
            (Self::Forest, air_units!()) => Some(MovementPoints::from(1.)),

            (Self::Hill, sea_units!()) => None,
            (Self::Hill, MovementType::Hover(_)) => Some(MovementPoints::from(2.)),
            (Self::Hill, MovementType::Wheel) => Some(MovementPoints::from(2.)),
            (Self::Hill, MovementType::Foot) => Some(MovementPoints::from(1.)),
            (Self::Hill, land_units!()) => Some(MovementPoints::from(1.5)),
            (Self::Hill, air_units!()) => Some(MovementPoints::from(1.)),

            (Self::Icebergs, MovementType::Boat) => Some(MovementPoints::from(1.5)),
            (Self::Icebergs, MovementType::Ship) => Some(MovementPoints::from(2.)),
            (Self::Icebergs, MovementType::Hover(_)) => Some(MovementPoints::from(1.5)),
            (Self::Icebergs, land_units!()) => None,
            (Self::Icebergs, air_units!()) => Some(MovementPoints::from(1.)),

            (Self::Lillypads, MovementType::Foot) => Some(MovementPoints::from(2.)),
            (Self::Lillypads, MovementType::Boat) => Some(MovementPoints::from(1.5)),
            (Self::Lillypads, MovementType::Ship) => Some(MovementPoints::from(1.)),
            (Self::Lillypads, land_units!()) => None,
            (Self::Lillypads, air_units!()) => Some(MovementPoints::from(1.)),
            (Self::Lillypads, MovementType::Hover(_)) => Some(MovementPoints::from(2.)),

            (Self::Mountain, sea_units!()) => None,
            (Self::Mountain, MovementType::Foot) => Some(MovementPoints::from(1.5)),
            (Self::Mountain, MovementType::Heli) => Some(MovementPoints::from(1.5)),
            (Self::Mountain, MovementType::Plane) => Some(MovementPoints::from(1.)),
            (Self::Mountain,
                MovementType::Hover(_) |
                MovementType::Wheel |
                MovementType::Treads |
                MovementType::Chess) => None,

            (Self::Sea, land_units!()) => None,
            (Self::Sea, MovementType::Hover(_)) => Some(MovementPoints::from(1.5)),
            (Self::Sea, _) => Some(MovementPoints::from(1.)),

            (Self::ShallowSea, land_units!()) => None,
            (Self::ShallowSea, _) => Some(MovementPoints::from(1.)),

            (Self::Reef, land_units!()) => None,
            (Self::Reef, MovementType::Ship) => None,
            (Self::Reef, MovementType::Boat) => Some(MovementPoints::from(1.5)),
            (Self::Reef, _) => Some(MovementPoints::from(1.)),

            (Self::Street, sea_units!()) => None,
            (Self::Street, _) => Some(MovementPoints::from(1.)),

            (Self::Ruins, sea_units!()) => None,
            (Self::Ruins, _) => Some(MovementPoints::from(1.)),

            (Self::Flame, _) => None,
            
            (Self::Realty(realty, _, _), movement_type) => return realty.movement_cost(movement_type),

            (Self::Tavern, MovementType::Chess) => None,
            (Self::Tavern, _) => Some(MovementPoints::from(1.)),

            (Self::Fountain, land_units!()) => None,
            (Self::Fountain, sea_units!()) => Some(MovementPoints::from(1.)),
            (Self::Fountain, MovementType::Hover(_)) => Some(MovementPoints::from(1.)),
            (Self::Fountain, MovementType::Heli) => Some(MovementPoints::from(1.5)),
            (Self::Fountain, MovementType::Plane) => Some(MovementPoints::from(1.)),

            (Self::Pipe(_), _) => None,

            (Self::ChessTile, sea_units!()) => None,
            (Self::ChessTile, MovementType::Chess) => Some(MovementPoints::from(0.)),
            (Self::ChessTile, _) => Some(MovementPoints::from(1.)),
            (Self::ChessPawnTile, sea_units!()) => None,
            (Self::ChessPawnTile, MovementType::Chess) => Some(MovementPoints::from(0.)),
            (Self::ChessPawnTile, _) => Some(MovementPoints::from(1.)),

            /*(Self::TrashIsland, land_units!()) => None,
            (Self::TrashIsland, MovementType::Boat) => Some(MovementPoints::from(1.5)),
            (Self::TrashIsland, _) => Some(MovementPoints::from(1.)),

            (Self::Crater, sea_units!()) => None,
            (Self::Crater, MovementType::Hover(_)) => Some(MovementPoints::from(2.)),
            (Self::Crater, MovementType::Foot) => Some(MovementPoints::from(1.)),
            (Self::Crater, land_units!()) => Some(MovementPoints::from(1.5)),
            (Self::Crater, air_units!()) => Some(MovementPoints::from(1.)),*/

            (Self::TentacleDepths, land_units!()) => None,
            (Self::TentacleDepths, _) => Some(MovementPoints::from(1.)),

            (Self::Kraken(_), air_units!()) => Some(MovementPoints::from(1.)),
            (Self::Kraken(_), sea_units!()) => Some(MovementPoints::from(2.)),
            (Self::Kraken(_), _) => None,
        }
    }
    pub fn is_land(&self) -> bool {
        self.movement_cost(MovementType::Foot).is_some()
    }
    pub fn is_chess(&self) -> bool {
        *self == Self::ChessPawnTile
        || *self == Self::ChessTile
    }
    pub fn is_water(&self) -> bool {
        self.movement_cost(MovementType::Boat).is_some()
    }
    pub fn like_beach_for_hovercraft(&self) -> bool {
        match self {
            Self::Beach => true,
            Self::Realty(realty, _, _) => realty.like_beach_for_hovercraft(),
            Self::Tavern => true,
            _ => false,
        }
    }
    pub fn update_movement_type(&self, movement_type: MovementType, prev_terrain: &Self) -> Option<MovementType> {
        // only sea-faring or flying units can cross between beach and bridge tiles
        if Self::Sea.movement_cost(movement_type).is_none() {
            match (prev_terrain, self) {
                (Self::Beach, Self::Bridge) |
                (Self::Bridge, Self::Beach) => {
                    return None;
                }
                _ => {}
            }
        }
        Some(match movement_type {
            MovementType::Hover(mode) => {
                if self.like_beach_for_hovercraft() {
                    MovementType::Hover(HoverMode::Beach)
                } else if mode == HoverMode::Beach {
                    MovementType::Hover(match self {
                        Self::Beach => HoverMode::Beach,
                        Self::Bridge => HoverMode::Sea,
                        Self::ChessPawnTile => HoverMode::Land,
                        Self::ChessTile => HoverMode::Land,
                        //Self::Crater => HoverMode::Land,
                        Self::Flame => HoverMode::Land,
                        Self::Forest => HoverMode::Land,
                        Self::Fountain => HoverMode::Sea,
                        Self::Grass => HoverMode::Land,
                        Self::Hill => HoverMode::Land,
                        Self::Icebergs => HoverMode::Sea,
                        Self::Lillypads => HoverMode::Sea,
                        Self::Mountain => HoverMode::Land,
                        Self::Pipe(_) => mode,
                        Self::Realty(_, _, _) => HoverMode::Land,
                        Self::Reef => HoverMode::Sea,
                        Self::Ruins => HoverMode::Land,
                        Self::Sea => HoverMode::Sea,
                        Self::ShallowSea => HoverMode::Sea,
                        Self::Street => HoverMode::Land,
                        Self::Tavern => HoverMode::Beach,
                        Self::TentacleDepths => HoverMode::Sea,
                        //Self::TrashIsland => HoverMode::Sea,
                        Self::Kraken(_) => HoverMode::Sea,
                    })
                } else {
                    MovementType::Hover(mode)
                }
            }
            _ => {
                movement_type
            }
        })
    }

    pub fn defense_bonus(&self, unit: &UnitType<D>) -> f32 {
        let movement_type = match unit {
            UnitType::Normal(unit) => {
                unit.get_movement(self, None).0
            }
            UnitType::Chess(_) => {
                MovementType::Chess
            }
            UnitType::Structure(_) => return 0.0,
            UnitType::Unknown => return 0.0,
        };
        match (self, movement_type) {
            (Self::Grass, land_units!()) => 0.1,
            (Self::Forest, land_units!()) => 0.2,
            (Self::Fountain, sea_units!()) => -0.2,
            (Self::Hill, land_units!()) => 0.1,
            (Self::Icebergs, sea_units!()) => 0.2,
            (Self::Lillypads, sea_units!()) => -0.1,
            (Self::Realty(_, _, _), land_units!()) => 0.3,
            (Self::Reef, sea_units!()) => 0.1,
            (Self::Ruins, land_units!()) => 0.3,
            (Self::Tavern, land_units!()) => 0.2,
            //(Self::TrashIsland, sea_units!()) => 0.1,
            (_, _) => 0.,
        }
    }

    pub fn adjacent_defense_bonus(&self, unit: &UnitType<D>) -> f32 {
        let movement_type = match unit {
            UnitType::Normal(unit) => {
                unit.get_movement(self, None).0
            }
            UnitType::Chess(_) => {
                MovementType::Chess
            }
            UnitType::Structure(_) => return 0.0,
            UnitType::Unknown => return 0.0,
        };
        match (self, movement_type) {
            //(Self::Statue, sea_units!()) => 0.2,
            (_, _) => 0.,
        }
    }

    pub fn range_bonus(&self) -> u8 {
        match self {
            Self::Hill => 1,
            Self::Mountain => 1,
            Self::Fountain => 1,
            _ => 0,
        }
    }

    pub fn hides_unit(&self, unit: &Unit<D>) -> bool {
        // TODO
        false
    }
    pub fn hides_unit_old(&self, unit: &UnitType<D>) -> bool {
        if !match self {
            Self::Forest => true,
            Self::Icebergs => true,
            _ => false
        } {
            return false;
        }
        match unit {
            UnitType::Structure(_) => false,
            UnitType::Chess(_) => true,
            UnitType::Normal(unit) => {
                let movement_type = unit.get_movement(self, None).0;
                movement_type != MovementType::Plane
                && movement_type != MovementType::Heli
            }
            UnitType::Unknown => false,
        }
    }

    pub fn get_vision(&self, game: &Game<D>, pos: Point, team: Perspective) -> HashMap<Point, FogIntensity> {
        let mut result = HashMap::new();
        match self {
            Terrain::Flame => {
                for layer in game.get_map().range_in_layers(pos, 2) {
                    for p in layer {
                        result.insert(p, FogIntensity::NormalVision);
                    }
                }
                result.insert(pos, FogIntensity::TrueSight);
            }
            Terrain::Realty(_, owner, _) => {
                if let Some(player) = owner.and_then(|owner| game.get_owning_player(owner)) {
                    if Some(player.team) == team {
                        result.insert(pos, FogIntensity::TrueSight);
                    }
                }
            }
            _ => {}
        }
        result
    }

    pub fn fog_replacement(&self, intensity: FogIntensity) -> Terrain<D> {
        match intensity {
            FogIntensity::NormalVision |
            FogIntensity::TrueSight => self.clone(),
            FogIntensity::Light |
            FogIntensity::Dark => {
                match self {
                    Terrain::Realty(realty, _, _) => Terrain::Realty(realty.clone(), None, CaptureProgress::None),
                    Terrain::Kraken(_) => Terrain::Kraken(0.into()),
                    _ => self.clone(),
                }
            }
        }
    }
}*/

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AmphibiousTyping {
    Land,
    Sea,
    Beach,
}

impl FromStr for AmphibiousTyping {
    type Err = ConfigParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut it = s.split(&['(', ',', '-', ')'])
        .map(str::trim);
        Ok(match it.next().unwrap() {
            "Land" => Self::Land,
            "Sea" => Self::Sea,
            "Beach" => Self::Beach,
            invalid => return Err(ConfigParseError::UnknownEnumMember(invalid.to_string())),
        })
    }
}

/*const MAX_BUILT_THIS_TURN: u8 = 9;
pub type BuiltThisTurn = U<{MAX_BUILT_THIS_TURN as i32}>;
#[derive(Debug, PartialEq, Clone, Copy, Zippable)]
#[zippable(bits = 1)]
pub enum CaptureProgress {
    None,
    Capturing(Owner, U<9>),
}

#[derive(Debug, PartialEq, Clone, Zippable)]
#[zippable(bits = 6)]
pub enum Realty {
    Hq,
    City,
    OilPlatform,
    Factory(BuiltThisTurn),
    Port(BuiltThisTurn),
    Airport(BuiltThisTurn),
    ConstructionSite(BuiltThisTurn),
}
impl Realty {
    pub fn income_factor(&self) -> i16 {
        match self {
            Self::City => 1,
            Self::OilPlatform => 1,
            _ => 0,
        }
    }
    pub fn can_build(&self) -> bool {
        match self {
            Self::Factory(_) => true,
            Self::Airport(_) => true,
            Self::Port(_) => true,
            Self::ConstructionSite(_) => true,
            _ => false
        }
    }
    pub fn buildable_units<D: Direction>(&self, game: &Game<D>, owner: Owner) -> Vec<(UnitType<D>, u16)> {
        match self {
            Self::Factory(built_this_turn) => build_options_factory(game, owner, **built_this_turn as u8),
            Self::Port(built_this_turn) => build_options_port(game, owner, **built_this_turn as u8),
            Self::Airport(built_this_turn) => build_options_airport(game, owner, **built_this_turn as u8),
            Self::ConstructionSite(built_this_turn) => build_options_construction_site(game, owner, **built_this_turn as u8),
            _ => vec![],
        }
    }
    pub fn can_repair(&self, unit_type: &NormalUnits) -> bool {
        match self {
            Self::Factory(_) | Self::City => unit_type.repairable_factory(),
            Self::Port(_) | Self::OilPlatform => unit_type.repairable_port(),
            Self::Airport(_) => unit_type.repairable_airport(),            
            _ => false,
        }
    }
    pub fn movement_cost(&self, movement_type: MovementType) -> Option<MovementPoints> {
        match (self, movement_type) {
            (Self::Hq, MovementType::Chess) => None,
            (Self::Port(_), MovementType::Chess) => None,
            (Self::OilPlatform, MovementType::Chess) => None,
            (Self::Hq, _) => Some(MovementPoints::from(1.)),
            (Self::Port(_), _) => Some(MovementPoints::from(1.)),
            (Self::OilPlatform, _) => Some(MovementPoints::from(1.)),
            (
                Self::City |
                Self::Factory(_) |
                Self::Airport(_),
                sea_units!()
            ) => None,
            _ => Some(MovementPoints::from(1.))
        }
    }
    pub fn like_beach_for_hovercraft(&self) -> bool {
        match self {
            Self::Hq => true,
            Self::Port(_) => true,
            Self::OilPlatform => true,
            _ => false,
        }
    }
    pub fn after_buying<D: Direction>(&self, pos: Point, handler: &mut EventHandler<D>) {
        match self {
            Self::Factory(built_this_turn) |
            Self::Airport(built_this_turn) |
            Self::ConstructionSite(built_this_turn) |
            Self::Port(built_this_turn) => {
                if **built_this_turn < MAX_BUILT_THIS_TURN as i32 {
                    handler.terrain_built_this_turn(pos, *built_this_turn + 1);
                }
            }
            _ => {}
        }
    }
}

pub fn build_options_factory<D: Direction>(_game: &Game<D>, owner: Owner, built_this_turn: u8) -> Vec<(UnitType<D>, u16)> {
    let mut result: Vec<(UnitType<D>, u16)> = NormalUnits::list()
    .iter()
    .filter(|u| u.repairable_factory())
    .map(|u| {
        let value = u.value() + 300 * built_this_turn as u16;
        let unit = UnitType::Normal(NormalUnit::new_instance(u.clone(), owner));
        (unit, value)
    }).collect();
    result.sort_by(|v1, v2| v1.1.cmp(&v2.1));
    result
}

pub fn build_options_port<D: Direction>(_game: &Game<D>, owner: Owner, built_this_turn: u8) -> Vec<(UnitType<D>, u16)> {
    let mut result: Vec<(UnitType<D>, u16)> = NormalUnits::list()
    .iter()
    .filter(|u| u.repairable_port())
    .map(|u| {
        let value = u.value() + 300 * built_this_turn as u16;
        let unit = UnitType::Normal(NormalUnit::new_instance(u.clone(), owner));
        (unit, value)
    }).collect();
    result.sort_by(|v1, v2| v1.1.cmp(&v2.1));
    result
}

pub fn build_options_airport<D: Direction>(_game: &Game<D>, owner: Owner, built_this_turn: u8) -> Vec<(UnitType<D>, u16)> {
    let mut result: Vec<(UnitType<D>, u16)> = NormalUnits::list()
    .iter()
    .filter(|u| u.repairable_airport())
    .map(|u| {
        let value = u.value() + 300 * built_this_turn as u16;
        let unit = UnitType::Normal(NormalUnit::new_instance(u.clone(), owner));
        (unit, value)
    }).collect();
    result.sort_by(|v1, v2| v1.1.cmp(&v2.1));
    result
}

pub fn build_options_construction_site<D: Direction>(_game: &Game<D>, owner: Owner, built_this_turn: u8) -> Vec<(UnitType<D>, u16)> {
    let mut list = vec![
        Structures::ShockTower(Some(owner)),
        Structures::DroneTower(owner, LVec::new(), 0.into())
    ];
    for d in D::list() {
        list.push(Structures::MegaCannon(Some(owner), d));
        list.push(Structures::LaserCannon(Some(owner), d));
    }
    let mut result: Vec<(UnitType<D>, u16)> = list.into_iter()
    .map(|u| {
        let value = u.value() + 300 * built_this_turn as u16;
        let unit = UnitType::Structure(Structure::new_instance(u.clone()));
        (unit, value)
    }).collect();
    result.sort_by(|v1, v2| v1.1.cmp(&v2.1));
    result
}*/
