
fn spawn_tentacle() {
    EVENT_HANDLER.place_unit(POSITION, CONFIG.build_unit("Tentacle").build());
}

fn get_captured() {
    let unit = BOARD.get_unit(POSITION);
    if unit == () || unit.owner_id != TERRAIN.get(TAG_CaptureOwner) || !CONFIG.table_entry("TERRAIN_CAPTURE", unit.type, TERRAIN.type) {
        EVENT_HANDLER.remove_terrain_tag(POSITION, TAG_CaptureOwner);
        EVENT_HANDLER.remove_terrain_tag(POSITION, TAG_CaptureProgress);
    }
    if unit?.owner_id == OWNER_ID && unit.team != TERRAIN.team && unit.has(FLAG_Capturing) && CONFIG.table_entry("TERRAIN_CAPTURE", unit.type, TERRAIN.type) {
        let progress = (TERRAIN.get(TAG_CaptureProgress) ?? 0) + Fraction(unit.get(TAG_Hp) ?? 100, 10).ceil();
        if progress >= 10 {
            let terrain = Terrain(TERRAIN.type);
            terrain.copy_from(TERRAIN);
            terrain.remove(TAG_CaptureOwner);
            terrain.remove(TAG_CaptureProgress);
            terrain.owner_id = unit.owner_id;
            EVENT_HANDLER.replace_terrain(POSITION, terrain);
        } else {
            EVENT_HANDLER.set_terrain_tag(POSITION, TAG_CaptureOwner, unit.owner_id);
            EVENT_HANDLER.set_terrain_tag(POSITION, TAG_CaptureProgress, progress);
        }
    }
}
