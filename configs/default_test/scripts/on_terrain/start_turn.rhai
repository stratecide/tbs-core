
fn spawn_tentacle(CONTEXT) {
    let unit = Unit(CONTEXT.config, UNIT_Tentacle);
    unit.set(TAG_Hp, 100);
    CONTEXT.event_handler.place_unit(CONTEXT.position, unit);
}

fn get_captured(CONTEXT) {
    let unit = CONTEXT.board.get_unit(CONTEXT.position);
    if unit == () || unit.owner_id != CONTEXT.terrain.get(TAG_CaptureOwner) || !CONTEXT.config.table_entry("TERRAIN_CAPTURE", unit.type, CONTEXT.terrain.type) {
        CONTEXT.event_handler.remove_terrain_tag(CONTEXT.position, TAG_CaptureOwner);
        CONTEXT.event_handler.remove_terrain_tag(CONTEXT.position, TAG_CaptureProgress);
    }
    if unit?.owner_id == CONTEXT.owner_id && unit.team != CONTEXT.terrain.team && unit.has(FLAG_Capturing) && CONTEXT.config.table_entry("TERRAIN_CAPTURE", unit.type, CONTEXT.terrain.type) {
        let gain = Fraction(unit.get(TAG_Hp) ?? 100, 10);
        gain *= unit.update_custom_value(CONTEXT.board, CONTEXT.position, "CaptureStrength", Fraction(1));
        let progress = (CONTEXT.terrain.get(TAG_CaptureProgress) ?? 0) + gain.ceil();
        if progress >= 10 {
            finish_capturing(CONTEXT.event_handler, CONTEXT.position, unit.owner_id);
        } else {
            CONTEXT.event_handler.set_terrain_tag(CONTEXT.position, TAG_CaptureOwner, unit.owner_id);
            CONTEXT.event_handler.set_terrain_tag(CONTEXT.position, TAG_CaptureProgress, progress);
        }
    }
}
