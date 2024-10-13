
fn leave_skull() {
    EVENT_HANDLER.place_skull(OTHER_POSITION, OTHER_UNIT, UNIT.owner_id);
}

fn resurrect() {
    let unit = CONFIG.build_unit(OTHER_UNIT.type)
        .owner_id(UNIT.owner_id)
        .hp(50)
        .zombified()
        .build();
    EVENT_HANDLER.place_unit(OTHER_POSITION, unit);
}

fn level_up() {
    if UNIT.level != () {
        EVENT_HANDLER.set_unit_level(POSITION, UNIT.level + 1);
    }
}
