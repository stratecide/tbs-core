
fn leave_skull() {
    // TODO: create skull of the killed unit
}

fn resurrect() {
    // TODO: resurrect the killed unit with 50 HP, belonging to the killer's owner
}

fn level_up() {
    if UNIT.level != () {
        EVENT_HANDLER.set_unit_level(POSITION, UNIT.level + 1);
    }
}
