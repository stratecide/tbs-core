use serde::Deserialize;

use crate::commander::commander_type::CommanderType;
use crate::script::player::PlayerScript;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct CommanderPowerConfig {
    pub(super) id: CommanderType,
    pub(crate) name: String, // of the ability
    #[serde(default)]
    pub(crate) usable_from_power: Vec<u8>,
    pub(crate) required_charge: u32,
    pub(crate) effects: Vec<PlayerScript>,
    #[serde(default)]
    pub(crate) next_power: u8, // at the start of the player's turn, this index is automatically switched to if possible (e.g. player has enough charge)
}
