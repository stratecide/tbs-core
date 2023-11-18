use std::fmt::{Display, Debug};

use rustc_hash::FxHashMap;
use zipper::*;

use crate::map::{direction::Direction, map::Map, point::Point};
use crate::game::fog::FogIntensity;
use crate::terrain::Terrain;

pub const DEFAULT_OWNER: i8 = 11;
pub const MAX_PLAYER_COUNT: i8 = 16;


macro_rules! listable_enum {(
    $(#[$meta:meta])*
    $vis:vis enum $name:ident {
        $(
            $member:ident,
        )*
    }) => {
        $(#[$meta])*
        $vis enum $name {
            $($member),*
        }
        impl $name {
            pub fn list() -> &'static [Self] {
                &[$($name::$member,)*]
            }
        }
    };
}




listable_enum! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum CommanderType {
        Vampire,
        Zombie,
        None,
    }
}

impl CommanderType {
    fn attribute_keys(&self) -> &'static [AttributeKey] {
        use AttributeKey as A;
        match self {
            Self::Zombie => &[A::Zombified],
            _ => &[],
        }
    }
    fn attribute_keys_hidden_by_fog(&self) -> &'static [AttributeKey] {
        use AttributeKey as A;
        match self {
            _ => &[],
        }
    }
}


#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum AttributeKey {
    Hp,
    Hero,
    Owner,
    ActionStatus,
    Amphipious,
    Direction,
    DroneStationId,
    DroneId,
    Transported,
    Zombified,
}

impl Display for AttributeKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", match self {
            Self::Hp => "Hp",
            Self::Hero => "Hero",
            Self::Owner => "Owner",
            Self::ActionStatus => "Action Status",
            Self::Amphipious => "Amphipious",
            Self::Direction => "Direction",
            Self::DroneStationId => "Unique Drone-Station Id",
            Self::DroneId => "Unique Drone Id",
            Self::Transported => "Transported Units",
            Self::Zombified => "Zombified",
        })
    }
}

impl AttributeKey {
    fn default<D: Direction>(&self, typ: UnitType) -> Attribute<D> {
        use Attribute as A;
        match self {
            Self::Hp => A::Hp(100),
            Self::Hero => A::Hero(Hero::new(HeroType::None)),
            Self::Owner => A::Owner(if typ.needs_owner() {DEFAULT_OWNER} else {-1}),
            Self::ActionStatus => A::ActionStatus(ActionStatus::Ready),
            Self::Amphipious => A::Amphipious(Amphipious::OnLand),
            Self::Direction => A::Direction(D::angle_0()),
            Self::DroneId => A::DroneId(0),
            Self::DroneStationId => A::DroneStationId(0),
            Self::Transported => A::Transported(Vec::new()),
            Self::Zombified => A::Zombified(false),
        }
    }

    fn disabled_when_transported(&self) -> bool {
        match self {
            Self::Owner => true,
            Self::DroneId => true,
            Self::Transported => true,
            Self::Zombified => true,
            _ => false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Attribute<D: Direction> {
    Hp(u8),
    Hero(Hero),
    Owner(i8),
    ActionStatus(ActionStatus),
    Amphipious(Amphipious),
    Direction(D),
    DroneStationId(u16),
    DroneId(u16),
    Transported(Vec<Unit<D>>),
    Zombified(bool),
}

impl<D: Direction> Attribute<D> {
    fn key(&self) -> AttributeKey {
        use AttributeKey as A;
        match self {
            Self::Hp(_) => A::Hp,
            Self::Hero(_) => A::Hero,
            Self::Owner(_) => A::Owner,
            Self::ActionStatus(_) => A::ActionStatus,
            Self::Amphipious(_) => A::Amphipious,
            Self::Direction(_) => A::Direction,
            Self::DroneStationId(_) => A::DroneStationId,
            Self::DroneId(_) => A::DroneId,
            Self::Transported(_) => A::Transported,
            Self::Zombified(_) => A::Zombified,
        }
    }

    fn export(&self, zipper: &mut Zipper, unit: UnitType, transported: bool, commander: &CommanderType) {
        match self {
            Self::Hp(hp) => U::<100>::from(*hp).export(zipper),
            Self::Hero(hero) => hero.export(zipper),
            Self::Owner(id) => {
                if unit.needs_owner() {
                    zipper.write_u8(0.max(*id) as u8, bits_needed_for_max_value(MAX_PLAYER_COUNT as u32 - 1));
                } else {
                    zipper.write_u8((*id + 1) as u8, bits_needed_for_max_value(MAX_PLAYER_COUNT as u32));
                }
            }
            Self::ActionStatus(status) => {
                if transported {
                    zipper.write_bool(*status != ActionStatus::Ready);
                } else {
                    let valid = unit.valid_action_status();
                    let bits = bits_needed_for_max_value(valid.len() as u32 - 1);
                    zipper.write_u32(valid.iter().position(|s| s == status).unwrap_or(0) as u32, bits);
                }
            }
            Self::Amphipious(amph) => zipper.write_bool(*amph == Amphipious::InWater),
            Self::Direction(d) => {
                let bits = bits_needed_for_max_value(D::list().len() as u32 - 1);
                zipper.write_u8(d.list_index() as u8, bits);
            }
            Self::DroneStationId(id) |
            Self::DroneId(id) => zipper.write_u16(*id, 16),
            Self::Transported(transported) => {
                let bits = bits_needed_for_max_value(unit.transport_capacity() as u32);
                zipper.write_u8(transported.len() as u8, bits);
                for u in transported {
                    u.export(zipper, commander, Some(unit));
                }
            }
            Self::Zombified(z) => zipper.write_bool(*z),
        }
    }

    fn import(unzipper: &mut Unzipper, key: AttributeKey, unit: UnitType, transported: bool, commander: &CommanderType) -> Result<Self, ZipperError> {
        use AttributeKey as A;
        Ok(match key {
            A::Hp => Self::Hp(*(U::<100>::import(unzipper)?) as u8),
            A::Hero => Self::Hero(Hero::import(unzipper)?),
            A::Owner => {
                Self::Owner(if unit.needs_owner() {
                    unzipper.read_u8(bits_needed_for_max_value(MAX_PLAYER_COUNT as u32 - 1))? as i8
                } else {
                    unzipper.read_u8(bits_needed_for_max_value(MAX_PLAYER_COUNT as u32))? as i8 - 1
                }.min(MAX_PLAYER_COUNT - 1))
            }
            A::ActionStatus => {
                Self::ActionStatus(if transported {
                    if unzipper.read_bool()? {
                        ActionStatus::Exhausted
                    } else {
                        ActionStatus::Ready
                    }
                } else {
                    let valid = unit.valid_action_status();
                    let bits = bits_needed_for_max_value(valid.len() as u32 - 1);
                    valid.get(unzipper.read_u32(bits)? as usize).cloned().ok_or(ZipperError::EnumOutOfBounds("ActionStatus".to_string()))?
                })
            }
            A::Amphipious => {
                Self::Amphipious(if unzipper.read_bool()? {
                    Amphipious::InWater
                } else {
                    Amphipious::OnLand
                })
            }
            A::Direction => {
                let bits = bits_needed_for_max_value(D::list().len() as u32 - 1);
                Self::Direction(D::list().get(unzipper.read_u8(bits)? as usize).cloned().unwrap_or(D::angle_0()))
            }
            A::DroneStationId => Self::DroneStationId(unzipper.read_u16(16)?),
            A::DroneId => Self::DroneId(unzipper.read_u16(16)?),
            A::Transported => {
                let bits = bits_needed_for_max_value(unit.transport_capacity() as u32);
                let len = (unzipper.read_u8(bits)? as usize).min(unit.transport_capacity());
                let mut result = Vec::new();
                while result.len() < len {
                    result.push(Unit::import(unzipper, commander, Some(unit))?);
                }
                Self::Transported(result)
            }
            A::Zombified => Self::Zombified(unzipper.read_bool()?),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct AttributeError {
    requested: AttributeKey,
    received: Option<AttributeKey>,
}

trait TrAttribute<D: Direction>: TryFrom<Attribute<D>, Error = AttributeError> + Into<Attribute<D>> {
    fn key() -> AttributeKey;
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

struct Hp(u8);
attribute_tuple!(Hp, Hp);

struct Owner(i8);
attribute_tuple!(Owner, Owner);

struct DroneStationId(u16);
attribute_tuple!(DroneStationId, DroneStationId);

struct DroneId(u16);
attribute_tuple!(DroneId, DroneId);

listable_enum! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    enum Amphipious {
        OnLand,
        InWater,
    }
}
attribute!(Amphipious, Amphipious);

listable_enum! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum ActionStatus {
        Ready,
        Exhausted,
        Capturing,
        Repairing,
    }
}
attribute!(ActionStatus, ActionStatus);



#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum UnitVisibility {
    Stealth,
    Normal,
    AlwaysVisible,
}


listable_enum! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum UnitType {
        HoverBike,
        SmallTank,
        DroneTower,
        LightDrone,
        Tentacle,
        Pyramid,
        Unknown,
    }
}

impl UnitType {
    fn attribute_keys(&self) -> &'static [AttributeKey] {
        use AttributeKey as A;
        match self {
            Self::HoverBike => &[A::Owner, A::Hero, A::Hp, A::ActionStatus, A::Amphipious],
            Self::SmallTank => &[A::Owner, A::Hero, A::Hp, A::ActionStatus],
            Self::DroneTower => &[A::Owner, A::Hp, A::ActionStatus, A::DroneStationId, A::Transported],
            Self::LightDrone => &[A::Owner, A::Hp, A::ActionStatus, A::DroneId],
            Self::Tentacle => &[A::Hp],
            Self::Pyramid => &[A::Owner, A::Hp],
            Self::Unknown => &[],
        }
    }

    fn attribute_keys_hidden_by_fog(&self) -> &'static [AttributeKey] {
        use AttributeKey as A;
        match self {
            Self::DroneTower => &[A::ActionStatus, A::DroneStationId, A::Transported],
            Self::Pyramid => &[],
            _ => &[],
        }
    }

    // should never return a list of size 1
    fn valid_action_status(&self) -> &'static [ActionStatus] {
        use ActionStatus as A;
        match self {
            Self::HoverBike => &[A::Ready, A::Exhausted, A::Capturing, A::Repairing],
            Self::SmallTank => &[A::Ready, A::Exhausted, A::Repairing],
            Self::DroneTower => &[A::Ready, A::Exhausted],
            Self::LightDrone => &[A::Ready, A::Exhausted],
            Self::Tentacle => &[],
            Self::Pyramid => &[],
            Self::Unknown => &[],
        }
    }

    // hm...
    // would it be better as 2 separate keys?
    // or give a boolean to the key?
    fn needs_owner(&self) -> bool {
        match self {
            Self::Unknown => false,
            Self::Tentacle => false,
            Self::Pyramid => false,
            _ => true
        }
    }

    fn transport_capacity(&self) -> usize {
        match self {
            Self::DroneTower => 3,
            _ => 0
        }
    }

    fn transports(&self) -> &'static [UnitType] {
        match self {
            Self::DroneTower => &[Self::LightDrone],
            _ => &[]
        }
    }

    fn visibility(&self) -> UnitVisibility {
        match self {
            Self::HoverBike => UnitVisibility::Normal,
            Self::SmallTank => UnitVisibility::Normal,
            Self::DroneTower => UnitVisibility::AlwaysVisible,
            Self::LightDrone => UnitVisibility::Normal,
            Self::Tentacle => UnitVisibility::Normal,
            Self::Pyramid => UnitVisibility::AlwaysVisible,
            Self::Unknown => UnitVisibility::Normal,
        }
    }

    pub fn instance<D: Direction>(&self) -> UnitBuilder<D> {
        UnitBuilder {
            unit: Unit {
                typ: *self,
                attributes: FxHashMap::default(),
            }
        }
    }
}

impl Display for UnitType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", match self {
            Self::HoverBike => "Hover-Bike",
            Self::SmallTank => "Small Tank",
            Self::DroneTower => "Drone Tower",
            Self::LightDrone => "Light Drone",
            Self::Tentacle => "Tentacle",
            Self::Pyramid => "Pyramid",
            Self::Unknown => "???",
        })
    }
}


listable_enum! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum HeroType {
        EarlGrey,
        None,
    }
}

impl HeroType {
    /*fn attribute_keys(&self) -> &'static [AttributeKey] {
        use AttributeKey as A;
        match self {
            Self::EarlGrey => &[A::Charge],
            Self::None => &[],
        }
    }*/

    fn max_charge(&self) -> u8 {
        match self {
            Self::EarlGrey => 10,
            Self::None => 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Hero {
    typ: HeroType,
    power: bool,
    charge: u8,
}
attribute!(Hero, Hero);

impl Hero {
    pub fn new(typ: HeroType) -> Self {
        Self {
            typ,
            power: false,
            charge: 0,
        }
    }

    fn get_charge(&self) -> u8 {
        self.charge
    }

    fn set_charge(&mut self, charge: u8) {
        self.charge = charge.min(self.typ.max_charge());
    }

    fn export(&self, zipper: &mut Zipper) {
        let bits = bits_needed_for_max_value(HeroType::list().len() as u32 - 1);
        zipper.write_u32(HeroType::list().iter().position(|t| *t == self.typ).unwrap_or(0) as u32, bits);
        if self.typ == HeroType::None {
            return;
        }
        zipper.write_bool(self.power);
        if !self.power && self.typ.max_charge() > 0 {
            let bits = bits_needed_for_max_value(self.typ.max_charge() as u32);
            zipper.write_u8(self.charge, bits);
        }
    }

    fn import(unzipper: &mut Unzipper) -> Result<Self, ZipperError> {
        let bits = bits_needed_for_max_value(HeroType::list().len() as u32 - 1);
        let typ = HeroType::list().get(unzipper.read_u32(bits)? as usize).ok_or(ZipperError::EnumOutOfBounds("HeroType".to_string()))?;
        let mut result = Self::new(*typ);
        if *typ != HeroType::None {
            result.power = unzipper.read_bool()?;
            if !result.power && typ.max_charge() > 0 {
                let bits = bits_needed_for_max_value(typ.max_charge() as u32);
                result.charge = typ.max_charge().min(unzipper.read_u8(bits)?);
            }
        }
        Ok(result)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EarlGrey {
    Charging(u8),
    Power,
}



#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitBuilder<D: Direction> {
    unit: Unit<D>,
}

impl<D: Direction> UnitBuilder<D> {
    pub fn with_owner(mut self, owner_id: i8) -> Self {
        self.set_owner(owner_id);
        self
    }
    pub fn set_owner(&mut self, owner_id: i8) {
        self.unit.set_owner_id(owner_id);
    }

    pub fn build(&self) -> Option<Unit<D>> {
        for key in self.unit.typ.attribute_keys() {
            if !self.unit.attributes.contains_key(key) {
                return None;
            }
        }
        Some(self.unit.clone())
    }

    /**
     * Take Care! The following setters can't be replaced with reasonable defaults:
     *  - drone_id
     *  - drone_station_id
     *  - owner_id
     *  - direction
     */
    pub fn build_with_defaults(&self) -> Unit<D> {
        let mut result = self.unit.clone();
        for key in result.typ.attribute_keys() {
            if !result.attributes.contains_key(key) {
                if *key == AttributeKey::DroneId || *key == AttributeKey::DroneStationId || *key == AttributeKey::Owner {
                    println!("WARNING: building unit with missing Attribute {key}");
                    //return Err(AttributeError { requested: *key, received: None });
                }
                result.attributes.insert(*key, key.default(result.typ));
            }
        }
        result
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unit<D: Direction> {
    typ: UnitType,
    attributes: FxHashMap<AttributeKey, Attribute<D>>,
}

impl<D: Direction> Unit<D> {
    fn get<T: TrAttribute<D>>(&self) -> T {
        if let Some(a) = self.attributes.get(&T::key()) {
            T::try_from(a.clone()).expect("Impossible! attribute of wrong type")
        } else {
            println!("Units of type {} don't have {} attribute, but it was requested anyways", self.typ, T::key());
            T::try_from(T::key().default(self.typ)).expect("Impossible! attribute defaults to wrong type")
        }
    }

    fn set<T: TrAttribute<D>>(&mut self, value: T) -> bool {
        if self.typ.attribute_keys().contains(&T::key()) {
            self.attributes.insert(T::key(), value.into());
            true
        } else {
            false
        }
    }

    pub fn typ(&self) -> UnitType {
        self.typ
    }

    pub fn get_owner_id(&self) -> i8 {
        self.get::<Owner>().0
    }
    pub fn set_owner_id(&mut self, id: i8) {
        if id >= 0 || !self.typ.needs_owner() {
            self.set(Owner(id.max(-1).min(MAX_PLAYER_COUNT - 1)));
        }
    }

    pub fn get_hero(&self) -> Hero {
        self.get::<Hero>()
    }
    pub fn set_hero(&mut self, hero: Hero) {
        // TODO: check if hero is compatible with this unit type
        self.set(hero);
    }

    pub fn get_charge(&self) -> u8 {
        self.get::<Hero>().get_charge()
    }
    pub fn set_charge(&mut self, charge: u8) {
        if let Some(Attribute::Hero(hero)) = self.attributes.get_mut(&AttributeKey::Hero) {
            hero.set_charge(charge);
        }
    }

    pub fn get_hp(&self) -> u8 {
        self.get::<Hp>().0
    }
    pub fn set_hp(&mut self, hp: u8) {
        self.set(Hp(hp.min(100)));
    }

    pub fn get_drone_id(&self) -> u16 {
        self.get::<DroneId>().0
    }
    pub fn set_drone_id(&mut self, id: u16) {
        self.set(DroneId(id));
    }

    pub fn get_drone_station_id(&self) -> u16 {
        self.get::<DroneStationId>().0
    }
    pub fn set_drone_station_id(&mut self, id: u16) {
        self.set(DroneStationId(id));
    }

    pub fn get_status(&self) -> ActionStatus {
        self.get::<ActionStatus>()
    }
    pub fn set_status(&mut self, status: ActionStatus) {
        if self.typ.valid_action_status().contains(&status) {
            self.set(status);
        }
    }
    pub fn is_exhausted(&self) -> bool {
        self.get_status() != ActionStatus::Ready
    }

    pub fn export(&self, zipper: &mut Zipper, commander: &CommanderType, transporter: Option<UnitType>) {
        let list = if let Some(transporter) = transporter {
            transporter.transports()
        } else {
            UnitType::list()
        };
        let bits = bits_needed_for_max_value(list.len() as u32 - 1);
        zipper.write_u32(list.iter().position(|t| *t == self.typ).unwrap_or(0) as u32, bits);
        for key in self.typ.attribute_keys().iter().chain(commander.attribute_keys().iter()) {
            if transporter.is_some() && key.disabled_when_transported() {
                continue;
            }
            let value = key.default(self.typ);
            let value = self.attributes.get(key).unwrap_or(&value);
            value.export(zipper, self.typ, transporter.is_some(), commander);
        }
    }

    pub fn import(unzipper: &mut Unzipper, commander: &CommanderType, transporter: Option<UnitType>) -> Result<Self, ZipperError> {
        let list = if let Some(transporter) = transporter {
            transporter.transports()
        } else {
            UnitType::list()
        };
        let bits = bits_needed_for_max_value(list.len() as u32 - 1);
        let typ = list.get(unzipper.read_u32(bits)? as usize).cloned().ok_or(ZipperError::EnumOutOfBounds("UnitType".to_string()))?;
        let mut attributes = FxHashMap::default();
        for key in typ.attribute_keys().iter().chain(commander.attribute_keys().iter()) {
            if transporter.is_some() && key.disabled_when_transported() {
                continue;
            }
            let attr = Attribute::import(unzipper, *key, typ, transporter.is_some(), commander)?;
            attributes.insert(*key, attr);
        }
        Ok(Unit {
            typ,
            attributes,
        })
    }

    /*pub fn fog_replacement(&self, terrain: &Terrain<D>, intensity: FogIntensity, commander: &CommanderType) -> Option<Self> {
        let visibility = self.typ.visibility();
        // TODO: let hero/commander influence visibility
        match intensity {
            FogIntensity::TrueSight => return Some(self.clone()),
            FogIntensity::NormalVision => {
                if match visibility {
                    UnitVisibility::Stealth => true,
                    UnitVisibility::Normal => terrain.hides_unit(self, commander),
                    UnitVisibility::AlwaysVisible => false,
                } {
                    return None
                }
            }
            FogIntensity::Light => {
                match visibility {
                    UnitVisibility::Stealth => return None,
                    UnitVisibility::Normal => {
                        if terrain.hides_unit(self, commander) {
                            return None
                        } else {
                            return Some(UnitType::Unknown.instance().build_with_defaults())
                        }
                    }
                    UnitVisibility::AlwaysVisible => (),
                }
            }
            FogIntensity::Dark => {
                // normal units don't have AlwaysVisible so far, but doesn't hurt
                if visibility != UnitVisibility::AlwaysVisible {
                    return None
                }
            }
        }
        // unit is visible, hide some attributes maybe
        let mut builder = self.typ.instance();
        let hidden_attributes = self.typ.attribute_keys_hidden_by_fog();
        let hidden_attributes2 = commander.attribute_keys_hidden_by_fog();
        for (k, v) in &self.attributes {
            if !hidden_attributes.contains(k) && !hidden_attributes2.contains(k) {
                builder.unit.attributes.insert(*k, v.clone());
            }
        }
        Some(builder.build_with_defaults())
    }*/

    pub fn transformed_by_movement(&self, map: &Map<D>, from: Point, to: Point) -> Option<Self> {
        let prev_terrain = map.get_terrain(from).unwrap();
        let mut changed = FxHashMap::default();
        if let Some(Attribute::Amphipious(amphibious)) = self.attributes.get(&AttributeKey::Amphipious) {
            if prev_terrain.like_beach_for_hovercraft() {
                let terrain = map.get_terrain(to).unwrap();
                // TODO: every terrain should be exactly one of is_water, is_land or is_beach
                if *amphibious == Amphipious::OnLand && terrain.is_water() {
                    changed.insert(AttributeKey::Amphipious, Attribute::Amphipious(Amphipious::InWater));
                } else if *amphibious == Amphipious::InWater && terrain.is_land() {
                    changed.insert(AttributeKey::Amphipious, Attribute::Amphipious(Amphipious::OnLand));
                }
            }
        }
        if changed.len() > 0 {
            let mut result = self.clone();
            for (k, v) in changed {
                result.attributes.insert(k, v);
            }
            Some(result)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::map::direction::Direction4;

    use super::*;

    #[test]
    fn create_simple_unit() {
        let unit = UnitType::SmallTank.instance::<Direction4>()
        .with_owner(DEFAULT_OWNER)
        .build_with_defaults();

        assert!(!unit.is_exhausted());
        assert_eq!(unit.get_hero(), Hero::new(HeroType::None));
        assert_eq!(unit.get_hp(), 100);

        let mut attributes = FxHashMap::default();
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
    }
}
