
fn ua_power_input() {
    build_unit_input(UNIT, POSITION)
}

fn ua_power(data) {
    build_unit(UNIT, POSITION, false, data);
    EVENT_HANDLER.set_unit_flag(POSITION, FLAG_Exhausted);
}
