
fn tick_sludge() {
    let counter = TOKEN.get(TAG_SludgeCounter) ?? 0;
    let token = EVENT_HANDLER.remove_token(POSITION, TOKEN);
    if counter > 0 {
        token.set(TAG_SludgeCounter, counter - 1);
        EVENT_HANDLER.place_token(POSITION, token);
    }
}
