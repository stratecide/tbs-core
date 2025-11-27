
fn show_explosion() {
    EVENT_HANDLER.effect(POSITION, FX_Explosion(UNIT.visibility(BOARD, POSITION)));
}

fn kraken() {
    for p in BOARD.all_positions() {
        let terrain = BOARD.get_terrain(p);
        if terrain.type != TerrainType(CONFIG, "Kraken") {
            continue;
        }
        let anger = ((terrain.get(TAG_Anger) ?? 0) + 1) % (KRAKEN_MAX_ANGER + 1);
        if anger != 0 {
            EVENT_HANDLER.set_terrain_tag(p, TAG_Anger, anger);
            continue;
        }
        // show kraken raging
        let effects = [];
        for point in BOARD.positions_in_range(p, KRAKEN_ATTACK_RANGE) {
            effects.push(FX_KrakenRage().at(point));
        }
        EVENT_HANDLER.effects(effects);
        let effects = [];
        for point in BOARD.positions_in_range(p, KRAKEN_ATTACK_RANGE) {
            let unit = BOARD.get_unit(point);
            if unit?.owner_id >= 0 && unit.has(TAG_Hp) {
                effects.push(change_unit_hp(EVENT_HANDLER, point, -KRAKEN_DAMAGE));
            }
        }
        // animate HP losses
        EVENT_HANDLER.effects(effects);
        EVENT_HANDLER.remove_terrain_tag(p, TAG_Anger);
    }
}

fn lose_game() {
    EVENT_HANDLER.make_player_lose(UNIT.owner_id)
}
