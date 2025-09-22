
fn uf_can_capture() {
    let terrain = BOARD.get_terrain(POSITION);
    terrain.owner_id != ()
    && terrain.team != UNIT.team
    && CONFIG.table_entry("TERRAIN_CAPTURE", UNIT.type, terrain.type)
}

fn ua_capture() {
    let capture_strength = UNIT.update_custom_value(BOARD, POSITION, "CaptureStrength", Fraction(1));
    if capture_strength.ceil() > 0 {
        EVENT_HANDLER.set_unit_flag(POSITION, FLAG_Capturing);
    } else {
        // Julia power
        finish_capturing(EVENT_HANDLER, POSITION, UNIT.owner_id);
    }
    EVENT_HANDLER.set_unit_flag(POSITION, FLAG_Exhausted);
}
