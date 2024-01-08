mod unit_type_config;
pub mod movement_type_config;
mod terrain_type_config;
mod hero_type_config;
mod commander_type_config;
mod commander_power_config;
mod commander_unit_config;
mod unit_filter;
pub mod config;
pub mod environment;

use std::fmt::Debug;
use std::fmt::Display;
use std::error::Error;

use crate::commander::commander_type::CommanderType;
#[derive(Debug, Clone)]
pub enum ConfigParseError {
    InvalidCellData(&'static str, usize, usize, String),
    MissingCommanderForPower(CommanderType),
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

