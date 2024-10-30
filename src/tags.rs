use rhai::*;
use rhai::plugin::*;
use rustc_hash::{FxHashMap, FxHashSet};
use zipper::*;
use zipper_derive::Zippable;

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
            write!(f, "{}", environment.flag_name(*flag))?;
        }
        write!(f, "], TAGS[")?;
        for (key, value) in &self.tags {
            write!(f, "{}=", environment.tag_name(*key))?;
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
            if environment.flag_visibility(*flag) >= minimum_visibility {
                result.flags.push(*flag);
            }
        }
        for (key, value) in &self.tags {
            if environment.tag_visibility(*key) >= minimum_visibility {
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
        if flag >= environment.flag_count() {
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

    pub fn is_tag_set(&self, key: usize) -> bool {
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
        let flag_bits = bits_needed_for_max_value(support.flag_count() as u32);
        zipper.write_u32(self.flags.len() as u32, flag_bits);
        for flag in &self.flags {
            zipper.write_u32(*flag as u32, flag_bits);
        }
        let tag_bits = bits_needed_for_max_value(support.tag_count() as u32);
        zipper.write_u32(self.tags.len() as u32, tag_bits);
        for (key, value) in &self.tags {
            zipper.write_u32(*key as u32, tag_bits);
            value.export(zipper, &(support.clone(), *key));
        }
    }
    fn import(unzipper: &mut Unzipper, support: &Environment) -> Result<Self, ZipperError> {
        let mut result = Self::new();
        let flag_bits = bits_needed_for_max_value(support.flag_count() as u32);
        let flag_count = unzipper.read_u32(flag_bits)?.min(support.flag_count() as u32);
        for _ in 0..flag_count {
            result.set_flag(support, unzipper.read_u32(flag_bits)? as usize);
        }
        let tag_bits = bits_needed_for_max_value(support.tag_count() as u32);
        let tag_count = unzipper.read_u32(tag_bits)?.min(support.tag_count() as u32);
        for _ in 0..tag_count {
            let key = support.tag_count().min(unzipper.read_u32(tag_bits)? as usize);
            let value = TagValue::import(unzipper, &(support.clone(), key))?;
            result.set_tag(support, key, value);
        }
        Ok(result)
    }
}

pub(crate) type TagValueZipSupport = (Environment, usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Zippable)]
#[zippable(bits = 4, support_ref=TagValueZipSupport)]
pub enum TagValue<D: Direction> {
    Unique(UniqueId),
    Int(Int32),
    #[supp(&support.0)]
    Point(Point),
    Direction(D),
    #[supp(&support.0)]
    UnitType(UnitType),
    #[supp(&support.0)]
    TerrainType(TerrainType),
    #[supp(&support.0)]
    MovementType(MovementType),
}

impl<D: Direction> TagValue<D> {
    pub(crate) fn has_valid_type(&self, environment: &Environment, key: usize) -> bool {
        match (self, environment.tag_type(key)) {
            (Self::Unique(_), TagType::Unique { .. }) => true,
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
            Self::Unique(value) => Dynamic::from(*value),
            Self::UnitType(value) => Dynamic::from(*value),
            Self::MovementType(value) => Dynamic::from(*value),
        }
    }

    pub fn from_dynamic(value: Dynamic, key: usize, environment: &Environment) -> Option<Self> {
        let result = match value.type_name().split("::").last().unwrap() {
            "Direction" => Some(Self::Direction(value.cast())),
            "i32" => {
                let TagType::Int { min, max } = environment.tag_type(key) else {
                    return None;
                };
                Some(Self::Int(Int32(value.cast::<i32>().max(*min).min(*max))))
            }
            "Point" => Some(Self::Point(value.cast())),
            "TerrainType" => Some(Self::TerrainType(value.try_cast()?)),
            "UnitType" => Some(Self::UnitType(value.try_cast()?)),
            "UniqueId" => Some(Self::Unique(value.try_cast()?)),
            _ => None
        }?;
        Some(result)
    }
}

impl<D: Direction> From<i32> for TagValue<D> {
    fn from(value: i32) -> Self {
        Self::Int(Int32(value))
    }
}

impl<D: Direction> From<UniqueId> for TagValue<D> {
    fn from(value: UniqueId) -> Self {
        Self::Unique(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Int32(pub i32);
impl SupportedZippable<&TagValueZipSupport> for Int32 {
    fn export(&self, zipper: &mut Zipper, (environment, key): &(Environment, usize)) {
        let TagType::Int { min, max } = environment.tag_type(*key) else {
            panic!("TagValue::Int doesn't have TagType::Int: '{}'", environment.tag_name(*key));
        };
        let bits = bits_needed_for_max_value((*max - *min) as u32);
        zipper.write_u32((self.0 - *min) as u32, bits);
    }
    fn import(unzipper: &mut Unzipper, (environment, key): &(Environment, usize)) -> Result<Self, ZipperError> {
        let TagType::Int { min, max } = environment.tag_type(*key) else {
            panic!("TagValue::Int doesn't have TagType::Int: '{}'", environment.tag_name(*key));
        };
        let bits = bits_needed_for_max_value((*max - *min) as u32);
        Ok(Self((unzipper.read_u32(bits)? as i32 + *min).min(*max)))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UniqueId(usize);

impl UniqueId {
    // has to be 7 or fewer decimal digits according to Wikipedia
    // https://en.wikipedia.org/wiki/Single-precision_floating-point_format
    const MAX_VALUE: usize = MAX_AREA as usize * 100 - 1;

    pub fn add_to_pool(&self, pool: &mut FxHashSet<usize>) {
        pool.insert(self.0);
    }

    pub fn new(pool: &FxHashSet<usize>, random: f32) -> Option<Self> {
        if pool.len() > Self::MAX_VALUE {
            return None;
        }
        let count = Self::MAX_VALUE + 1;
        let mut i = (count as f32 * random) as usize;
        while pool.contains(&i) {
            i = (i + 1) % count;
        }
        Some(Self(i))
    }
}

impl Zippable for UniqueId {
    fn zip(&self, zipper: &mut Zipper) {
        let bits = bits_needed_for_max_value(Self::MAX_VALUE as u32);
        zipper.write_u32(self.0 as u32, bits);
    }
    fn unzip(unzipper: &mut Unzipper) -> Result<Self, ZipperError> {
        let bits = bits_needed_for_max_value(Self::MAX_VALUE as u32);
        Ok(Self(Self::MAX_VALUE.min(unzipper.read_u32(bits)? as usize)))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FlagKey(pub usize);

impl SupportedZippable<&Environment> for FlagKey {
    fn export(&self, zipper: &mut Zipper, support: &Environment) {
        zipper.write_u32(self.0 as u32, bits_needed_for_max_value(support.flag_count() as u32));
    }
    fn import(unzipper: &mut Unzipper, support: &Environment) -> Result<Self, ZipperError> {
        Ok(Self(unzipper.read_u32(bits_needed_for_max_value(support.flag_count() as u32))? as usize))
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
        zipper.write_u32(self.0 as u32, bits_needed_for_max_value(support.tag_count() as u32));
    }
    fn import(unzipper: &mut Unzipper, support: &Environment) -> Result<Self, ZipperError> {
        Ok(Self(unzipper.read_u32(bits_needed_for_max_value(support.tag_count() as u32))? as usize))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TagKeyValues<const K: usize, D: Direction>(pub TagKey, pub [TagValue<D>; K]);
impl<const K: usize, D: Direction> SupportedZippable<&Environment> for TagKeyValues<K, D> {
    fn export(&self, zipper: &mut Zipper, support: &Environment) {
        self.0.export(zipper, support);
        let support = (support.clone(), self.0.0);
        for value in &self.1 {
            value.export(zipper, &support);
        }
    }
    fn import(unzipper: &mut Unzipper, support: &Environment) -> Result<Self, ZipperError> {
        let key = TagKey::import(unzipper, support)?;
        let support = (support.clone(), key.0);
        let mut values = Vec::with_capacity(K);
        for _ in 0..K {
            values.push(TagValue::import(unzipper, &support)?);
        }
        Ok(Self(key, values.try_into().unwrap()))
    }
}


#[export_module]
mod tag_module {
    pub type UniqueId = super::UniqueId;

    #[rhai_fn(pure, name = "==")]
    pub fn eq(u1: &mut UniqueId, u2: UniqueId) -> bool {
        *u1 == u2
    }
    #[rhai_fn(pure, name = "!=")]
    pub fn neq(u1: &mut UniqueId, u2: UniqueId) -> bool {
        *u1 != u2
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

    pub const FLAG_ZOMBIFIED: usize = 0;
    pub const FLAG_EXHAUSTED: usize = 2;
    pub const FLAG_REPAIRING: usize = 3;
    pub const FLAG_CAPTURING: usize = 4;
    
    pub const TAG_HP: usize = 0;
    pub const TAG_DRONE_STATION_ID: usize = 1;
    pub const TAG_DRONE_ID: usize = 2;
    pub const TAG_LEVEL: usize = 3;
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
    #[test]
    fn verify_tag_test_constants() {
        let config = Arc::new(Config::test_config());
        let environment = Environment::new_map(config, MapSize::new(5, 5));
        assert_eq!(environment.flag_name(FLAG_EXHAUSTED), "Exhausted");
        assert_eq!(environment.flag_name(FLAG_REPAIRING), "Repairing");
        assert_eq!(environment.flag_name(FLAG_CAPTURING), "Capturing");
        assert_eq!(environment.tag_name(TAG_HP), "Hp");
        assert_eq!(environment.tag_name(TAG_DRONE_STATION_ID), "DroneStationId");
        assert_eq!(environment.tag_name(TAG_DRONE_ID), "DroneId");
        assert_eq!(environment.tag_name(TAG_HERO_ORIGIN), "HeroOrigin");
        assert_eq!(environment.tag_name(TAG_PAWN_DIRECTION), "PawnDirection");
        assert_eq!(environment.tag_name(TAG_ANGER), "Anger");
        assert_eq!(environment.tag_name(TAG_BUILT_THIS_TURN), "BuiltThisTurn");
        assert_eq!(environment.tag_name(TAG_CAPTURE_OWNER), "CaptureOwner");
        assert_eq!(environment.tag_name(TAG_CAPTURE_PROGRESS), "CaptureProgress");
        assert_eq!(environment.tag_name(TAG_UNIT_TYPE), "UnitType");
        assert_eq!(environment.tag_name(TAG_MOVEMENT_TYPE), "MovementType");
        assert_eq!(environment.tag_name(TAG_SLUDGE_COUNTER), "SludgeCounter");
        assert_eq!(environment.tag_name(TAG_COINS), "Coins");
    }

}