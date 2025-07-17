
fn co_rot() {
    place_sludge_tokens(OWNER_ID, 0);
}

fn co_decay() {
    place_sludge_tokens(OWNER_ID, 1);
}

fn place_sludge_tokens(OWNER_ID, counter) {
    for uwp in BOARD.all_units() {
        if uwp.unit.owner_id != () {
            let token = Token("Sludge");
            let old_token = for t in BOARD.get_tokens(uwp.position) {
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
