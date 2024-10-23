use rustc_hash::{FxHashMap, FxHashSet};
use zipper::*;
use zipper_derive::Zippable;

use crate::config::environment::Environment;
use crate::config::tag_config::TagType;
use crate::map::direction::Direction;
use crate::map::point::Point;
use crate::map::point_map::MAX_AREA;
use crate::map::wrapping_map::Distortion;
use crate::terrain::TerrainType;
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
            write!(f, "{}={value:?}", environment.tag_name(*key))?;
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

    pub fn get_flag(&self, flag: usize) -> bool {
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
}

impl<D: Direction> TagValue<D> {
    fn has_valid_type(&self, environment: &Environment, key: usize) -> bool {
        match (self, environment.tag_type(key)) {
            (Self::Unique(_), TagType::Unique { .. }) => true,
            (Self::Point(_), TagType::Point) => true,
            (Self::Direction(_), TagType::Direction) => true,
            (Self::UnitType(_), TagType::UnitType) => true,
            (Self::TerrainType(_), TagType::TerrainType) => true,
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
