
fn heal_10() {
    EVENT_HANDLER.effect(change_unit_hp(POSITION, 10));
}

fn earn_income() {
    let owner_id = UNIT.owner_id;
    EVENT_HANDLER.gain_money(owner_id, CONFIG.income_factor(owner_id));
}

fn damage_10() {
    EVENT_HANDLER.effect(change_unit_hp(POSITION, -10));
}

fn damage_20() {
    EVENT_HANDLER.effect(change_unit_hp(POSITION, -20));
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
