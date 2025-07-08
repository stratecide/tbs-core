use std::sync::Arc;
use rhai::*;
use rhai::plugin::*;
use rustc_hash::{FxHashMap, FxHashSet};
use zipper::*;

use crate::config::environment::Environment;
use crate::config::parse::FromConfig;
use crate::config::tag_config::TagType;
use crate::map::direction::Direction;
use crate::map::point::Point;
use crate::map::point_map::MAX_AREA;
use crate::map::wrapping_map::Distortion;
use crate::terrain::TerrainType;
use crate::units::movement::MovementType;
use crate::units::unit_types::UnitType;
use crate::units::UnitVisibility;


#[derive(Clone, PartialEq, Eq)]
pub struct TagBag<D: Direction> {
    flags: Vec<usize>,
    tags: FxHashMap<usize, TagValue<D>>
}

impl<D: Direction> TagBag<D> {
    pub fn new() -> Self {
        Self {
            flags: Vec::new(),
            tags: FxHashMap::default(),
        }
    }

    pub fn debug(&self, f: &mut std::fmt::Formatter<'_>, environment: &Environment) -> std::fmt::Result {
        write!(f, "FLAGS[")?;
        for flag in &self.flags {
            write!(f, "{}", environment.config.flag_name(*flag))?;
        }
        write!(f, "], TAGS[")?;
        for (key, value) in &self.tags {
            write!(f, "{}=", environment.config.tag_name(*key))?;
            match value {
                TagValue::Unique(value) => write!(f, "{value:?}")?,
                TagValue::Int(value) => write!(f, "{}", value.0)?,
                TagValue::Point(value) => write!(f, "{value:?}")?,
                TagValue::Direction(value) => write!(f, "{value:?}")?,
                TagValue::UnitType(value) => write!(f, "{}", environment.config.unit_name(*value))?,
                TagValue::TerrainType(value) => write!(f, "{}", environment.config.terrain_name(*value))?,
                TagValue::MovementType(value) => write!(f, "{}", environment.config.movement_type_name(*value))?,
            }
        }
        write!(f, "]")
    }

    pub fn fog_replacement(&self, environment: &Environment, minimum_visibility: UnitVisibility) -> Self {
        let mut result = Self::new();
        for flag in &self.flags {
            if environment.config.flag_visibility(*flag) >= minimum_visibility {
                result.flags.push(*flag);
            }
        }
        for (key, value) in &self.tags {
            if environment.config.tag_visibility(*key) >= minimum_visibility {
                result.tags.insert(*key, value.clone());
            }
        }
        result
    }

    pub fn flags(&self) -> impl Iterator<Item = &usize> {
        self.flags.iter()
    }

    pub fn has_flag(&self, flag: usize) -> bool {
        self.flags.contains(&flag)
    }

    pub fn set_flag(&mut self, environment: &Environment, flag: usize) -> bool {
        if flag >= environment.config.flag_count() {
            return false;
        }
        if self.flags.contains(&flag) {
            false
        } else {
            self.flags.push(flag);
            true
        }
    }

    pub fn remove_flag(&mut self, flag: usize) -> bool {
        if let Some(index) = self.flags.iter().position(|i| *i == flag) {
            self.flags.swap_remove(index);
            true
        } else {
            false
        }
    }

    pub fn tags(&self) -> impl Iterator<Item = (&usize, &TagValue<D>)> {
        self.tags.iter()
    }

    pub fn has_tag(&self, key: usize) -> bool {
        self.tags.contains_key(&key)
    }

    pub fn get_tag(&self, key: usize) -> Option<TagValue<D>> {
        self.tags.get(&key).cloned()
    }

    pub fn set_tag(&mut self, environment: &Environment, key: usize, value: TagValue<D>) -> Option<TagValue<D>> {
        if value.has_valid_type(environment, key) {
            self.tags.insert(key, value)
        } else {
            None
        }
    }

    pub fn remove_tag(&mut self, key: usize) -> Option<TagValue<D>> {
        self.tags.remove(&key)
    }

    pub fn distort(&mut self, distortion: Distortion<D>) {
        for tag in self.tags.values_mut() {
            tag.distort(distortion);
        }
    }
    pub fn translate(&mut self, translations: [D::T; 2], odd_if_hex: bool) {
        for tag in self.tags.values_mut() {
            tag.translate(translations, odd_if_hex);
        }
    }
}

impl<D: Direction> SupportedZippable<&Environment> for TagBag<D> {
    fn export(&self, zipper: &mut Zipper, support: &Environment) {
        let flag_bits = bits_needed_for_max_value(support.config.flag_count() as u32);
        zipper.write_u32(self.flags.len() as u32, flag_bits);
        for flag in &self.flags {
            zipper.write_u32(*flag as u32, flag_bits);
        }
        let tag_bits = bits_needed_for_max_value(support.config.tag_count() as u32);
        zipper.write_u32(self.tags.len() as u32, tag_bits);
        for (key, value) in &self.tags {
            zipper.write_u32(*key as u32, tag_bits);
            value.export(zipper, support, *key);
        }
    }
    fn import(unzipper: &mut Unzipper, support: &Environment) -> Result<Self, ZipperError> {
        let mut result = Self::new();
        let flag_bits = bits_needed_for_max_value(support.config.flag_count() as u32);
        let flag_count = unzipper.read_u32(flag_bits)?.min(support.config.flag_count() as u32);
        for _ in 0..flag_count {
            result.set_flag(support, unzipper.read_u32(flag_bits)? as usize);
        }
        let tag_bits = bits_needed_for_max_value(support.config.tag_count() as u32);
        let tag_count = unzipper.read_u32(tag_bits)?.min(support.config.tag_count() as u32);
        for _ in 0..tag_count {
            let key = support.config.tag_count().min(unzipper.read_u32(tag_bits)? as usize);
            let value = TagValue::import(unzipper, support, key)?;
            result.set_tag(support, key, value);
        }
        Ok(result)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TagValue<D: Direction> {
    Unique(Arc<UniqueId>),
    Int(Int32),
    Point(Point),
    Direction(D),
    UnitType(UnitType),
    TerrainType(TerrainType),
    MovementType(MovementType),
}

const TAG_VALUE_ENUM_BITS: u8 = 4;
impl<D: Direction> TagValue<D> {
    fn export(&self, zipper: &mut Zipper, environment: &Environment, tag_key: usize) {
        match self {
            Self::Unique(value) => {
                zipper.write_u8(0, TAG_VALUE_ENUM_BITS);
                value.export(zipper);
            }
            Self::Int(value) => {
                zipper.write_u8(1, TAG_VALUE_ENUM_BITS);
                value.export(zipper, environment, tag_key);
            }
            Self::Point(value) => {
                zipper.write_u8(2, TAG_VALUE_ENUM_BITS);
                value.export(zipper, environment);
            }
            Self::Direction(value) => {
                zipper.write_u8(3, TAG_VALUE_ENUM_BITS);
                value.zip(zipper);
            }
            Self::UnitType(value) => {
                zipper.write_u8(4, TAG_VALUE_ENUM_BITS);
                value.export(zipper, environment);
            }
            Self::TerrainType(value) => {
                zipper.write_u8(5, TAG_VALUE_ENUM_BITS);
                value.export(zipper, environment);
            }
            Self::MovementType(value) => {
                zipper.write_u8(6, TAG_VALUE_ENUM_BITS);
                value.export(zipper, environment);
            }
        }
    }

    fn import(unzipper: &mut Unzipper, environment: &Environment, tag_key: usize) -> Result<Self, ZipperError> {
        match unzipper.read_u8(TAG_VALUE_ENUM_BITS)? {
            0 => Ok(Self::Unique(UniqueId::import(unzipper, environment, tag_key)?)),
            1 => Ok(Self::Int(Int32::import(unzipper, environment, tag_key)?)),
            2 => Ok(Self::Point(Point::import(unzipper, environment)?)),
            3 => Ok(Self::Direction(D::unzip(unzipper)?)),
            4 => Ok(Self::UnitType(UnitType::import(unzipper, environment)?)),
            5 => Ok(Self::TerrainType(TerrainType::import(unzipper, environment)?)),
            6 => Ok(Self::MovementType(MovementType::import(unzipper, environment)?)),
            e => Err(ZipperError::EnumOutOfBounds(format!("TagValue::{e} for tag {}", environment.config.tag_name(tag_key))))
        }
    }

    pub(crate) fn has_valid_type(&self, environment: &Environment, key: usize) -> bool {
        match (self, environment.config.tag_type(key)) {
            (Self::Unique(value), tag_type) => {
                value.environment == *environment && environment.config.tag_type(value.tag) == tag_type
            },
            (Self::Point(_), TagType::Point) => true,
            (Self::Direction(_), TagType::Direction) => true,
            (Self::UnitType(_), TagType::UnitType) => true,
            (Self::TerrainType(_), TagType::TerrainType) => true,
            (Self::MovementType(_), TagType::MovementType) => true,
            (Self::Int(value), TagType::Int { min, max }) => {
                value.0 >= *min && value.0 <= *max
            }
            _ => false
        }
    }

    pub fn distort(&mut self, distortion: Distortion<D>) {
        match self {
            Self::Direction(d) => {
                *d = distortion.update_direction(*d);
            }
            _ => ()
        }
    }
    pub fn translate(&mut self, translations: [D::T; 2], odd_if_hex: bool) {
        match self {
            Self::Point(p) => {
                *p = p.translate::<D>(&translations[p.y as usize % 2], odd_if_hex)
            }
            _ => ()
        }
    }

    pub fn into_dynamic(&self) -> Dynamic {
        match self {
            Self::Direction(value) => Dynamic::from(*value),
            Self::Int(value) => Dynamic::from(value.0),
            Self::Point(value) => Dynamic::from(*value),
            Self::TerrainType(value) => Dynamic::from(*value),
            Self::Unique(value) => Dynamic::from(value.clone()),
            Self::UnitType(value) => Dynamic::from(*value),
            Self::MovementType(value) => Dynamic::from(*value),
        }
    }

    pub fn from_dynamic(value: Dynamic, key: usize, environment: &Environment) -> Option<Self> {
        let result = match value.type_name().split("::").last().unwrap() {
            "Direction" => Some(Self::Direction(value.cast())),
            "i32" => {
                let TagType::Int { min, max } = environment.config.tag_type(key) else {
                    return None;
                };
                Some(Self::Int(Int32(value.cast::<i32>().max(*min).min(*max))))
            }
            "Point" => Some(Self::Point(value.cast())),
            "TerrainType" => Some(Self::TerrainType(value.try_cast()?)),
            "UnitType" => Some(Self::UnitType(value.try_cast()?)),
            "UniqueId>" => Some(Self::Unique(value.try_cast()?)),
            err => {
                tracing::error!("failed to turn dynamic '{err}' into TagValue");
                None
            }
        }?;
        Some(result)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Int32(pub i32);

impl<D: Direction> From<i32> for TagValue<D> {
    fn from(value: i32) -> Self {
        Self::Int(Int32(value))
    }
}

impl Int32 {
    fn export(&self, zipper: &mut Zipper, environment: &Environment, tag_key: usize) {
        let TagType::Int { min, max } = environment.config.tag_type(tag_key) else {
            panic!("TagValue::Int doesn't have TagType::Int: '{}'", environment.config.tag_name(tag_key));
        };
        let bits = bits_needed_for_max_value((*max - *min) as u32);
        zipper.write_u32((self.0 - *min) as u32, bits);
    }
    fn import(unzipper: &mut Unzipper, environment: &Environment, tag_key: usize) -> Result<Self, ZipperError> {
        let TagType::Int { min, max } = environment.config.tag_type(tag_key) else {
            panic!("TagValue::Int doesn't have TagType::Int: '{}'", environment.config.tag_name(tag_key));
        };
        let bits = bits_needed_for_max_value((*max - *min) as u32);
        Ok(Self((unzipper.read_u32(bits)? as i32 + *min).min(*max)))
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct UniqueId {
    environment: Environment,
    tag: usize,
    id: usize,
}

impl<D: Direction> From<Arc<UniqueId>> for TagValue<D> {
    fn from(value: Arc<UniqueId>) -> Self {
        Self::Unique(value)
    }
}

impl UniqueId {
    pub(crate) const MAX_VALUE: usize = MAX_AREA as usize * 100 - 1;

    pub fn add_to_pool(&self, pool: &mut FxHashSet<usize>) {
        pool.insert(self.id);
    }

    pub fn new(environment: &Environment, tag_key: usize, random: f32) -> Option<Arc<Self>> {
        // "add_unique_id" isn't needed here
        // because "generate_unique_id" automatically adds the generated id to the pool
        let id = environment.generate_unique_id(tag_key, random)?;
        Some(Arc::new(Self {
            environment: environment.clone(),
            tag: tag_key,
            id,
        }))
    }

    pub fn get_id(&self) -> usize {
        self.id
    }

    fn export(&self, zipper: &mut Zipper) {
        let bits = bits_needed_for_max_value(Self::MAX_VALUE as u32);
        zipper.write_u32(self.id as u32, bits);
    }

    fn import(unzipper: &mut Unzipper, environment: &Environment, tag_key: usize) -> Result<Arc<Self>, ZipperError> {
        let bits = bits_needed_for_max_value(Self::MAX_VALUE as u32);
        let id = Self::MAX_VALUE.min(unzipper.read_u32(bits)? as usize);
        environment.add_unique_id(tag_key, id);
        Ok(Arc::new(Self {
            environment: environment.clone(),
            tag: tag_key,
            id,
        }))
    }
}

impl Drop for UniqueId {
    fn drop(&mut self) {
        self.environment.remove_unique_id(self.tag, self.id);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FlagKey(pub usize);

impl SupportedZippable<&Environment> for FlagKey {
    fn export(&self, zipper: &mut Zipper, support: &Environment) {
        zipper.write_u32(self.0 as u32, bits_needed_for_max_value(support.config.flag_count() as u32));
    }
    fn import(unzipper: &mut Unzipper, support: &Environment) -> Result<Self, ZipperError> {
        Ok(Self(unzipper.read_u32(bits_needed_for_max_value(support.config.flag_count() as u32))? as usize))
    }
}

impl FromConfig for FlagKey {
    fn from_conf<'a>(s: &'a str, loader: &mut crate::config::file_loader::FileLoader) -> Result<(Self, &'a str), crate::config::ConfigParseError> {
        let (base, s) = crate::config::parse::string_base(s);
        match loader.flags.iter().position(|name| name.as_str() == base) {
            Some(i) => Ok((Self(i), s)),
            None => Err(crate::config::ConfigParseError::UnknownEnumMember(format!("FlagKey::{}", base.to_string())))
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TagKey(pub usize);

impl SupportedZippable<&Environment> for TagKey {
    fn export(&self, zipper: &mut Zipper, support: &Environment) {
        zipper.write_u32(self.0 as u32, bits_needed_for_max_value(support.config.tag_count() as u32));
    }
    fn import(unzipper: &mut Unzipper, support: &Environment) -> Result<Self, ZipperError> {
        Ok(Self(unzipper.read_u32(bits_needed_for_max_value(support.config.tag_count() as u32))? as usize))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TagKeyValues<const K: usize, D: Direction>(pub TagKey, pub [TagValue<D>; K]);
impl<const K: usize, D: Direction> SupportedZippable<&Environment> for TagKeyValues<K, D> {
    fn export(&self, zipper: &mut Zipper, support: &Environment) {
        self.0.export(zipper, support);
        for value in &self.1 {
            value.export(zipper, support, self.0.0);
        }
    }
    fn import(unzipper: &mut Unzipper, support: &Environment) -> Result<Self, ZipperError> {
        let key = TagKey::import(unzipper, support)?;
        let mut values = Vec::with_capacity(K);
        for _ in 0..K {
            values.push(TagValue::import(unzipper, support, key.0)?);
        }
        Ok(Self(key, values.try_into().unwrap()))
    }
}

impl FromConfig for TagKey {
    fn from_conf<'a>(s: &'a str, loader: &mut crate::config::file_loader::FileLoader) -> Result<(Self, &'a str), crate::config::ConfigParseError> {
        let (base, s) = crate::config::parse::string_base(s);
        match loader.tags.iter().position(|name| name.as_str() == base) {
            Some(i) => Ok((Self(i), s)),
            None => Err(crate::config::ConfigParseError::UnknownEnumMember(format!("TagKey::{}", base.to_string())))
        }
    }
}


#[export_module]
mod tag_module {
    pub type UniqueId = Arc<super::UniqueId>;

    #[rhai_fn(pure, name = "==")]
    pub fn eq(u1: &mut UniqueId, u2: UniqueId) -> bool {
        u1.id == u2.id
    }
    #[rhai_fn(pure, name = "!=")]
    pub fn neq(u1: &mut UniqueId, u2: UniqueId) -> bool {
        u1.id != u2.id
    }

    #[rhai_fn(pure, name = "to_string")]
    pub fn to_string(id: &mut UniqueId) -> String {
        format!("[{}]", id.id)
    }
    #[rhai_fn(pure, name = "to_debug")]
    pub fn to_debug(id: &mut UniqueId) -> String {
        format!("[{}]", id.id)
    }
}

def_package! {
    pub TagPackage(module)
    {
        combine_with_exported_module!(module, "tag_module", tag_module);
    } |> |_engine| {
    }
}

#[cfg(test)]
pub mod tests {
    use std::sync::Arc;

    use crate::config::config::Config;
    use crate::config::environment::Environment;
    use crate::map::point_map::MapSize;

    use super::UniqueId;

    pub const FLAG_ZOMBIFIED: usize = 0;
    pub const FLAG_UNMOVED: usize = 1;
    pub const FLAG_EXHAUSTED: usize = 2;
    pub const FLAG_REPAIRING: usize = 3;
    pub const FLAG_CAPTURING: usize = 4;
    pub const FLAG_STUNNED: usize = 5;
    
    pub const TAG_HP: usize = 0;
    pub const TAG_DRONE_STATION_ID: usize = 1;
    pub const TAG_DRONE_ID: usize = 2;
    pub const TAG_LEVEL: usize = 3;
    pub const TAG_EN_PASSANT: usize = 4;
    pub const TAG_HERO_ORIGIN: usize = 5;
    pub const TAG_PAWN_DIRECTION: usize = 6;
    pub const TAG_ANGER: usize = 8;
    pub const TAG_BUILT_THIS_TURN: usize = 9;
    pub const TAG_CAPTURE_OWNER: usize = 10;
    pub const TAG_CAPTURE_PROGRESS: usize = 11;
    pub const TAG_UNIT_TYPE: usize = 12;
    pub const TAG_MOVEMENT_TYPE: usize = 13;
    pub const TAG_SLUDGE_COUNTER: usize = 14;
    pub const TAG_COINS: usize = 15;
    #[test_log::test]
    fn verify_tag_test_constants() {
        let config = Arc::new(Config::test_config());
        let environment = Environment::new_map(config, MapSize::new(5, 5));
        assert_eq!(environment.config.flag_name(FLAG_ZOMBIFIED), "Zombified");
        assert_eq!(environment.config.flag_name(FLAG_UNMOVED), "Unmoved");
        assert_eq!(environment.config.flag_name(FLAG_EXHAUSTED), "Exhausted");
        assert_eq!(environment.config.flag_name(FLAG_REPAIRING), "Repairing");
        assert_eq!(environment.config.flag_name(FLAG_CAPTURING), "Capturing");
        assert_eq!(environment.config.flag_name(FLAG_STUNNED), "Stunned");
        assert_eq!(environment.config.tag_name(TAG_HP), "Hp");
        assert_eq!(environment.config.tag_name(TAG_DRONE_STATION_ID), "DroneStationId");
        assert_eq!(environment.config.tag_name(TAG_DRONE_ID), "DroneId");
        assert_eq!(environment.config.tag_name(TAG_LEVEL), "Level");
        assert_eq!(environment.config.tag_name(TAG_EN_PASSANT), "EnPassant");
        assert_eq!(environment.config.tag_name(TAG_HERO_ORIGIN), "HeroOrigin");
        assert_eq!(environment.config.tag_name(TAG_PAWN_DIRECTION), "PawnDirection");
        assert_eq!(environment.config.tag_name(TAG_ANGER), "Anger");
        assert_eq!(environment.config.tag_name(TAG_BUILT_THIS_TURN), "BuiltThisTurn");
        assert_eq!(environment.config.tag_name(TAG_CAPTURE_OWNER), "CaptureOwner");
        assert_eq!(environment.config.tag_name(TAG_CAPTURE_PROGRESS), "CaptureProgress");
        assert_eq!(environment.config.tag_name(TAG_UNIT_TYPE), "UnitType");
        assert_eq!(environment.config.tag_name(TAG_MOVEMENT_TYPE), "MovementType");
        assert_eq!(environment.config.tag_name(TAG_SLUDGE_COUNTER), "SludgeCounter");
        assert_eq!(environment.config.tag_name(TAG_COINS), "Coins");
    }

    #[test_log::test]
    fn unique_ids_are_unique() {
        let config = Arc::new(Config::test_config());
        let environment = Environment::new_map(config, MapSize::new(5, 5));
        // ids get dropped and freed immediately, so all ids are the same (since the rng is fixed)
        for _ in 0..10 {
            assert_eq!(UniqueId::new(&environment, TAG_DRONE_STATION_ID, 0.).unwrap().id, 0);
            assert_eq!(UniqueId::new(&environment, TAG_DRONE_ID, 0.).unwrap().id, 0);
        }
        // now ids don't get dropped, so all ids are sequential (since the rng is fixed)
        let mut ids = Vec::new();
        for i in 0..10 {
            let uid = UniqueId::new(&environment, TAG_DRONE_STATION_ID, 0.).unwrap();
            assert_eq!(uid.id, i * 2);
            ids.push(uid);
            let uid = UniqueId::new(&environment, TAG_DRONE_ID, 0.).unwrap();
            assert_eq!(uid.id, i * 2 + 1);
            ids.push(uid);
        }
    }
}