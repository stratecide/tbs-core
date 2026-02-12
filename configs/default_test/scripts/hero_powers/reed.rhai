
fn ua_power(CONTEXT) {
    exhaust_all_on_chess_board(CONTEXT.event_handler, CONTEXT.path.start, CONTEXT.unit.owner_id);
    CONTEXT.event_handler.set_unit_flag(CONTEXT.position, FLAG_Exhausted);
    for position in CONTEXT.board.get_neighbors(CONTEXT.position) {
        if position == CONTEXT.position {
            continue;
        }
        let unit = CONTEXT.board.get_unit(position);
        if unit?.owner_id == CONTEXT.unit.owner_id && unit.has(FLAG_Exhausted) {
            CONTEXT.event_handler.remove_unit_flag(position, FLAG_Exhausted);
        }
    }
}
