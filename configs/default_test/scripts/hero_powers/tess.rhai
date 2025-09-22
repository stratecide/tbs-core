
fn ua_power_input() {
    build_unit_input(BOARD, PLAYER, UNIT, POSITION)
}

fn ua_power(data) {
    exhaust_all_on_chess_board(EVENT_HANDLER, PATH.start, UNIT.owner_id);
    EVENT_HANDLER.set_unit_flag(POSITION, FLAG_Exhausted);
    build_unit(EVENT_HANDLER, UNIT, POSITION, false, data);
}
