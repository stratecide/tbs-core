
fn unit_death_test(unit, position) {
    (unit.get(TAG_Hp) ?? 100) <= 0
}

fn unit_movable_test() {
    !UNIT.has(FLAG_Exhausted) && !UNIT.has(FLAG_Stunned)
}

fn deal_damage_to_unit() {
    let hp = UNIT.get(TAG_Hp);
    if hp != () {
        EVENT_HANDLER.set_unit_tag(POSITION, TAG_Hp, hp - DAMAGE);
    }
}
