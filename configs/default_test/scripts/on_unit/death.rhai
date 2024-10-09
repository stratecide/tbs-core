
fn kraken() {
    for p in BOARD().all_positions() {
        let terrain = BOARD().get_terrain(p);
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
        for point in BOARD().positions_in_range(p, KRAKEN_ATTACK_RANGE) {
            let unit = BOARD().get_unit(point);
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
    // TODO: set owner to neutral and revive with 50HP
}

fn revive_converted() {
    // TODO: revive with 50HP, owned by the killer
}

fn sludge_1() {
    // TODO
}
