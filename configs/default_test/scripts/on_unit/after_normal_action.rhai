
fn exhaust(CONTEXT) {
    exhaust_all_on_chess_board(CONTEXT.event_handler, CONTEXT.path.start, CONTEXT.unit.owner_id);
    CONTEXT.event_handler.set(CONTEXT.unit_id, FLAG_Exhausted);
    CONTEXT.event_handler.remove(CONTEXT.unit_id, FLAG_Unmoved);
}

fn setup_en_passant(CONTEXT) {
    if CONTEXT.path.len > 1 {
        let path = CONTEXT.path;
        path.pop();
        CONTEXT.event_handler.set_unit_tag(CONTEXT.position, TAG_EnPassant, path.end(CONTEXT.board));
    }
}
