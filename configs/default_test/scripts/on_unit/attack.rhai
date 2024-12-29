
fn life_steal_20() {
    let hp = UNIT.get(TAG_Hp);
    if hp != () {
        EVENT_HANDLER.effect(change_unit_hp(POSITION, DAMAGE * 20 / 100));
    }
}
