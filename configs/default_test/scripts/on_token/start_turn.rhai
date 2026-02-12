
fn tick_sludge(CONTEXT) {
    let counter = CONTEXT.token.get(TAG_SludgeCounter) ?? 0;
    let token = CONTEXT.event_handler.remove_token(CONTEXT.position, CONTEXT.token);
    if counter > 0 {
        token.set(TAG_SludgeCounter, counter - 1);
        CONTEXT.event_handler.place_token(CONTEXT.position, token);
    }
}
