
fn build_unit_input() {
    _build_unit_input(TERRAIN, POSITION)
}

fn build_unit(data) {
    EVENT_HANDLER.set_terrain_flag(POSITION, FLAG_Exhausted);
    _build_unit(TERRAIN, POSITION, data, true);
}

fn and_turn_into_grass_input() {
    _build_unit_input(TERRAIN, POSITION)
}

fn and_turn_into_grass(data) {
    EVENT_HANDLER.replace_terrain(POSITION, Terrain("Grass"));
    _build_unit(TERRAIN, POSITION, data, true);
}

fn _build_unit_input(TERRAIN, POSITION) {
    terrain_buy_unit_input(POSITION, TERRAIN.type, TERRAIN.owner_id, TERRAIN.get(TAG_Direction))
}

fn _build_unit(TERRAIN, POSITION, data, exhaust_unit) {
    terrain_buy_unit(POSITION, TERRAIN.owner_id, data, exhaust_unit)
}
