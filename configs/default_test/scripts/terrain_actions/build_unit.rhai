
fn build_unit_input(CONTEXT) {
    _build_unit_input(CONTEXT)
}

fn build_unit(CONTEXT, data) {
    CONTEXT.event_handler.set_terrain_flag(CONTEXT.position, FLAG_Exhausted);
    _build_unit(CONTEXT, data, true);
}

fn and_turn_into_grass_input(CONTEXT) {
    _build_unit_input(CONTEXT)
}

fn and_turn_into_grass(CONTEXT, data) {
    CONTEXT.event_handler.replace_terrain(CONTEXT.position, Terrain(CONTEXT.config, TERRAIN_Grass));
    _build_unit(CONTEXT, data, true);
}

fn _build_unit_input(CONTEXT) {
    terrain_buy_unit_input(CONTEXT.board, CONTEXT.player, CONTEXT.terrain.type, CONTEXT.terrain.owner_id, CONTEXT.terrain.get(TAG_Direction))
}

fn _build_unit(CONTEXT, data, exhaust_unit) {
    terrain_buy_unit(CONTEXT.event_handler, CONTEXT.position, CONTEXT.terrain.owner_id, data, exhaust_unit)
}
