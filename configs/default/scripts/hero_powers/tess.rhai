
fn ua_power_input() {
    build_unit_input(UNIT, POSITION)
}

fn ua_power(data) {
    build_unit(EVENT_HANDLER, UNIT, POSITION, false, data)
}
