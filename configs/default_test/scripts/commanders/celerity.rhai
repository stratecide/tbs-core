
fn co_recovery(CONTEXT) {
    let effects = [];
    for uwp in CONTEXT.board.all_units() {
        if uwp.unit.owner_id == CONTEXT.owner_id && uwp.unit.has(TAG_Hp) {
            effects.push(change_unit_hp(CONTEXT.event_handler, uwp.position, 30));
        }
    }
    CONTEXT.event_handler.effects(effects);
}

fn level_increases_attack(CONTEXT, value) {
    value + Fraction(CONTEXT.unit.get(TAG_Level) ?? 0, 10)
}
