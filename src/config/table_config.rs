use std::error::Error;

use rhai::Dynamic;
use rustc_hash::{FxHashMap as HashMap, FxHashSet as HashSet};

use crate::config::parse::*;
use crate::terrain::*;
use crate::units::unit_types::UnitType;

use super::file_loader::{FileLoader, TableLine};
use super::ConfigParseError;

pub type CustomTable = HashMap<(TableAxisKey, TableAxisKey), TableValue>;

#[derive(Debug)]
pub struct TableConfig {
    pub(super) id: String,
    pub(super) path: String,
    pub(super) typ: TableType,
    pub(super) default_value: TableValue,
    pub(super) left: TableAxis,
    pub(super) top: TableAxis,
}

impl TableLine for TableConfig {
    type Header = TableConfigHeader;
    fn parse(data: &HashMap<Self::Header, &str>, loader: &mut FileLoader) -> Result<Self, Box<dyn Error>> {
        use TableConfigHeader as H;
        use ConfigParseError as E;
        let get = |key| {
            data.get(&key).ok_or(E::MissingColumn(format!("{key:?}")))
        };
        let typ = parse(data, H::Type, loader)?;
        let default_value = TableValue::from_conf(typ, get(H::DefaultValue)?, loader)?;
        let result = Self {
            id: get(H::Id)?.trim().to_string(),
            path: get(H::Path)?.trim().to_string(),
            typ,
            default_value,
            left: parse(data, H::Left, loader)?,
            top: parse(data, H::Top, loader)?,
        };
        Ok(result)
    }

    fn simple_validation(&self) -> Result<(), Box<dyn Error>> {
        if self.id.trim().len() == 0 {
            return Err(Box::new(ConfigParseError::NameTooShort));
        }
        Ok(())
    }
}

impl TableConfig {
    pub(crate) fn build_table(&self, loader: &mut FileLoader) -> Result<CustomTable, Box<dyn Error>> {
        let data = loader.load_config(&format!("tables/{}", self.path))?;
        let mut reader = csv::ReaderBuilder::new().delimiter(b';').from_reader(data.as_bytes());
        // TODO: ensure uniqueness of column and row IDs
        let mut headers: Vec<TableAxisKey> = Vec::new();
        for h in reader.headers()?.into_iter().skip(1) {
            let header = TableAxisKey::from_conf(self.top, h, loader)?;
            if headers.contains(&header) {
                return Err(Box::new(ConfigParseError::DuplicateHeader(h.to_string())))
            }
            headers.push(header);
        }
        let mut rows: HashSet<TableAxisKey> = HashSet::default();
        let mut result = HashMap::default();
        for line in reader.records() {
            let line = line?;
            let mut line = line.into_iter();
            let left: TableAxisKey = match line.next() {
                Some(t) => TableAxisKey::from_conf(self.left, t, loader)?,
                _ => continue,
            };
            if !rows.insert(left) {
                return Err(Box::new(ConfigParseError::DuplicateHeader(format!("{left:?}"))))
            }
            for (i, value) in line.enumerate() {
                let value = value.trim();
                if value.len() == 0 {
                    continue;
                }
                let value = TableValue::from_conf(self.typ, value, loader)?;
                if value != self.default_value {
                    result.insert((headers[i], left), value);
                }
            }
        }
        Ok(result)
    }
}

crate::listable_enum! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum TableConfigHeader {
        Id,
        Path,
        Type,
        DefaultValue,
        Left,
        Top,
    }
}


crate::listable_enum! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum TableType {
        Boolean,
        Int,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TableValue {
    Bool(bool),
    Int(i32),
}

impl TableValue {
    fn from_conf(typ: TableType, s: &str, loader: &mut FileLoader) -> Result<Self, ConfigParseError> {
        match typ {
            TableType::Boolean => {
                Ok(Self::Bool(bool::from_conf(s, loader)?.0))
            }
            TableType::Int => {
                Ok(Self::Int(i32::from_conf(s, loader)?.0))
            }
        }
    }
}

impl From<TableValue> for Dynamic {
    fn from(value: TableValue) -> Self {
        match value {
            TableValue::Bool(v) => v.into(),
            TableValue::Int(v) => v.into(),
        }
    }
}


crate::listable_enum! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum TableAxis {
        Unit,
        Terrain,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TableAxisKey {
    Unit(UnitType),
    Terrain(TerrainType),
}

impl TableAxisKey {
    fn from_conf(axis: TableAxis, s: &str, loader: &mut FileLoader) -> Result<Self, ConfigParseError> {
        match axis {
            TableAxis::Unit => {
                Ok(Self::Unit(UnitType::from_conf(s, loader)?.0))
            }
            TableAxis::Terrain => {
                Ok(Self::Terrain(TerrainType::from_conf(s, loader)?.0))
            }
        }
    }

    pub fn from_dynamic(value: Dynamic) -> Option<Self> {
        match value.type_name().split("::").last()? {
            "UnitType" => Some(Self::Unit(value.try_cast()?)),
            "TerrainType" => Some(Self::Terrain(value.try_cast()?)),
            _ => None
        }
    }
}
