use std::collections::HashMap;
use std::fmt::{Display, Debug};

use zipper::*;

use crate::config::config::Config;
use crate::config::environment::Environment;
use crate::config::parse::{parse_tuple1, string_base, FromConfig};
use crate::map::direction::{Direction, Direction4, Direction6};
use crate::map::point::Point;
use crate::player::Owner;
use crate::config::ConfigParseError;
use crate::terrain::AmphibiousTyping;

use super::unit_types::UnitType;
use super::unit::*;
use super::hero::*;

pub const DEFAULT_OWNER: i8 = 0;


crate::listable_enum! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum AttributeKey {
        // Owner should be the first attribute
        // because other atributes may depend on Owner for importing
        // and attributes are imported in order (except commander-specific ones)
        Owner,
        Hero,
        Hp,
        ActionStatus,
        Amphibious,
        Direction,
        DroneStationId,
        DroneId,
        Zombified,
        Unmoved,
        EnPassant,
        Level,
        Transported,
    }
}

impl Display for AttributeKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", match self {
            Self::Hp => "Hp",
            Self::Hero => "Hero",
            Self::Owner => "Owner",
            Self::ActionStatus => "Action Status",
            Self::Amphibious => "Amphibious",
            Self::Direction => "Direction",
            Self::DroneStationId => "Unique Drone-Station Id",
            Self::DroneId => "Unique Drone Id",
            Self::Transported => "Transported Units",
            Self::Zombified => "Zombified",
            Self::Unmoved => "Unmoved",
            Self::EnPassant => "Can be taken En Passant",
            Self::Level => "Level",
        })
    }
}

impl AttributeKey {
    pub fn default<D: Direction>(&self) -> Attribute<D> {
        use Attribute as A;
        match self {
            Self::Hp => A::Hp(100),
            Self::Hero => A::Hero(Hero::new(HeroType::None, None)),
            Self::Owner => A::Owner(-1),
            Self::ActionStatus => A::ActionStatus(ActionStatus::Ready),
            Self::Amphibious => A::Amphibious(Amphibious::default()),
            Self::Direction => A::Direction(D::angle_0()),
            Self::DroneId => A::DroneId(0),
            Self::DroneStationId => A::DroneStationId(0),
            Self::Transported => A::Transported(Vec::new()),
            Self::Zombified => A::Zombified(false),
            Self::Unmoved => A::Unmoved(true),
            Self::EnPassant => A::EnPassant(None),
            Self::Level => A::Level(0),
        }
    }

    pub fn is_skull_data(&self, _config: &Config) -> bool {
        match self {
            Self::Amphibious => true,
            Self::Direction => true,
            Self::DroneStationId => true,
            _ => false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Attribute<D: Direction> {
    Hp(u8),
    Hero(Hero),
    Owner(i8),
    ActionStatus(ActionStatus),
    Amphibious(Amphibious),
    Direction(D),
    DroneStationId(u16),
    DroneId(u16),
    Transported(Vec<Unit<D>>),
    Zombified(bool),
    Unmoved(bool),
    EnPassant(Option<Point>),
    Level(u8),
}

impl<D: Direction> Attribute<D> {
    pub fn key(&self) -> AttributeKey {
        use AttributeKey as A;
        match self {
            Self::Hp(_) => A::Hp,
            Self::Hero(_) => A::Hero,
            Self::Owner(_) => A::Owner,
            Self::ActionStatus(_) => A::ActionStatus,
            Self::Amphibious(_) => A::Amphibious,
            Self::Direction(_) => A::Direction,
            Self::DroneStationId(_) => A::DroneStationId,
            Self::DroneId(_) => A::DroneId,
            Self::Transported(_) => A::Transported,
            Self::Zombified(_) => A::Zombified,
            Self::Unmoved(_) => A::Unmoved,
            Self::EnPassant(_) => A::EnPassant,
            Self::Level(_) => A::Level,
        }
    }

    pub(crate) fn export(&self, environment: &Environment, zipper: &mut Zipper, typ: UnitType, transported: bool, owner: i8, hero: HeroType) {
        match self {
            Self::Hp(hp) => U::<100>::from(*hp).zip(zipper),
            Self::Hero(hero) => hero.export(zipper, environment),
            Self::Owner(id) => {
                if environment.config.unit_needs_owner(typ) {
                    zipper.write_u8(0.max(*id) as u8, bits_needed_for_max_value(environment.config.max_player_count() as u32 - 1));
                } else {
                    zipper.write_u8((*id + 1) as u8, bits_needed_for_max_value(environment.config.max_player_count() as u32));
                }
            }
            Self::ActionStatus(status) => {
                if transported {
                    zipper.write_bool(*status != ActionStatus::Ready);
                } else {
                    let valid = environment.unit_valid_action_status(typ, owner);
                    let bits = bits_needed_for_max_value(valid.len() as u32 - 1);
                    zipper.write_u32(valid.iter().position(|s| s == status).unwrap_or(0) as u32, bits);
                }
            }
            Self::Amphibious(amph) => zipper.write_bool(*amph == Amphibious::InWater),
            Self::Direction(d) => {
                let bits = bits_needed_for_max_value(D::list().len() as u32 - 1);
                zipper.write_u8(d.list_index() as u8, bits);
            }
            Self::DroneStationId(id) |
            Self::DroneId(id) => zipper.write_u16(*id, 16),
            Self::Transported(transported) => {
                let bits = bits_needed_for_max_value(environment.unit_transport_capacity(typ, owner, hero) as u32);
                zipper.write_u8(transported.len() as u8, bits);
                for u in transported {
                    u.zip(zipper, Some((typ, owner)));
                }
            }
            Self::Zombified(z) => zipper.write_bool(*z),
            Self::Unmoved(z) => zipper.write_bool(*z),
            Self::EnPassant(z) => z.export(zipper, environment),
            Self::Level(level) => {
                let bits = bits_needed_for_max_value(environment.config.max_unit_level() as u32);
                zipper.write_u8(*level, bits);
            }
        }
    }

    pub(crate) fn import(unzipper: &mut Unzipper, environment: &Environment, key: AttributeKey, typ: UnitType, transported: bool, owner: i8, hero: HeroType) -> Result<Self, ZipperError> {
        use AttributeKey as A;
        Ok(match key {
            A::Hp => Self::Hp(*(U::<100>::unzip(unzipper)?) as u8),
            A::Hero => Self::Hero(Hero::import(unzipper, environment)?),
            A::Owner => {
                Self::Owner(if environment.config.unit_needs_owner(typ) {
                    unzipper.read_u8(bits_needed_for_max_value(environment.config.max_player_count() as u32 - 1))? as i8
                } else {
                    unzipper.read_u8(bits_needed_for_max_value(environment.config.max_player_count() as u32))? as i8 - 1
                }.min(environment.config.max_player_count() - 1))
            }
            A::ActionStatus => {
                Self::ActionStatus(if transported {
                    if unzipper.read_bool()? {
                        ActionStatus::Exhausted
                    } else {
                        ActionStatus::Ready
                    }
                } else {
                    let valid = environment.unit_valid_action_status(typ, owner);
                    let bits = bits_needed_for_max_value(valid.len() as u32 - 1);
                    valid.get(unzipper.read_u32(bits)? as usize).cloned().ok_or(ZipperError::EnumOutOfBounds("ActionStatus".to_string()))?
                })
            }
            A::Amphibious => {
                Self::Amphibious(if unzipper.read_bool()? {
                    Amphibious::InWater
                } else {
                    Amphibious::OnLand
                })
            }
            A::Direction => {
                let bits = bits_needed_for_max_value(D::list().len() as u32 - 1);
                Self::Direction(D::list().get(unzipper.read_u8(bits)? as usize).cloned().unwrap_or(D::angle_0()))
            }
            A::DroneStationId => Self::DroneStationId(unzipper.read_u16(16)?),
            A::DroneId => Self::DroneId(unzipper.read_u16(16)?),
            A::Transported => {
                let bits = bits_needed_for_max_value(environment.unit_transport_capacity(typ, owner, hero) as u32);
                let len = (unzipper.read_u8(bits)? as usize).min(environment.unit_transport_capacity(typ, owner, hero));
                let mut result = Vec::new();
                while result.len() < len {
                    result.push(Unit::unzip(unzipper, environment, Some((typ, owner)))?);
                }
                Self::Transported(result)
            }
            A::Zombified => Self::Zombified(unzipper.read_bool()?),
            A::Unmoved => Self::Unmoved(unzipper.read_bool()?),
            A::EnPassant => Self::EnPassant(Option::<Point>::import(unzipper, environment)?),
            A::Level => {
                let bits = bits_needed_for_max_value(environment.config.max_unit_level() as u32);
                Self::Level(unzipper.read_u8(bits)?)
            }
        })
    }
    
    pub(super) fn build_from_transporter(key: AttributeKey) -> Option<Box<dyn Fn(&HashMap<AttributeKey, Attribute<D>>) -> Option<Attribute<D>>>> {
        match key {
            AttributeKey::DroneId => Some(Box::new(|attributes| {
                if let Some(Attribute::DroneStationId(id)) = attributes.get(&AttributeKey::DroneStationId) {
                    Some(Attribute::DroneId(*id))
                } else {
                    None
                }
            })),
            AttributeKey::Transported => Some(Box::new(|_| Some(Attribute::Transported(Vec::new())))),
            AttributeKey::Owner => Some(Box::new(|attributes| attributes.get(&AttributeKey::Owner).cloned())),
            AttributeKey::Zombified => Some(Box::new(|attributes| attributes.get(&AttributeKey::Zombified).cloned())),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct AttributeError {
    pub requested: AttributeKey,
    pub received: Option<AttributeKey>,
}

pub trait TrAttribute<D: Direction>: TryFrom<Attribute<D>, Error = AttributeError> + Into<Attribute<D>> {
    fn key() -> AttributeKey;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AttributeOverride {
    InWater,
    OnLand,
    Hp(u8),
    Zombified,
    Unowned,
}

impl FromConfig for AttributeOverride {
    fn from_conf(s: &str) -> Result<(Self, &str), ConfigParseError> {
        let (base, mut s) = string_base(s);
        Ok((match base {
            "InWater" => Self::InWater,
            "OnLand" => Self::OnLand,
            "Zombified" => Self::Zombified,
            "Hp" => {
                let (hp, r) = parse_tuple1::<u8>(s)?;
                if hp == 0 || hp > 100 {
                    return Err(ConfigParseError::InvalidInteger(hp.to_string()));
                }
                s = r;
                Self::Hp(hp)
            }
            "Unowned" => Self::Unowned,
            invalid => return Err(ConfigParseError::UnknownEnumMember(invalid.to_string())),
        }, s))
    }
}

impl AttributeOverride {
    pub fn key(&self) -> AttributeKey {
        match self {
            Self::InWater => AttributeKey::Amphibious,
            Self::OnLand => AttributeKey::Amphibious,
            Self::Hp(_) => AttributeKey::Hp,
            Self::Zombified => AttributeKey::Zombified,
            Self::Unowned => AttributeKey::Owner,
        }
    }
}

impl<D: Direction> From<&AttributeOverride> for Attribute<D> {
    fn from(value: &AttributeOverride) -> Self {
        match value {
            AttributeOverride::Hp(hp) => Attribute::Hp(*hp),
            AttributeOverride::InWater => Attribute::Amphibious(Amphibious::InWater),
            AttributeOverride::OnLand => Attribute::Amphibious(Amphibious::OnLand),
            AttributeOverride::Zombified => Attribute::Zombified(true),
            AttributeOverride::Unowned => Attribute::Owner(-1),
        }
    }
}

macro_rules! attribute_tuple {
    ($name: ty, $attr: ident) => {
        impl<D: Direction> TryFrom<Attribute<D>> for $name {
            type Error = AttributeError;
            fn try_from(value: Attribute<D>) -> Result<Self, Self::Error> {
                if let Attribute::$attr(value) = value {
                    Ok(Self(value))
                } else {
                    Err(AttributeError { requested: <Self as TrAttribute<D>>::key(), received: Some(value.key()) })
                }
            }
        }

        impl<D: Direction> From<$name> for Attribute<D> {
            fn from(value: $name) -> Self {
                Attribute::$attr(value.0)
            }
        }

        impl<D: Direction> TrAttribute<D> for $name {
            fn key() -> AttributeKey {
                AttributeKey::$attr
            }
        }
    };
}

macro_rules! attribute {
    ($name: ty, $attr: ident) => {
        impl<D: Direction> TryFrom<Attribute<D>> for $name {
            type Error = AttributeError;
            fn try_from(value: Attribute<D>) -> Result<Self, Self::Error> {
                if let Attribute::$attr(value) = value {
                    Ok(value)
                } else {
                    Err(AttributeError { requested: <Self as TrAttribute<D>>::key(), received: Some(value.key()) })
                }
            }
        }

        impl<D: Direction> From<$name> for Attribute<D> {
            fn from(value: $name) -> Self {
                Attribute::$attr(value)
            }
        }
        
        impl<D: Direction> TrAttribute<D> for $name {
            fn key() -> AttributeKey {
                AttributeKey::$attr
            }
        }        
    };
}
pub(crate) use attribute;

pub(super) struct Hp(pub(super) u8);
attribute_tuple!(Hp, Hp);

attribute_tuple!(Owner, Owner);

impl TrAttribute<Direction4> for Direction4 {
    fn key() -> AttributeKey {
        AttributeKey::Direction
    }
}
impl TrAttribute<Direction6> for Direction6 {
    fn key() -> AttributeKey {
        AttributeKey::Direction
    }
}

pub(super) struct DroneStationId(pub(super) u16);
attribute_tuple!(DroneStationId, DroneStationId);

pub(super) struct DroneId(pub(super) u16);
attribute_tuple!(DroneId, DroneId);

crate::listable_enum! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum Amphibious {
        OnLand,
        InWater,
    }
}

// TODO: delete this
impl Display for Amphibious {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", match self {
            Self::OnLand => "Land-Mode",
            Self::InWater => "Sea-Mode",
        })
    }
}

attribute!(Amphibious, Amphibious);

impl Default for Amphibious {
    fn default() -> Self {
        Self::OnLand
    }
}

impl From<&AmphibiousTyping> for Amphibious {
    fn from(value: &AmphibiousTyping) -> Self {
        match value {
            AmphibiousTyping::Beach => Amphibious::InWater,
            AmphibiousTyping::Land => Amphibious::OnLand,
            AmphibiousTyping::Sea => Amphibious::InWater,
        }
    }
}

crate::listable_enum! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum ActionStatus {
        Ready,
        Exhausted,
        Capturing,
        Repairing,
    }
}

impl SupportedZippable<&Environment> for ActionStatus {
    fn export(&self, zipper: &mut Zipper, _support: &Environment) {
        let list = Self::list();
        let index = list.iter().position(|s| s == self).unwrap() as u8;
        zipper.write_u8(index, bits_needed_for_max_value(list.len() as u32));
    }
    fn import(unzipper: &mut Unzipper, _support: &Environment) -> Result<Self, ZipperError> {
        let list = Self::list();
        let index = unzipper.read_u8(bits_needed_for_max_value(list.len() as u32))?;
        Ok(list[index as usize])
    }
}

impl Display for ActionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", match self {
            Self::Ready => "Ready",
            Self::Exhausted => "Exhausted",
            Self::Capturing => "Capturing",
            Self::Repairing => "Repairing",
        })
    }
}

attribute!(ActionStatus, ActionStatus);

pub(super) struct Unmoved(pub(super) bool);
attribute_tuple!(Unmoved, Unmoved);

attribute!(Option<Point>, EnPassant);

pub(super) struct Zombified(pub(super) bool);
attribute_tuple!(Zombified, Zombified);

#[derive(Debug, Clone, PartialEq)]
pub struct Level(pub u8);
attribute_tuple!(Level, Level);

impl SupportedZippable<&Environment> for Level {
    fn export(&self, zipper: &mut Zipper, support: &Environment) {
        let bits = bits_needed_for_max_value(support.config.max_unit_level() as u32);
        zipper.write_u8(self.0, bits);
    }
    fn import(unzipper: &mut Unzipper, support: &Environment) -> Result<Self, ZipperError> {
        let bits = bits_needed_for_max_value(support.config.max_unit_level() as u32);
        Ok(Self(unzipper.read_u8(bits)?))
    }
}

impl From<u8> for Level {
    fn from(value: u8) -> Self {
        Self(value)
    }
}


crate::listable_enum! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum UnitVisibility {
        Stealth,
        Normal,
        AlwaysVisible,
    }
}




#[cfg(test)]
mod tests {
    /*use crate::map::direction::Direction4;

    use super::*;

    #[test]
    fn create_simple_unit() {
        let unit = UnitType::SmallTank.instance::<Direction4>()
        .with_owner(DEFAULT_OWNER)
        .build_with_defaults();

        assert!(!unit.is_exhausted());
        assert_eq!(unit.get_hero(), Hero::new(HeroType::None));
        assert_eq!(unit.get_hp(), 100);

        let mut attributes = HashMap::default();
        attributes.insert(AttributeKey::Owner, Attribute::Owner(DEFAULT_OWNER));
        attributes.insert(AttributeKey::Hp, Attribute::Hp(100));
        attributes.insert(AttributeKey::Hero, Attribute::Hero(Hero::new(HeroType::None)));
        attributes.insert(AttributeKey::ActionStatus, Attribute::ActionStatus(ActionStatus::Ready));

        assert_eq!(
            unit,
            Unit {
                typ: UnitType::SmallTank,
                attributes,
            }
        );
    }

    #[test]
    fn check_action_status() {
        for unit_type in UnitType::list() {
            let valid = unit_type.valid_action_status();
            // if only 1 is possible, the attribute is unneeded
            assert_ne!(valid.len(), 1);
            // attribute should exist if and only if there are valid ActionStatus values
            assert_eq!(
                valid.len() > 0,
                unit_type.attribute_keys().contains(&AttributeKey::ActionStatus)
            );
            // no double-entries
            for v in valid {
                assert_eq!(1, valid.iter().filter(|s| *s == v).count());
            }
        }
    }

    #[test]
    fn check_attribute_keys() {
        for unit_type in UnitType::list() {
            let keys = unit_type.attribute_keys();
            for key in keys {
                // no double-entries
                assert_eq!(1, keys.iter().filter(|a| *a == key).count());
            }
            let hidden_keys = unit_type.attribute_keys_hidden_by_fog();
            for key in hidden_keys {
                assert!(keys.contains(key));
                // no double-entries
                assert_eq!(1, hidden_keys.iter().filter(|a| *a == key).count());
            }
            if unit_type.needs_owner() {
                assert!(keys.contains(&AttributeKey::Owner));
                assert!(!hidden_keys.contains(&AttributeKey::Owner));
            }
            assert_eq!(
                unit_type.transports().len() > 0,
                keys.contains(&AttributeKey::Transported)
            );
            assert_eq!(
                unit_type.transport_capacity() > 0,
                keys.contains(&AttributeKey::Transported)
            );
        }
    }*/
}
