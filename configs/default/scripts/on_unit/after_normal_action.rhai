
fn exhaust() {
    exhaust_all_on_chess_board(PATH.start, UNIT.owner_id);
    EVENT_HANDLER.set_unit_flag(POSITION, FLAG_Exhausted);
    EVENT_HANDLER.remove_unit_flag(POSITION, FLAG_Unmoved);
}

fn setup_en_passant() {
    if PATH.len > 1 {
        let path = PATH;
        path.pop();
        EVENT_HANDLER.set_unit_tag(POSITION, TAG_EnPassant, path.end);
    }
}
