
fn income(CONTEXT) {
    let income = (CONTEXT.board.get(CONTEXT.owner_id, TAG_Income) ?? 0) * CONTEXT.board.player_income(CONTEXT.owner_id);
    let funds = CONTEXT.board.get(CONTEXT.owner_id, TAG_Funds) ?? 0;
    CONTEXT.event_handler.set(CONTEXT.owner_id, TAG_Funds, funds + income.floor());
}
