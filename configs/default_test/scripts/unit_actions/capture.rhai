
fn uf_can_capture(CONTEXT) {
    let terrain = CONTEXT.board.get_terrain(CONTEXT.position);
    terrain.owner_id != ()
    && terrain.team != CONTEXT.unit.team
    && CONTEXT.config.table_entry("TERRAIN_CAPTURE", CONTEXT.unit.type, terrain.type)
}

fn ua_capture(CONTEXT) {
    let capture_strength = CONTEXT.unit.update_custom_value(CONTEXT.board, CONTEXT.position, "CaptureStrength", Fraction(1));
    if capture_strength.ceil() > 0 {
        CONTEXT.event_handler.set_unit_flag(CONTEXT.position, FLAG_Capturing);
    } else {
        // Julia power
        finish_capturing(CONTEXT.event_handler, CONTEXT.position, CONTEXT.unit.owner_id);
    }
    CONTEXT.event_handler.set_unit_flag(CONTEXT.position, FLAG_Exhausted);
}
