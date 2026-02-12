
fn show_explosion(CONTEXT) {
    CONTEXT.event_handler.effect(CONTEXT.position, FX_Explosion(CONTEXT.unit.visibility(CONTEXT.board, CONTEXT.position)));
}

fn kraken(CONTEXT) {
    for p in CONTEXT.board.all_positions() {
        let terrain = CONTEXT.board.get_terrain(p);
        if terrain.type != TerrainType(CONTEXT.config, "Kraken") {
            continue;
        }
        let anger = ((terrain.get(TAG_Anger) ?? 0) + 1) % (KRAKEN_MAX_ANGER + 1);
        if anger != 0 {
            CONTEXT.event_handler.set_terrain_tag(p, TAG_Anger, anger);
            continue;
        }
        // show kraken raging
        let effects = [];
        for point in CONTEXT.board.positions_in_range(p, KRAKEN_ATTACK_RANGE) {
            effects.push(FX_KrakenRage().at(point));
        }
        CONTEXT.event_handler.effects(effects);
        let effects = [];
        for point in CONTEXT.board.positions_in_range(p, KRAKEN_ATTACK_RANGE) {
            let unit = CONTEXT.board.get_unit(point);
            if unit?.owner_id >= 0 && unit.has(TAG_Hp) {
                effects.push(change_unit_hp(CONTEXT.event_handler, point, -KRAKEN_DAMAGE));
            }
        }
        // animate HP losses
        CONTEXT.event_handler.effects(effects);
        CONTEXT.event_handler.remove_terrain_tag(p, TAG_Anger);
    }
}

fn lose_game(CONTEXT) {
    CONTEXT.event_handler.make_player_lose(CONTEXT.unit.owner_id)
}
