
fn exhaust() {
    exhaust_all_on_chess_board(EVENT_HANDLER, PATH.start, UNIT.owner_id);
    EVENT_HANDLER.set(UNIT_ID, FLAG_Exhausted);
    EVENT_HANDLER.remove(UNIT_ID, FLAG_Unmoved);
}

fn setup_en_passant() {
    if PATH.len > 1 {
        let path = PATH;
        path.pop();
        EVENT_HANDLER.set_unit_tag(POSITION, TAG_EnPassant, path.end(BOARD));
    }
}
