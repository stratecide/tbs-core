
fn co_recovery() {
    let heals = new_mass_heal();
    for point in BOARD.all_positions() {
        let unit = BOARD.get_unit(point);
        if unit?.owner_id == OWNER_ID {
            heals.add(point, 30);
        }
    }
    EVENT_HANDLER.heal_units(heals);
}

fn level_increases_attack(value) {
    value + Fraction(UNIT.get(TAG_Level) ?? 0, 10)
}
