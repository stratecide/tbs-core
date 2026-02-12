
fn unit_death_test(CONTEXT) {
    (CONTEXT.unit.get(TAG_Hp) ?? 100) <= 0
}

fn unit_movable_test(CONTEXT) {
    !CONTEXT.unit.has(FLAG_Exhausted) && !CONTEXT.unit.has(FLAG_Stunned)
}

fn attack_target_rook(CONTEXT) {
    CONTEXT.unit.owner_id == CONTEXT.other_unit.owner_id && CONTEXT.unit.has(FLAG_Unmoved) && CONTEXT.other_unit.has(FLAG_Unmoved)
}
