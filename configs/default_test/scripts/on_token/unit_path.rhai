
fn collect_coins(position, token) {
    EVENT_HANDLER.remove_token(position, token);
    let coins = token.get(TAG_Coins) ?? 0;
    EVENT_HANDLER.gain_money(UNIT.owner_id, Fraction(coins * BOARD.player_income(UNIT.owner_id), 2).ceil());
}

fn remove_if_enemy(position, token) {
    if UNIT.team != token.team {
        EVENT_HANDLER.remove_token(position, token);
    }
}
