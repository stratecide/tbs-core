
fn income() {
    let income = (BOARD.get(OWNER_ID, TAG_Income) ?? 0) * BOARD.player_income(OWNER_ID);
    let funds = BOARD.get(OWNER_ID, TAG_Funds) ?? 0;
    EVENT_HANDLER.set(OWNER_ID, TAG_Funds, funds + income.floor());
}
