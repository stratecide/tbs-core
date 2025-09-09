
fn build_unit_input() {
    _build_unit_input(BOARD, PLAYER, TERRAIN, POSITION)
}

fn build_unit(data) {
    EVENT_HANDLER.set_terrain_flag(POSITION, FLAG_Exhausted);
    _build_unit(EVENT_HANDLER, TERRAIN, POSITION, data, true);
}

fn and_turn_into_grass_input() {
    _build_unit_input(BOARD, PLAYER, TERRAIN, POSITION)
}

fn and_turn_into_grass(data) {
    EVENT_HANDLER.replace_terrain(POSITION, Terrain(CONFIG, TERRAIN_Grass));
    _build_unit(EVENT_HANDLER, TERRAIN, POSITION, data, true);
}

fn _build_unit_input(BOARD, PLAYER, TERRAIN, POSITION) {
    terrain_buy_unit_input(BOARD, PLAYER, POSITION, TERRAIN.type, TERRAIN.owner_id, TERRAIN.get(TAG_Direction))
}

fn _build_unit(EVENT_HANDLER, TERRAIN, POSITION, data, exhaust_unit) {
    terrain_buy_unit(EVENT_HANDLER, POSITION, TERRAIN.owner_id, data, exhaust_unit)
}
