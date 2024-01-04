use serde::Deserialize;

use crate::commander::commander_type::CommanderType;

/**
 * contains data that shouldn't change when using a different power
 */
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct CommanderTypeConfig {
    pub(super) id: CommanderType,
    pub(super) name: String,
    #[serde(default)]
    pub(super) transport_capacity: u8,
    pub(super) max_charge: u32,
}
