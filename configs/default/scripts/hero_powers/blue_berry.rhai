
fn ua_power() {
    for position in BOARD.get_neighbors(POSITION) {
        if position == POSITION {
            continue;
        }
        let unit = BOARD.get_unit(position);
        if unit?.owner_id == UNIT.owner_id && unit.has(FLAG_Exhausted) {
            EVENT_HANDLER.remove_unit_flag(position, FLAG_Exhausted);
        }
    }
    EVENT_HANDLER.set_unit_flag(POSITION, FLAG_Exhausted);
}
