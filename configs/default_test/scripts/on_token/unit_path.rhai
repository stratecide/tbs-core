
fn collect_coins(position, token) {
    EVENT_HANDLER.remove_token(position, token);
    let coins = token.get(TAG_Coins) ?? 0;
    let funds = BOARD.get(UNIT.owner_id, TAG_Funds) ?? 0;
    EVENT_HANDLER.set(UNIT.owner_id, TAG_Funds, funds + Fraction(coins * BOARD.get(UNIT.owner_id, TAG_Income), 2).ceil());
}

fn remove_if_enemy(position, token) {
    if UNIT.team != token.team {
        EVENT_HANDLER.remove_token(position, token);
    }
}
