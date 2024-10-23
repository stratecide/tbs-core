
fn unit_death_test(unit, position) {
    (unit.get_tag(Hp) ?? 100) <= 0
}

fn unit_movable_test() {
    !UNIT.has_flag(Exhausted)
}

fn deal_damage_to_unit() {
    let hp = UNIT.get_tag(Hp);
    //if hp != () {
        EVENT_HANDLER.set_unit_tag(POSITION, Hp, hp - DAMAGE);
    //}
}
