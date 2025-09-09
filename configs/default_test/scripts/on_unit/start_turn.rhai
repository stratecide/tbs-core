
fn heal_10() {
    EVENT_HANDLER.effect(change_unit_hp(EVENT_HANDLER, POSITION, 10));
}

fn earn_income() {
    let owner_id = UNIT.owner_id;
    let income = BOARD.get(owner_id, TAG_Income) ?? 0;
    let funds = BOARD.get(owner_id, TAG_Funds) ?? 0;
    EVENT_HANDLER.set(owner_id, TAG_Funds, funds + income);
}

fn damage_10() {
    EVENT_HANDLER.effect(change_unit_hp(EVENT_HANDLER, POSITION, -10));
}

fn damage_20() {
    EVENT_HANDLER.effect(change_unit_hp(EVENT_HANDLER, POSITION, -20));
}

fn damage_50() {
    EVENT_HANDLER.effect(change_unit_hp(EVENT_HANDLER, POSITION, -50));
}

fn clear_status_flags() {
    EVENT_HANDLER.remove(UNIT_ID, FLAG_Repairing);
    EVENT_HANDLER.remove(UNIT_ID, FLAG_Capturing);
    if UNIT.has(FLAG_Stunned) {
        EVENT_HANDLER.set(UNIT_ID, FLAG_Exhausted);
        EVENT_HANDLER.remove(UNIT_ID, FLAG_Stunned);
    }
    EVENT_HANDLER.remove(UNIT_ID, TAG_EnPassant);
}

fn return_drones() {
    let capacity = EVENT_HANDLER.remaining_transport_capacity(UNIT_ID);
    let id = UNIT.get(TAG_DroneStationId);
    for p in BOARD.all_positions() {
        let drone = BOARD.get_unit(p);
        if drone != () && drone.get(TAG_DroneId) == id {
            if capacity == 0 {
                EVENT_HANDLER.effect(p, FX_Explosion(drone.visibility(BOARD, p)));
            }
            EVENT_HANDLER.take_unit(p);
            if capacity > 0 {
                capacity -= 1;
                EVENT_HANDLER.transport_unit(POSITION, drone);
            }
        }
    }
}

fn self_destruct() {
    EVENT_HANDLER.effect(change_unit_hp(EVENT_HANDLER, POSITION, -100));
}
