const KRAKEN_MAX_ANGER = 8;
const KRAKEN_ATTACK_RANGE = 3;
const KRAKEN_DAMAGE = 40;

fn uf_can_build() {
    if UNIT.transported != () {
        // build inside
        let capacity = UNIT.transport_capacity;
        if UNIT.drone_station_id != () {
            for p in BOARD().all_positions() {
                let drone = BOARD().get_unit(p);
                if drone != () && drone.drone_id == UNIT.drone_station_id {
                    capacity -= 1;
                }
            }
        }
        UNIT.transported_len < capacity
    } else {
        // build outside
        for point in BOARD().get_neighbors(POSITION) {
            if BOARD().get_unit(point) == () {
                return true;
            }
        }
        false
    }
}

fn build_unit_input(UNIT, POSITION) {
    let options = new_unit_shop();
    for unit_type in CONFIG().table_row("UNIT_BUILD", UNIT.type, true) {
        let unit = CONFIG().build_unit(unit_type)
            .owner_id(UNIT.owner_id)
            .build();
        let price = unit.full_price(POSITION);
        options.add(unit, price);
    }
    let unit = user_selection(options, false);
    if UNIT.transported == () {
        // need to find a place outside
        let options = new_direction_selection(POSITION);
        for n in BOARD().get_neighbors_with_direction(POSITION) {
            if BOARD().get_unit(n.point) == ()
            && BOARD().get_terrain(n.point).movement_cost(unit.movement_type) != () {
                options += n.direction
            }
        }
        user_selection(options, false);
    }
    success()
}

fn build_unit(EVENT_HANDLER, UNIT, POSITION, exhaust, data) {
    let unit = data[0];
    if exhaust {
        unit.status = status_exhausted();
    }
    if UNIT.transported == () {
        let direction = data[1];
        let target = BOARD().get_neighbor(POSITION, direction);
        if BOARD().get_unit(target) != () {
            // hidden unit blocks the build, but money isn't spent either
            // TODO: show alert! effect
            return;
        }
        // TODO: animate path from "POSITION" to "target"
        EVENT_HANDLER.place_unit(target, unit);
    } else {
        EVENT_HANDLER.transport_unit(POSITION, unit);
    }
    let cost = unit.full_price(POSITION);
    EVENT_HANDLER.spend_money(UNIT.owner_id, cost);
}
