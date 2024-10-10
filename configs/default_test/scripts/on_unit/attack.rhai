
fn life_steal_20() {
    let amount = DAMAGE * 20 / 100;
    EVENT_HANDLER.heal_unit(POSITION, amount);
}
