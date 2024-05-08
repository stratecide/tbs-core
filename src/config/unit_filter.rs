use std::collections::HashSet;
use std::error::Error;

use crate::commander::commander_type::CommanderType;
use crate::details::Detail;
use crate::game::fog::FogIntensity;
use crate::game::game_view::GameView;
use crate::map::point::Point;
use crate::terrain::TerrainType;
use crate::units::attributes::ActionStatus;
use crate::units::combat::AttackTypeKey;
use crate::units::hero::{Hero, HeroType};
use crate::units::movement::{MovementType, TBallast};
use crate::units::unit::Unit;
use crate::units::unit_types::UnitType;
use crate::map::direction::Direction;

use super::movement_type_config::MovementPattern;
use super::parse::{parse_inner_vec, parse_inner_vec_dyn, parse_tuple1, parse_tuple2, parse_tuple3, string_base, FromConfig};
use super::ConfigParseError;
use super::config::Config;



#[derive(Debug, Clone)]
pub(super) enum UnitTypeFilter {
    Unit(HashSet<UnitType>),
    MovementPattern(HashSet<MovementPattern>),
}

impl FromConfig for UnitTypeFilter {
    fn from_conf(s: &str) -> Result<(Self, &str), ConfigParseError> {
        let (base, mut remainder) = string_base(s);
        Ok((match base {
            "U" | "Unit" => {
                let (list, r) = parse_inner_vec::<UnitType>(remainder, true)?;
                remainder = r;
                Self::Unit(list.into_iter().collect())
            }
            "MP" | "MovementPattern" => {
                let (list, r) = parse_inner_vec::<MovementPattern>(remainder, true)?;
                remainder = r;
                Self::MovementPattern(list.into_iter().collect())
            }
            _ => return Err(ConfigParseError::UnknownEnumMember(s.to_string()))
        }, remainder))
    }
}

impl UnitTypeFilter {
    pub fn check(&self, config: &Config, unit_type: UnitType) -> bool {
        match self {
            Self::Unit(u) => u.contains(&unit_type),
            Self::MovementPattern(m) => m.contains(&config.movement_pattern(unit_type)),
        }
    }
}


crate::listable_enum! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum TableAxis {
        Unit,
        OtherUnit,
        Terrain,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum TableAxisKey {
    Unit(UnitType),
    Terrain(TerrainType),
}

impl TableAxisKey {
    fn from_conf(axis: TableAxis, s: &str) -> Result<Self, ConfigParseError> {
        match axis {
            TableAxis::Unit |
            TableAxis::OtherUnit => {
                Ok(Self::Unit(UnitType::from_conf(s)?.0))
            }
            TableAxis::Terrain => {
                Ok(Self::Terrain(TerrainType::from_conf(s)?.0))
            }
        }
    }
}

/**
 * UnitFilter and custom actions are the first things to replace with Rhai
 */
#[derive(Debug, Clone)]
pub(crate) enum UnitFilter {
    Unit(HashSet<UnitType>),
    Movement(HashSet<MovementType>),
    Terrain(HashSet<TerrainType>),
    MovementPattern(HashSet<MovementPattern>),
    Hero(HashSet<(HeroType, Option<u8>)>),
    HeroGlobal(HashSet<(HeroType, Option<u8>)>),
    IsHero(HashSet<(HeroType, Option<u8>)>),
    AttackType(HashSet<AttackTypeKey>),
    CommanderCharge(u32),
    Fog(HashSet<FogIntensity>),
    Moved,
    Unowned,
    Status(HashSet<ActionStatus>),
    Sludge,
    Commander(CommanderType, Option<u8>),
    Hp(u8),
    TerrainOwner,
    Counter,
    Table(TableAxis, TableAxis, HashSet<[TableAxisKey; 2]>),
    Not(Vec<Self>),
}

impl UnitFilter {
    pub fn from_conf<'a>(s: &'a str, load_config: &Box<dyn Fn(&str) -> Result<String, Box<dyn Error>>>) -> Result<(Self, &'a str), ConfigParseError> {
        let (base, mut remainder) = string_base(s);
        Ok((match base {
            "Unit" | "U" => {
                let (list, r) = parse_inner_vec::<UnitType>(remainder, true)?;
                remainder = r;
                Self::Unit(list.into_iter().collect())
            }
            "Movement" | "M" => {
                let (list, r) = parse_inner_vec::<MovementType>(remainder, true)?;
                remainder = r;
                Self::Movement(list.into_iter().collect())
            }
            "Terrain" | "T" => {
                let (list, r) = parse_inner_vec::<TerrainType>(remainder, true)?;
                remainder = r;
                Self::Terrain(list.into_iter().collect())
            }
            "MP" | "MovementPattern" => {
                let (list, r) = parse_inner_vec::<MovementPattern>(remainder, true)?;
                remainder = r;
                Self::MovementPattern(list.into_iter().collect())
            }
            "H" | "Hero" => {
                let (list, r) = parse_inner_vec::<(HeroType, Option<u8>)>(remainder, true)?;
                remainder = r;
                Self::Hero(list.into_iter().collect())
            }
            "HG" | "HeroGlobal" => {
                let (list, r) = parse_inner_vec::<(HeroType, Option<u8>)>(remainder, true)?;
                remainder = r;
                Self::HeroGlobal(list.into_iter().collect())
            }
            "IH" | "IsHero" => {
                let (list, r) = parse_inner_vec::<(HeroType, Option<u8>)>(remainder, false)?;
                remainder = r;
                Self::IsHero(list.into_iter().collect())
            }
            "A" | "AttackType" => {
                let (list, r) = parse_inner_vec::<AttackTypeKey>(remainder, true)?;
                remainder = r;
                Self::AttackType(list.into_iter().collect())
            }
            "CC" | "CommanderCharge" => {
                let (charge, r) = parse_tuple1(remainder)?;
                remainder = r;
                Self::CommanderCharge(charge)
            }
            "Fog" => {
                let (list, r) = parse_inner_vec::<FogIntensity>(remainder, true)?;
                remainder = r;
                Self::Fog(list.into_iter().collect())
            }
            "Moved" => Self::Moved,
            "Unowned" => Self::Unowned,
            "S" | "Status" => {
                let (list, r) = parse_inner_vec::<ActionStatus>(remainder, true)?;
                remainder = r;
                Self::Status(list.into_iter().collect())
            }
            "Sludge" => Self::Sludge,
            "Commander" | "Co" => {
                if let Ok((commander, power, r)) = parse_tuple2(remainder) {
                    remainder = r;
                    Self::Commander(commander, Some(power))
                } else {
                    let (commander, r) = parse_tuple1(remainder)?;
                    remainder = r;
                    Self::Commander(commander, None)
                }
            }
            "Hp" => {
                let (hp, r) = parse_tuple1(remainder)?;
                remainder = r;
                Self::Hp(hp)
            }
            "TerrainOwner" => Self::TerrainOwner,
            "Counter" => Self::Counter,
            "Table" => {
                let (y_axis, x_axis, filename, r): (TableAxis, TableAxis, String, &str) = parse_tuple3(remainder)?;
                remainder = r;
                if x_axis == y_axis {
                    return Err(ConfigParseError::TableAxesShouldDiffer(format!("{:?}", x_axis)));
                }
                let data = load_config(&filename).map_err(|e| ConfigParseError::Other(e.to_string()))?;
                let mut reader = csv::ReaderBuilder::new().delimiter(b';').from_reader(data.as_bytes());
                let mut headers = Vec::new();
                for h in reader.headers().map_err(|e| ConfigParseError::Other(e.to_string()))?.into_iter().skip(1) {
                    let header = TableAxisKey::from_conf(x_axis, h)?;
                    if headers.contains(&header) {
                        return Err(ConfigParseError::DuplicateHeader(h.to_string()))
                    }
                    headers.push(header);
                }
                let mut set = HashSet::new();
                for line in reader.records() {
                    let line = line.map_err(|e| ConfigParseError::Other(e.to_string()))?;
                    let mut line = line.into_iter();
                    let y = TableAxisKey::from_conf(y_axis, line.next().unwrap())?;
                    for (i, val) in line.enumerate() {
                        if val == "true" {
                            set.insert([headers[i], y]);
                        }
                    }
                }
                if set.len() == 0 {
                    return Err(ConfigParseError::TableEmpty);
                }
                Self::Table(x_axis, y_axis, set)
            }
            "Not" => {
                let (list, r) = parse_inner_vec_dyn::<Self>(remainder, true, |s| Self::from_conf(s, load_config))?;
                remainder = r;
                Self::Not(list)
            }
            invalid => return Err(ConfigParseError::UnknownEnumMember(invalid.to_string())),
        }, remainder))
    }

    pub fn check<D: Direction>(
        &self,
        map: &impl GameView<D>,
        unit: &Unit<D>,
        unit_pos: (Point, Option<usize>),
        // when moving out of a transporter, or start_turn for transported units
        transporter: Option<(&Unit<D>, Point)>,
        // the attacked unit, the unit this one was destroyed by, ...
        other_unit: Option<(&Unit<D>, Point)>,
        // the heroes affecting this unit. shouldn't be taken from game since they could have died before this function is called
        heroes: &[(Unit<D>, Hero, Point, Option<usize>)],
        // empty if the unit hasn't moved
        temporary_ballast: &[TBallast<D>],
        // true only during counter-attacks
        is_counter: bool,
    ) -> bool {
        match self {
            Self::Unit(u) => u.contains(&unit.typ()),
            Self::Movement(m) => m.contains(&unit.default_movement_type()),
            Self::Terrain(t) => t.contains(&map.get_terrain(unit_pos.0).unwrap().typ()),
            Self::MovementPattern(m) => m.contains(&unit.movement_pattern()),
            Self::Hero(h) => {
                for (_, hero, _, _) in heroes {
                    let power = hero.get_active_power() as u8;
                    if h.iter().any(|h| h.0 == hero.typ() && h.1.unwrap_or(power) == power) {
                        return true;
                    }
                }
                false
            }
            Self::HeroGlobal(h) => {
                for p in map.all_points() {
                    if let Some(u) = map.get_unit(p) {
                        if u.get_owner_id() == unit.get_owner_id() && u.is_hero() {
                            let hero = u.get_hero();
                            let power = hero.get_active_power() as u8;
                            let hero = hero.typ();
                            if h.iter().any(|h| h.0 == hero && h.1.unwrap_or(power) == power) {
                                return true;
                            }
                        }
                    }
                }
                false
            }
            Self::IsHero(h) => {
                let hero = unit.get_hero();
                let power = hero.get_active_power() as u8;
                let hero = hero.typ();
                h.len() == 0 && hero != HeroType::None
                || h.iter().any(|h| h.0 == hero && h.1.unwrap_or(power) == power)
            }
            Self::AttackType(a) => {
                let attack_type = unit.attack_pattern().key();
                a.iter().any(|a| *a == attack_type)
            }
            Self::CommanderCharge(charge) => {
                unit.get_commander(map).get_charge() >= *charge
            }
            Self::Fog(f) => {
                let fog = map.fog_intensity();
                f.iter().any(|f| *f == fog)
            }
            Self::Moved => {
                temporary_ballast.len() > 0
            }
            Self::Unowned => unit.get_owner_id() < 0,
            Self::Status(status) => {
                let s = unit.get_status();
                status.iter().any(|a| *a == s)
            }
            Self::Sludge => {
                map.get_details(unit_pos.0).iter()
                .any(|d| match d {
                    Detail::SludgeToken(_) => true,
                    _ => false
                })
            }
            Self::Commander(commander_type, power) => {
                let commander = unit.get_commander(map);
                commander.typ() == *commander_type
                && (power.is_none() || power.unwrap() as usize == commander.get_active_power())
            }
            Self::Hp(hp) => unit.get_hp() >= *hp,
            Self::TerrainOwner => {
                map.get_terrain(unit_pos.0).unwrap().get_owner_id() == unit.get_owner_id()
            }
            Self::Counter => is_counter,
            Self::Table(x_axis, y_axis, set) => {
                if let [Some(x), Some(y)] = [x_axis, y_axis].map(|axis| match axis {
                    TableAxis::Unit => Some(TableAxisKey::Unit(unit.typ())),
                    TableAxis::OtherUnit => other_unit.map(|(u, _)| TableAxisKey::Unit(u.typ())),
                    TableAxis::Terrain => map.get_terrain(unit_pos.0).map(|t| TableAxisKey::Terrain(t.typ())),
                }) {
                    set.contains(&[x, y])
                } else {
                    false
                }
            }
            Self::Not(negated) => {
                // returns true if at least one check returns false
                // if you need all checks to return false, put them into separate Self::Not wrappers instead
                negated.iter()
                .any(|negated| !negated.check(map, unit, unit_pos, transporter, other_unit, heroes, temporary_ballast, is_counter))
            }
        }
    }
}
