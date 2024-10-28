
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
    _build_unit(TERRAIN, POSITION, data, false);
}

fn _build_unit_input(TERRAIN, POSITION) {
    let options = new_shop("Build Unit");
    for unit_type in CONFIG.table_row("TERRAIN_BUILD", TERRAIN.type, true) {
        let unit = Unit(unit_type);
        unit.owner_id = TERRAIN.owner_id;
        unit.set(TAG_Hp, 100);
        let price = unit.value(POSITION);
        options.add(unit, price);
    }
    user_selection(options, false);
    success()
}

fn _build_unit(TERRAIN, POSITION, data, exhaust_unit) {
    let unit = data[0].key;
    let cost = data[0].costs[0];
    if exhaust_unit {
        unit.set(FLAG_Exhausted);
    }
    if TERRAIN.has(TAG_Direction) {
        unit.set(TAG_Direction, TERRAIN.get(TAG_Direction));
    }
    EVENT_HANDLER.spend_money(TERRAIN.owner_id, cost);
    EVENT_HANDLER.place_unit(POSITION, unit);
}
