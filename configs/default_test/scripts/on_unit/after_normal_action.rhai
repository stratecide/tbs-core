
fn exhaust() {
    exhaust_all_on_chess_board(PATH.start, UNIT.owner_id);
    EVENT_HANDLER.set_unit_flag(POSITION, FLAG_Exhausted);
    EVENT_HANDLER.remove_unit_flag(POSITION, FLAG_Unmoved);
}
