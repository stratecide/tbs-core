
fn uf_can_buy_hero(CONTEXT) {
    if CONTEXT.unit.hero != () || CONTEXT.unit.has(FLAG_Zombified) {
        return false;
    }
    // can't summon Hero if they're already on the board
    let hero_type = CONTEXT.config.get_hero_type(CONTEXT.unit.owner_id);
    for uwp in CONTEXT.board.all_units() {
        if uwp.unit.owner_id == CONTEXT.unit.owner_id && uwp.unit.hero?.type == hero_type {
            return false;
        }
    }
    true
}

fn ua_buy_hero_input(CONTEXT) {
    let options = new_shop("Summon Hero");
    let funds = CONTEXT.board.get(CONTEXT.unit.owner_id, TAG_Funds) ?? 0;
    let hero_type = CONTEXT.config.get_hero_type(CONTEXT.unit.owner_id);
    if !CONTEXT.config.table_entry("UNIT_HEROES", CONTEXT.unit.type, hero_type) {
        return failure();
    }
    let item = ShopItem(hero_type);
    let unit_value = unit_value(CONTEXT.unit);
    let cost = 500 + unit_value / 2;
    if hero_type == HERO_Castor {
        cost += unit_value;
    }
    item.costs.push(cost);
    if cost > funds {
        item.enabled = false;
    }
    if item.enabled {
        for uwp in CONTEXT.board.all_units() {
            if uwp.unit.owner_id == CONTEXT.unit.owner_id && uwp.unit.hero?.type == hero_type {
                item.enabled = false;
                break;
            }
        }
    }
    options.add(item);
    CONTEXT.player.choice(options, false);
    if hero_type == HERO_Castor {
        let options = new_direction_selection(CONTEXT.position);
        for n in CONTEXT.board.get_neighbors_with_direction(CONTEXT.position) {
            if CONTEXT.board.get_unit(n.point) == ()
            && CONTEXT.board.get_terrain(n.point).movement_cost(CONTEXT.unit.movement_type) != () {
                options += n.direction
            }
        }
        CONTEXT.player.choice(options, false);
    }
    success()
}

fn ua_buy_hero(CONTEXT, data) {
    CONTEXT.event_handler.set_unit_flag(CONTEXT.position, FLAG_Exhausted);
    let hero_type = data[0].key;
    let cost = data[0].costs[0];
    if hero_type == HERO_Castor {
        let direction = data[1];
        let target = CONTEXT.event_handler.board.get_neighbor(CONTEXT.position, direction);
        if CONTEXT.event_handler.board.get_unit(target) != () {
            // hidden unit blocks the build, but money isn't spent either
            // TODO: show alert! effect
            return;
        }
        let unit = Unit(CONTEXT.board.config, CONTEXT.unit.type);
        unit.owner_id = CONTEXT.unit.owner_id;
        unit.set(TAG_Hp, 100);
        unit.set(FLAG_Exhausted);
        if CONTEXT.unit.get(TAG_DroneStationId) != () {
            unit.set(TAG_DroneStationId, CONTEXT.event_handler.generate_unique_id(TAG_DroneStationId));
        }
        CONTEXT.event_handler.place_unit(target, unit);
        CONTEXT.event_handler.set_hero(target, Hero(HERO_Pollux));
    }
    CONTEXT.event_handler.set_hero(CONTEXT.position, Hero(hero_type));
    CONTEXT.event_handler.set(CONTEXT.unit.owner_id, TAG_Funds, (CONTEXT.board.get(CONTEXT.unit.owner_id, TAG_Funds) ?? 0) - cost);
}
