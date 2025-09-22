
fn uf_can_buy_hero() {
    if UNIT.hero != () || UNIT.has(FLAG_Zombified) {
        return false;
    }
    // can't summon Hero if they're already on the board
    let hero_type = CONFIG.get_hero_type(UNIT.owner_id);
    for uwp in BOARD.all_units() {
        if uwp.unit.owner_id == UNIT.owner_id && uwp.unit.hero?.type == hero_type {
            return false;
        }
    }
    true
}

fn ua_buy_hero_input() {
    let options = new_shop("Summon Hero");
    let funds = BOARD.get(UNIT.owner_id, TAG_Funds) ?? 0;
    let hero_type = CONFIG.get_hero_type(UNIT.owner_id);
    if !CONFIG.table_entry("UNIT_HEROES", UNIT.type, hero_type) {
        return failure();
    }
    let item = ShopItem(hero_type);
    let unit_value = unit_value(UNIT);
    let cost = 500 + unit_value / 2;
    if hero_type == HERO_Castor {
        cost += unit_value;
    }
    item.costs.push(cost);
    if cost > funds {
        item.enabled = false;
    }
    if item.enabled {
        for uwp in BOARD.all_units() {
            if uwp.unit.owner_id == UNIT.owner_id && uwp.unit.hero?.type == hero_type {
                item.enabled = false;
                break;
            }
        }
    }
    options.add(item);
    PLAYER.choice(options, false);
    if hero_type == HERO_Castor {
        let options = new_direction_selection(POSITION);
        for n in BOARD.get_neighbors_with_direction(POSITION) {
            if BOARD.get_unit(n.point) == ()
            && BOARD.get_terrain(n.point).movement_cost(UNIT.movement_type) != () {
                options += n.direction
            }
        }
        PLAYER.choice(options, false);
    }
    success()
}

fn ua_buy_hero(data) {
    EVENT_HANDLER.set_unit_flag(POSITION, FLAG_Exhausted);
    let hero_type = data[0].key;
    let cost = data[0].costs[0];
    if hero_type == HERO_Castor {
        let direction = data[1];
        let target = EVENT_HANDLER.board.get_neighbor(POSITION, direction);
        if EVENT_HANDLER.board.get_unit(target) != () {
            // hidden unit blocks the build, but money isn't spent either
            // TODO: show alert! effect
            return;
        }
        let unit = Unit(BOARD.config, UNIT.type);
        unit.owner_id = UNIT.owner_id;
        unit.set(TAG_Hp, 100);
        unit.set(FLAG_Exhausted);
        if UNIT.get(TAG_DroneStationId) != () {
            unit.set(TAG_DroneStationId, EVENT_HANDLER.generate_unique_id(TAG_DroneStationId));
        }
        EVENT_HANDLER.place_unit(target, unit);
    }
    EVENT_HANDLER.set_hero(POSITION, Hero(hero_type));
    EVENT_HANDLER.set(UNIT.owner_id, TAG_Funds, (BOARD.get(UNIT.owner_id, TAG_Funds) ?? 0) - cost);
}
