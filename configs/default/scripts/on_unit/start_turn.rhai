
fn heal_10() {
    EVENT_HANDLER.heal_unit(POSITION, 10);
}

fn earn_income() {
    let owner_id = UNIT.owner_id;
    EVENT_HANDLER.gain_money(owner_id, CONFIG.income_factor(owner_id));
}

fn damage_10() {
    EVENT_HANDLER.damage_unit(POSITION, 10);
}

fn damage_20() {
    EVENT_HANDLER.damage_unit(POSITION, 20);
}
