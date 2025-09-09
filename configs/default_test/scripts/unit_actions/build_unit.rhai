
fn uf_can_build() {
    unit_can_build(BOARD, UNIT, POSITION)
}

fn ua_exhausted_input() {
    build_unit_input(BOARD, PLAYER, UNIT, POSITION)
}

fn ua_exhausted(data) {
    build_unit(EVENT_HANDLER, UNIT, POSITION, true, data)
}
