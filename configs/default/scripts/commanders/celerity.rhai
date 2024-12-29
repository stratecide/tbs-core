
fn co_recovery() {
    let effects = [];
    for point in BOARD.all_positions() {
        let unit = BOARD.get_unit(point);
        if unit?.owner_id == OWNER_ID && unit.has(TAG_Hp) {
            effects.push(change_unit_hp(point, 30));
        }
    }
    EVENT_HANDLER.effects(effects);
}

fn level_increases_attack(value) {
    value + Fraction(UNIT.get(TAG_Level) ?? 0, 10)
}
