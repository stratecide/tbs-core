
fn co_rot() {
    place_sludge_tokens(EVENT_HANDLER, OWNER_ID, 0);
}

fn co_decay() {
    place_sludge_tokens(EVENT_HANDLER, OWNER_ID, 1);
}

fn place_sludge_tokens(EVENT_HANDLER, OWNER_ID, counter) {
    for uwp in EVENT_HANDLER.board.all_units() {
        if uwp.unit.owner_id != () {
            let token = Token(EVENT_HANDLER.config, TOKEN_Sludge);
            let old_token = for t in EVENT_HANDLER.board.get_tokens(uwp.position) {
                if t.type == token.type {
                    break t;
                }
            };
            if old_token != () {
                if old_token.get(TAG_SludgeCounter) > counter {
                    continue;
                } else {
                    EVENT_HANDLER.remove_token(uwp.position, old_token);
                }
            }
            token.owner_id = OWNER_ID;
            token.set(TAG_SludgeCounter, counter);
            EVENT_HANDLER.place_token(uwp.position, token);
        }
    }
}
