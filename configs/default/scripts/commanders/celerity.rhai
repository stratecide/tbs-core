
fn co_recovery() {
    for point in BOARD.all_positions() {
        let unit = BOARD.get_unit(point);
        if unit?.owner_id == OWNER_ID && unit.has(TAG_Hp) {
            EVENT_HANDLER.set_unit_tag(point, TAG_Hp, (unit.get(TAG_Hp) ?? 0) + 30);
        }
    }
}

fn level_increases_attack(value) {
    value + Fraction(UNIT.get(TAG_Level) ?? 0, 10)
}
