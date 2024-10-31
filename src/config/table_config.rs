use std::error::Error;

use num_rational::Rational32;
use rhai::Dynamic;
use rustc_hash::FxHashMap as HashMap;

use crate::config::parse::*;
use crate::terrain::*;
use crate::units::hero::HeroType;
use crate::units::movement::MovementType;
use crate::units::unit_types::UnitType;

use super::file_loader::{FileLoader, TableLine};
use super::ConfigParseError;

pub struct CustomTable {
    pub default_value: TableValue,
    pub values: HashMap<(TableAxisKey, TableAxisKey),  TableValue>,
    pub row_keys: Vec<TableAxisKey>,
    pub column_keys: Vec<TableAxisKey>,
}

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
        let mut row_keys: Vec<TableAxisKey> = Vec::new();
        let mut values = HashMap::default();
        for line in reader.records() {
            let line = line?;
            let mut line = line.into_iter();
            let left: TableAxisKey = match line.next() {
                Some(t) => TableAxisKey::from_conf(self.left, t, loader)?,
                _ => continue,
            };
            if row_keys.contains(&left) {
                return Err(Box::new(ConfigParseError::DuplicateHeader(format!("{left:?}"))))
            }
            row_keys.push(left);
            for (i, value) in line.enumerate() {
                let value = value.trim();
                if value.len() == 0 {
                    continue;
                }
                let value = TableValue::from_conf(self.typ, value, loader)?;
                if value != self.default_value {
                    values.insert((headers[i], left), value);
                }
            }
        }
        Ok(CustomTable {
            values,
            default_value: self.default_value,
            column_keys: headers,
            row_keys,
        })
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
        Fraction,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TableValue {
    Bool(bool),
    Int(i32),
    Fraction(Rational32),
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
            TableType::Fraction => {
                Ok(Self::Fraction(Rational32::from_conf(s, loader)?.0))
            }
        }
    }

    pub fn from_dynamic(value: Dynamic) -> Option<Self> {
        match value.type_name().split("::").last().unwrap() {
            "bool" => Some(Self::Bool(value.try_cast()?)),
            "i32" => Some(Self::Int(value.try_cast()?)),
            "Ratio<i32>" => Some(Self::Fraction(value.try_cast()?)),
            _ => None
        }
    }
    pub fn into_dynamic(self) -> Dynamic {
        match self {
            Self::Bool(v) => v.into(),
            Self::Int(v) => v.into(),
            Self::Fraction(v) => Dynamic::from(v),
        }
    }
}


crate::listable_enum! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum TableAxis {
        Unit,
        Terrain,
        Hero,
        Movement,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TableAxisKey {
    Unit(UnitType),
    Terrain(TerrainType),
    Hero(HeroType),
    Movement(MovementType),
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
            TableAxis::Hero => {
                Ok(Self::Hero(HeroType::from_conf(s, loader)?.0))
            }
            TableAxis::Movement => {
                Ok(Self::Movement(MovementType::from_conf(s, loader)?.0))
            }
        }
    }

    pub fn from_dynamic(value: Dynamic) -> Option<Self> {
        match value.type_name().split("::").last().unwrap() {
            "UnitType" => Some(Self::Unit(value.try_cast()?)),
            "TerrainType" => Some(Self::Terrain(value.try_cast()?)),
            "HeroType" => Some(Self::Hero(value.try_cast()?)),
            "MovementType" => Some(Self::Movement(value.try_cast()?)),
            _ => None
        }
    }
    pub fn into_dynamic(self) -> Dynamic {
        match self {
            Self::Unit(value) => Dynamic::from(value),
            Self::Terrain(value) => Dynamic::from(value),
            Self::Hero(value) => Dynamic::from(value),
            Self::Movement(value) => Dynamic::from(value),
        }
    }
}
