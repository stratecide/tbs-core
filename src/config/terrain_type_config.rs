use serde::Deserialize;
use num_rational::Rational32;

use crate::game::fog::VisionMode;
use crate::script::unit::UnitScript;
use crate::terrain::*;

use super::movement_type_config::MovementPattern;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct TerrainTypeConfig {
    pub(super) id: TerrainType,
    pub(super) name: String,
    pub(super) needs_owner: bool,
    pub(super) capture_resistance: u8,
    #[serde(default)]
    pub(super) update_amphibious: Option<AmphibiousTyping>,
    pub(super) max_capture_progress: u8,
    pub(super) max_builds_per_turn: u8,
    pub(super) max_anger: u8,
    pub(super) vision_range: Option<u8>,
    pub(super) income_factor: i16,
    pub(super) can_repair: bool,
    pub(super) can_build: bool,
    pub(super) can_sell_hero: bool,
    pub(super) chess: bool,
    #[serde(default)]
    pub(super) extra_movement_options: ExtraMovementOptions,
}
