
fn spawn_tentacle() {
    let unit = Unit(CONFIG, UNIT_Tentacle);
    unit.set(TAG_Hp, 100);
    EVENT_HANDLER.place_unit(POSITION, unit);
}

fn get_captured() {
    let unit = BOARD.get_unit(POSITION);
    if unit == () || unit.owner_id != TERRAIN.get(TAG_CaptureOwner) || !CONFIG.table_entry("TERRAIN_CAPTURE", unit.type, TERRAIN.type) {
        EVENT_HANDLER.remove_terrain_tag(POSITION, TAG_CaptureOwner);
        EVENT_HANDLER.remove_terrain_tag(POSITION, TAG_CaptureProgress);
    }
    if unit?.owner_id == OWNER_ID && unit.team != TERRAIN.team && unit.has(FLAG_Capturing) && CONFIG.table_entry("TERRAIN_CAPTURE", unit.type, TERRAIN.type) {
        let gain = Fraction(unit.get(TAG_Hp) ?? 100, 10);
        gain *= unit.update_custom_value(BOARD, POSITION, "CaptureStrength", Fraction(1));
        let progress = (TERRAIN.get(TAG_CaptureProgress) ?? 0) + gain.ceil();
        if progress >= 10 {
            finish_capturing(EVENT_HANDLER, POSITION, unit.owner_id);
        } else {
            EVENT_HANDLER.set_terrain_tag(POSITION, TAG_CaptureOwner, unit.owner_id);
            EVENT_HANDLER.set_terrain_tag(POSITION, TAG_CaptureProgress, progress);
        }
    }
}
