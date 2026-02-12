
fn heal_10(CONTEXT) {
    CONTEXT.event_handler.effect(change_unit_hp(CONTEXT.event_handler, CONTEXT.position, 10));
}

fn earn_income(CONTEXT) {
    let owner_id = CONTEXT.unit.owner_id;
    let income = CONTEXT.board.get(owner_id, TAG_Income) ?? 0;
    let funds = CONTEXT.board.get(owner_id, TAG_Funds) ?? 0;
    CONTEXT.event_handler.set(owner_id, TAG_Funds, funds + income);
}

fn damage_10(CONTEXT) {
    CONTEXT.event_handler.effect(change_unit_hp(CONTEXT.event_handler, CONTEXT.position, -10));
}

fn damage_20(CONTEXT) {
    CONTEXT.event_handler.effect(change_unit_hp(CONTEXT.event_handler, CONTEXT.position, -20));
}

fn damage_50(CONTEXT) {
    CONTEXT.event_handler.effect(change_unit_hp(CONTEXT.event_handler, CONTEXT.position, -50));
}

fn clear_status_flags(CONTEXT) {
    CONTEXT.event_handler.remove(CONTEXT.unit_id, FLAG_Repairing);
    CONTEXT.event_handler.remove(CONTEXT.unit_id, FLAG_Capturing);
    if CONTEXT.unit.has(FLAG_Stunned) {
        CONTEXT.event_handler.set(CONTEXT.unit_id, FLAG_Exhausted);
        CONTEXT.event_handler.remove(CONTEXT.unit_id, FLAG_Stunned);
    }
    CONTEXT.event_handler.remove(CONTEXT.unit_id, TAG_EnPassant);
}

fn return_drones(CONTEXT) {
    let capacity = CONTEXT.event_handler.remaining_transport_capacity(CONTEXT.unit_id);
    let id = CONTEXT.unit.get(TAG_DroneStationId);
    for p in CONTEXT.board.all_positions() {
        let drone = CONTEXT.board.get_unit(p);
        if drone != () && drone.get(TAG_DroneId) == id {
            if capacity == 0 {
                CONTEXT.event_handler.effect(p, FX_Explosion(drone.visibility(CONTEXT.board, p)));
            }
            CONTEXT.event_handler.take_unit(p);
            if capacity > 0 {
                capacity -= 1;
                CONTEXT.event_handler.transport_unit(CONTEXT.position, drone);
            }
        }
    }
}

fn self_destruct(CONTEXT) {
    CONTEXT.event_handler.effect(change_unit_hp(CONTEXT.event_handler, CONTEXT.position, -100));
}
