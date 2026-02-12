
fn ua_power(CONTEXT) {
    for position in CONTEXT.board.positions_in_range(CONTEXT.position, 5) {
        let unit = CONTEXT.board.get_unit(position);
        if unit?.owner_id == CONTEXT.unit.owner_id {
            CONTEXT.event_handler.effect(change_unit_hp(CONTEXT.event_handler, position, 20));
        }
    }
    exhaust_all_on_chess_board(CONTEXT.event_handler, CONTEXT.path.start, CONTEXT.unit.owner_id);
    CONTEXT.event_handler.set_unit_flag(CONTEXT.position, FLAG_Exhausted);
}
