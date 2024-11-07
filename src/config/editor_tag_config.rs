use std::error::Error;
use std::fmt::Debug;
use std::hash::Hash;

use rustc_hash::FxHashMap as HashMap;

use crate::tags::{FlagKey, TagKey};

use super::file_loader::FileLoader;
use super::parse::FromConfig;
use super::ConfigParseError;


crate::listable_enum! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum TagEditorVisibility {
        // never visible in editor
        Hidden,
        // visible in advanced mode, inactive by default
        Advanced,
        // always visible, inactive by default (null values allowed)
        Normal,
        // always visible, activated by default (no null values)
        Always,
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Either<T, U> where
    T: Debug + Clone + PartialEq,
    U: Debug + Clone + PartialEq,
{
    Left(T),
    Right(U),
}

pub(super) fn parse<T: FromConfig + Clone + Hash + Eq>(filename: &str, loader: &mut FileLoader) -> Result<[HashMap<(usize, T), TagEditorVisibility>; 2], Box<dyn Error>> {
    let data = loader.load_config(filename)?;
    let mut reader = csv::ReaderBuilder::new().delimiter(b';').from_reader(data.as_bytes());
    // TODO: ensure uniqueness of column and row IDs
    let mut headers: Vec<Either<FlagKey, TagKey>> = Vec::new();
    for h in reader.headers()?.into_iter().skip(1) {
        let header = FlagKey::from_conf(h, loader).map(|k| Either::Left(k.0))
        .or(TagKey::from_conf(h, loader).map(|k| Either::Right(k.0)))?;
        if headers.contains(&header) {
            return Err(Box::new(ConfigParseError::DuplicateHeader(h.to_string())))
        }
        headers.push(header);
    }
    let mut flags = HashMap::default();
    let mut tags = HashMap::default();
    for line in reader.records() {
        let line = line?;
        let mut line = line.into_iter();
        let row_key: T = match line.next() {
            Some(t) => T::from_conf(t, loader)?.0,
            _ => continue,
        };
        for (i, val) in line.enumerate() {
            if val.len() > 0 && i < headers.len() {
                let visibility = TagEditorVisibility::from_conf(val, loader)?.0;
                match headers[i].clone() {
                    Either::Left(flag) => flags.insert((flag.0, row_key.clone()), visibility),
                    Either::Right(tag) => tags.insert((tag.0, row_key.clone()), visibility),
                };
            }
        }
    }
    Ok([flags, tags])
}
