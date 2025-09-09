
fn co_recovery() {
    let effects = [];
    for uwp in BOARD.all_units() {
        if uwp.unit.owner_id == OWNER_ID && uwp.unit.has(TAG_Hp) {
            effects.push(change_unit_hp(EVENT_HANDLER, uwp.position, 30));
        }
    }
    EVENT_HANDLER.effects(effects);
}

fn level_increases_attack(value) {
    value + Fraction(UNIT.get(TAG_Level) ?? 0, 10)
}
