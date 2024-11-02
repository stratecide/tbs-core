
fn life_steal_20() {
    let hp = UNIT.get(TAG_Hp);
    if hp != () {
        EVENT_HANDLER.set_unit_tag(POSITION, TAG_Hp, hp + DAMAGE * 20 / 100);
    }
}
