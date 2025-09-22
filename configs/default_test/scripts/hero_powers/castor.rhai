
fn ua_power() {
    for position in BOARD.get_neighbors(POSITION) {
        if position == POSITION {
            continue;
        }
        let unit = BOARD.get_unit(position);
        if unit?.owner_id == UNIT.owner_id {
            EVENT_HANDLER.effect(change_unit_hp(EVENT_HANDLER, position, 30));
        }
    }
    exhaust_all_on_chess_board(EVENT_HANDLER, PATH.start, UNIT.owner_id);
    EVENT_HANDLER.set_unit_flag(POSITION, FLAG_Exhausted);
}
