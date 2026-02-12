
fn ua_power(CONTEXT) {
    for position in CONTEXT.board.get_neighbors(CONTEXT.position) {
        if position == CONTEXT.position {
            continue;
        }
        let unit = CONTEXT.board.get_unit(position);
        if unit != () && unit.team != CONTEXT.unit.team && unit.owner_id >= 0 {
            CONTEXT.event_handler.effect(change_unit_hp(CONTEXT.event_handler, position, -30));
        }
    }
    exhaust_all_on_chess_board(CONTEXT.event_handler, CONTEXT.path.start, CONTEXT.unit.owner_id);
    CONTEXT.event_handler.set_unit_flag(CONTEXT.position, FLAG_Exhausted);
}
