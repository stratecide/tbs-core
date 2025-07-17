
fn uf_can_capture() {
    let terrain = BOARD.get_terrain(POSITION);
    terrain.owner_id != ()
    && terrain.team != UNIT.team
    && CONFIG.table_entry("TERRAIN_CAPTURE", UNIT.type, terrain.type)
}

fn ua_capture() {
    EVENT_HANDLER.set_unit_flag(POSITION, FLAG_Capturing);
    EVENT_HANDLER.set_unit_flag(POSITION, FLAG_Exhausted);
}
