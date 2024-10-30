
fn heal_10() {
    EVENT_HANDLER.set_unit_tag(POSITION, TAG_Hp, UNIT.get(TAG_Hp) + 10);
}

fn earn_income() {
    let owner_id = UNIT.owner_id;
    EVENT_HANDLER.gain_money(owner_id, CONFIG.income_factor(owner_id));
}

fn damage_10() {
    EVENT_HANDLER.set_unit_tag(POSITION, TAG_Hp, UNIT.get(TAG_Hp) - 10);
}

fn damage_20() {
    EVENT_HANDLER.set_unit_tag(POSITION, TAG_Hp, UNIT.get(TAG_Hp) - 20);
}

fn clear_status_flags() {
    EVENT_HANDLER.remove_unit_flag(POSITION, FLAG_Repairing);
    EVENT_HANDLER.remove_unit_flag(POSITION, FLAG_Capturing);
    if UNIT.has(FLAG_Stunned) {
        EVENT_HANDLER.set_unit_flag(POSITION, FLAG_Exhausted);
        EVENT_HANDLER.remove_unit_flag(POSITION, FLAG_Stunned);
    }
}
