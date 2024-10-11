
fn co_rot() {
    place_sludge_tokens(EVENT_HANDLER, OWNER_ID, 0);
}

fn co_decay() {
    place_sludge_tokens(EVENT_HANDLER, OWNER_ID, 1);
}

fn place_sludge_tokens(EVENT_HANDLER, OWNER_ID, counter) {
    for point in BOARD().all_positions() {
        let unit = BOARD().get_unit(point);
        if unit?.owner_id != () {
            EVENT_HANDLER.place_sludge(point, OWNER_ID, counter);
        }
    }
}
