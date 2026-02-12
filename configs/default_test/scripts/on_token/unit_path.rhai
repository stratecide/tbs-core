
fn collect_coins(CONTEXT) {
    CONTEXT.event_handler.remove_token(CONTEXT.position, CONTEXT.token);
    let coins = CONTEXT.token.get(TAG_Coins) ?? 0;
    let funds = CONTEXT.board.get(CONTEXT.unit.owner_id, TAG_Funds) ?? 0;
    CONTEXT.event_handler.set(CONTEXT.unit.owner_id, TAG_Funds, funds + Fraction(coins * CONTEXT.board.get(CONTEXT.unit.owner_id, TAG_Income), 2).ceil());
}

fn remove_if_enemy(CONTEXT) {
    if CONTEXT.unit.team != CONTEXT.token.team {
        CONTEXT.event_handler.remove_token(CONTEXT.position, CONTEXT.token);
    }
}
