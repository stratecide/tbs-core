
fn ua_power() {
    for position in BOARD.get_neighbors(POSITION) {
        if position == POSITION {
            continue;
        }
        let unit = BOARD.get_unit(position);
        if unit?.owner_id == UNIT.owner_id && unit.status == status_exhausted() {
            EVENT_HANDLER.set_unit_status(position, status_ready());
        }
    }
}
