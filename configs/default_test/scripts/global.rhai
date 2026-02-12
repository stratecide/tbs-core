const KRAKEN_MAX_ANGER = 8;
const KRAKEN_ATTACK_RANGE = 3;
const KRAKEN_DAMAGE = 40;

fn unit_value(unit, hp) {
    let result = Fraction(parse_int(unit.config.get_custom_value(unit.type, "Value")) ?? 0);
    if unit.commander_type == COMMANDER_Celerity {
        result *= Fraction(90, 100);
    }
    if hp != () {
        result *= Fraction(hp, 100);
    }
    result.round()
}
fn unit_value(unit) {
    unit_value(unit, ())
}

fn unit_can_build(board, unit, position) {
    let capacity = unit.transport_capacity;
    if capacity > 0 {
        // build inside
        if unit.get(TAG_DroneStationId) != () {
            for p in board.all_positions() {
                let drone = board.get_unit(p);
                if drone != () && drone.get(TAG_DroneId) == unit.get(TAG_DroneStationId) {
                    capacity -= 1;
                }
            }
        }
        unit.transported_len < capacity
    } else {
        // build outside
        for point in board.get_neighbors(position) {
            if board.get_unit(point) == () {
                return true;
            }
        }
        false
    }
}

fn build_unit_input(board, player, parent_unit, position) {
    let options = new_shop("Build Unit");
    let funds = board.get(parent_unit.owner_id, TAG_Funds) ?? 0;
    for unit_type in board.config.table_row("UNIT_BUILD", parent_unit.type, true) {
        let new_unit = Unit(board.config, unit_type);
        new_unit.owner_id = parent_unit.owner_id;
        new_unit.set(TAG_Hp, 100);
        if parent_unit.get(TAG_DroneStationId) != () {
            new_unit.set(TAG_DroneId, parent_unit.get(TAG_DroneStationId));
        }
        let price = unit_value(new_unit);
        let item = ShopItem(new_unit);
        item.costs.push(price);
        if price > funds {
            item.enabled = false;
        }
        options.add(item);
    }
    let new_unit = player.choice(options, false).key;
    if parent_unit.transport_capacity == 0 {
        // need to find a place outside
        let options = new_direction_selection(position);
        for n in board.get_neighbors_with_direction(position) {
            if board.get_unit(n.point) == ()
            && board.get_terrain(n.point).movement_cost(new_unit.movement_type) != () {
                options += n.direction
            }
        }
        player.choice(options, false);
    }
    success()
}

fn build_unit(event_handler, parent_unit, position, exhaust, data) {
    event_handler.set_unit_flag(position, FLAG_Exhausted);
    let new_unit = data[0].key;
    let cost = data[0].costs[0];
    if exhaust {
        new_unit.set(FLAG_Exhausted);
    }
    if new_unit.type == UNIT_LightDrone && !parent_unit.has(TAG_DroneStationId) {
        let uid = event_handler.generate_unique_id(TAG_DroneStationId);
        event_handler.set_unit_tag(position, TAG_DroneStationId, uid);
        new_unit.set(TAG_DroneId, uid);
    }
    if parent_unit.transport_capacity == 0 {
        let direction = data[1];
        let target = event_handler.board.get_neighbor(position, direction);
        if event_handler.board.get_unit(target) != () {
            // hidden unit blocks the build, but money isn't spent either
            // TODO: show alert! effect
            return;
        }
        // TODO: animate path from "position" to "target"
        event_handler.place_unit(target, new_unit);
    } else {
        event_handler.transport_unit(position, new_unit);
    }
    event_handler.set(parent_unit.owner_id, TAG_Funds, (event_handler.board.get(parent_unit.owner_id, TAG_Funds) ?? 0) - cost);
}

fn terrain_buy_unit_input(board, player, terrain_type, owner_id, direction) {
    let options = new_shop("Build Unit");
    let funds = board.get(owner_id, TAG_Funds) ?? 0;
    for unit_type in board.config.table_row("TERRAIN_BUILD", terrain_type, true) {
        let unit = Unit(board.config, unit_type);
        unit.owner_id = owner_id;
        unit.set(TAG_Hp, 100);
        if direction != () {
            unit.set(TAG_Direction, direction);
        }
        if unit.movement_types.len > 1 && terrain_type == TERRAIN_Port {
            unit.movement_type = MOVEMENT_Amphibious;
        }
        let price = unit_value(unit);
        let item = ShopItem(unit);
        item.costs.push(price);
        if price > funds {
            item.enabled = false;
        }
        options.add(item);
    }
    player.choice(options, false);
    success()
}

fn terrain_buy_unit(event_handler, position, owner_id, data, exhaust_unit) {
    let unit = data[0].key;
    let cost = data[0].costs[0];
    if exhaust_unit {
        unit.set(FLAG_Exhausted);
    }
    event_handler.set(owner_id, TAG_Funds, (event_handler.board.get(owner_id, TAG_Funds) ?? 0) - cost);
    event_handler.place_unit(position, unit);
}

fn exhaust_all_on_chess_board(event_handler, start_position, owner_id) {
    let positions = event_handler.board.spread_search(start_position, |board, p| {
        let terrain = board.get_terrain(p);
        terrain.type == TERRAIN_ChessTile || terrain.type == TERRAIN_ChessPawnTile
    });
    for p in positions {
        if event_handler.board.get_unit(p)?.owner_id == owner_id {
            event_handler.set_unit_flag(p, FLAG_Exhausted);
        }
    }
}

fn change_unit_hp(event_handler, position, change) {
    let unit = event_handler.board.get_unit(position);
    event_handler.set_unit_tag(position, TAG_Hp, unit.get(TAG_Hp) + change);
    FX_HpChange(change).at(position)
}

fn finish_capturing(event_handler, position, owner_id) {
    let old_terrain = event_handler.board.get_terrain(position);
    let terrain = Terrain(event_handler.config, old_terrain.type);
    terrain.copy_from(old_terrain);
    terrain.remove(TAG_CaptureOwner);
    terrain.remove(TAG_CaptureProgress);
    terrain.owner_id = owner_id;
    event_handler.replace_terrain(position, terrain);
    if terrain.type == TERRAIN_Hq {
        event_handler.make_player_lose(old_terrain.owner_id);
    }
}
