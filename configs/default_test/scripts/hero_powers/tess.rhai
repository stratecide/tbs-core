
fn ua_power_input(CONTEXT) {
    build_unit_input(CONTEXT.board, CONTEXT.player, CONTEXT.unit, CONTEXT.position)
}

fn ua_power(CONTEXT, data) {
    exhaust_all_on_chess_board(CONTEXT.event_handler, CONTEXT.path.start, CONTEXT.unit.owner_id);
    CONTEXT.event_handler.set_unit_flag(CONTEXT.position, FLAG_Exhausted);
    build_unit(CONTEXT.event_handler, CONTEXT.unit, CONTEXT.position, false, data);
}
