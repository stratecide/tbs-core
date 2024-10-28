mod unit_type_config;
pub mod movement_type_config;
mod terrain_type_config;
mod hero_type_config;
mod commander_type_config;
mod commander_power_config;
mod commander_unit_config;
mod unit_filter;
mod number_modification;
pub mod config;
pub mod parse;
pub mod environment;
mod custom_action_config;
mod hero_power_config;
mod terrain_powered;
pub mod file_loader;
pub mod table_config;
pub mod tag_config;
pub mod token_typ_config;

use std::fmt::Debug;
use std::fmt::Display;
use std::error::Error;
use std::path::PathBuf;

use crate::commander::commander_type::CommanderType;
use crate::units::hero::HeroType;

#[derive(Debug, Clone)]
pub enum ConfigParseError {
    CommanderMaxChargeExceeded(u32),
    CustomActionScriptMissing(String),
    DontCallGlobalScriptDirectly(String),
    DuplicateEntry(String),
    DuplicateHeader(String),
    EmptyList,
    FileMissing(String),
    FolderMissing(PathBuf),
    HeroMaxChargeExceeded(u8),
    InvalidCellData(&'static str, usize, usize, String),
    InvalidColumnValue(String, String),
    InvalidBool(String),
    InvalidInteger(String),
    DivisionByZero(i32),
    InvalidNumber(String),
    InvalidNumberModifier(String),
    MissingColumn(String),
    MissingCommanderForAttributes(CommanderType),
    MissingCommanderForPower(CommanderType),
    MissingHeroForPower(HeroType),
    NameTooShort,
    NotEnoughValues(String),
    NumberTooBig(String),
    ScriptCompilation(String, String),
    ScriptFunctionNotFound(String, String),
    ScriptNeedsFileAndFunctionName(String),
    TooManyPowers(CommanderType, usize),
    UnknownEnumMember(String),
    TableAxesShouldDiffer(String),
    TableEmpty,
    NotEnoughPlayerColors,
    MissingUnit(String),
    MissingTerrain(String),
    MissingNeutralColor,
    MissingToken(String),
    InvalidColor(String),
    Other(String),
}

impl Display for ConfigParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // TODO
        write!(f, "{self:?}")
        /*match self {
            Self::InvalidCellData(file, line, column, cell_data) => {
                write!(f, "Invalid Cell Data in file {file} at line {line}, column {column}: '{cell_data}'")
            }
        }*/
    }
}

impl Error for ConfigParseError {}

