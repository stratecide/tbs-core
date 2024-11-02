const KRAKEN_MAX_ANGER = 8;
const KRAKEN_ATTACK_RANGE = 3;
const KRAKEN_DAMAGE = 40;

fn unit_can_build(UNIT, POSITION) {
    let capacity = UNIT.transport_capacity;
    if capacity > 0 {
        // build inside
        if UNIT.get(TAG_DroneStationId) != () {
            for p in BOARD.all_positions() {
                let drone = BOARD.get_unit(p);
                if drone != () && drone.get(TAG_DroneId) == UNIT.get(TAG_DroneStationId) {
                    capacity -= 1;
                }
            }
        }
        UNIT.transported_len < capacity
    } else {
        // build outside
        for point in BOARD.get_neighbors(POSITION) {
            if BOARD.get_unit(point) == () {
                return true;
            }
        }
        false
    }
}

fn build_unit_input(UNIT, POSITION) {
    let options = new_shop("Build Unit");
    for unit_type in CONFIG.table_row("UNIT_BUILD", UNIT.type, true) {
        let unit = Unit(unit_type);
        unit.owner_id = UNIT.owner_id;
        unit.set(TAG_Hp, 100);
        if UNIT.get(TAG_DroneStationId) != () {
            unit.set(TAG_DroneId, UNIT.get(TAG_DroneStationId));
        }
        let price = unit.value(POSITION);
        options.add(unit, price);
    }
    let unit = user_selection(options, false).key;
    if UNIT.transport_capacity == 0 {
        // need to find a place outside
        let options = new_direction_selection(POSITION);
        for n in BOARD.get_neighbors_with_direction(POSITION) {
            if BOARD.get_unit(n.point) == ()
            && BOARD.get_terrain(n.point).movement_cost(unit.movement_type) != () {
                options += n.direction
            }
        }
        user_selection(options, false);
    }
    success()
}

fn build_unit(UNIT, POSITION, exhaust, data) {
    EVENT_HANDLER.set_unit_flag(POSITION, FLAG_Exhausted);
    let unit = data[0].key;
    let cost = data[0].costs[0];
    if exhaust {
        unit.set(FLAG_Exhausted);
    }
    if unit.type == UNIT_LightDrone && !UNIT.has(TAG_DroneStationId) {
        let uid = EVENT_HANDLER.generate_unique_id(TAG_DroneStationId);
        EVENT_HANDLER.set_unit_tag(POSITION, TAG_DroneStationId, uid);
        unit.set(TAG_DroneId, uid);
    }
    if UNIT.transport_capacity == 0 {
        let direction = data[1];
        let target = BOARD.get_neighbor(POSITION, direction);
        if BOARD.get_unit(target) != () {
            // hidden unit blocks the build, but money isn't spent either
            // TODO: show alert! effect
            return;
        }
        // TODO: animate path from "POSITION" to "target"
        EVENT_HANDLER.place_unit(target, unit);
    } else {
        EVENT_HANDLER.transport_unit(POSITION, unit);
    }
    EVENT_HANDLER.spend_money(UNIT.owner_id, cost);
}

fn terrain_buy_unit_input(position, terrain_type, owner_id, direction) {
    let options = new_shop("Build Unit");
    for unit_type in CONFIG.table_row("TERRAIN_BUILD", terrain_type, true) {
        let unit = Unit(unit_type);
        unit.owner_id = owner_id;
        unit.set(TAG_Hp, 100);
        if direction != () {
            unit.set(TAG_Direction, direction);
        }
        if unit.movement_types.len > 1 && terrain_type == TERRAIN_Port {
            unit.movement_type = MOVEMENT_Amphibious;
        }
        let price = unit.value(position);
        options.add(unit, price);
    }
    user_selection(options, false);
    success()
}

fn terrain_buy_unit(position, owner_id, data, exhaust_unit) {
    let unit = data[0].key;
    let cost = data[0].costs[0];
    if exhaust_unit {
        unit.set(FLAG_Exhausted);
    }
    EVENT_HANDLER.spend_money(owner_id, cost);
    EVENT_HANDLER.place_unit(position, unit);
}
