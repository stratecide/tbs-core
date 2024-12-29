
fn show_explosion() {
    EVENT_HANDLER.effect(POSITION, FX_Explosion(UNIT.visibility(POSITION)));
}

fn kraken() {
    for p in BOARD.all_positions() {
        let terrain = BOARD.get_terrain(p);
        if terrain.type != TerrainType("Kraken") {
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
                effects.push(change_unit_hp(point, -KRAKEN_DAMAGE));
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

fn revive_neutral() {
    let new_unit = Unit(UNIT.type);
    new_unit.copy_from(UNIT);
    new_unit.owner_id = -1;
    new_unit.set(TAG_Hp, 50);
    // TODO: show effect?
    EVENT_HANDLER.place_unit(POSITION, new_unit);
}

fn revive_converted() {
    let new_unit = Unit(UNIT.type);
    new_unit.copy_from(UNIT);
    new_unit.owner_id = OTHER_UNIT?.owner_id ?? -1;
    new_unit.set(TAG_Hp, 50);
    // TODO: show effect?
    EVENT_HANDLER.place_unit(POSITION, new_unit);
}

fn sludge_1() {
    let counter = 0;
    let token = Token("Sludge");
    let old_token = for t in BOARD.get_tokens(POSITION) {
        if t.type == token.type {
            break t;
        }
    };
    if old_token != () {
        if old_token.get(TAG_SludgeCounter) > counter {
            return;
        } else {
            EVENT_HANDLER.remove_token(POSITION, old_token);
        }
    }
    token.owner_id = UNIT.owner_id;
    token.set(TAG_SludgeCounter, counter);
    EVENT_HANDLER.place_token(POSITION, token);
}
