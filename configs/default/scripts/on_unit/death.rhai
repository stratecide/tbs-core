
fn kraken() {
    for p in BOARD.all_positions() {
        let terrain = BOARD.get_terrain(p);
        if terrain.anger == () {
            continue;
        }
        let anger = (terrain.anger + 1) % (KRAKEN_MAX_ANGER + 1);
        if anger != 0 {
            EVENT_HANDLER.set_terrain_anger(p, anger);
            continue;
        }
        // TODO: show effect
        let damages = new_mass_damage();
        for point in BOARD.positions_in_range(p, KRAKEN_ATTACK_RANGE) {
            let unit = BOARD.get_unit(point);
            if unit?.owner_id >= 0 {
                damages.add(point, KRAKEN_DAMAGE);
            }
        }
        EVENT_HANDLER.damage_units(damages);
        EVENT_HANDLER.set_terrain_anger(p, anger);
    }
}

fn lose_game() {
    EVENT_HANDLER.make_player_lose(UNIT.owner_id)
}

fn revive_neutral() {
    let new_unit = Unit(UNIT.type);
    new_unit.copy_from(UNIT);
    new_unit.owner_id(-1);
    new_unit.hp(50);
    // TODO: show effect?
    EVENT_HANDLER.place_unit(POSITION, new_unit);
}

fn revive_converted() {
    let new_unit = Unit(UNIT.type);
    new_unit.copy_from(UNIT);
    new_unit.owner_id(OTHER_UNIT?.owner_id ?? -1);
    new_unit.hp(50);
    // TODO: show effect?
    EVENT_HANDLER.place_unit(POSITION, new_unit);
}

fn sludge_1() {
    EVENT_HANDLER.place_sludge(POSITION, UNIT.owner_id, 0);
}
