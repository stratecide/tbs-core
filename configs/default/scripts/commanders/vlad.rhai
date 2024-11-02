
fn co_red_dance() {
    let damages = [];
    for point in BOARD.all_positions() {
        let unit = BOARD.get_unit(point);
        if unit?.owner_id == OWNER_ID {
            for point in BOARD.positions_in_range(point, 4) {
                let unit = BOARD.get_unit(point);
                if unit != () && unit.owner_id >= 0 && unit.team != TEAM && !damages.contains(point) {
                    damages.push(point);
                    EVENT_HANDLER.set_unit_tag(point, TAG_Hp, unit.get(TAG_Hp) - 10);
                }
            }
        }
    }
}

fn co_crimson_rain() {
    let damages = [];
    for point in BOARD.all_positions() {
        let unit = BOARD.get_unit(point);
        if unit?.owner_id == OWNER_ID {
            for point in BOARD.positions_in_range(point, 4) {
                let unit = BOARD.get_unit(point);
                if unit != () && unit.owner_id >= 0 && unit.team != TEAM && !damages.contains(point) {
                    damages.push(point);
                    EVENT_HANDLER.set_unit_tag(point, TAG_Hp, unit.get(TAG_Hp) - 10);
                }
            }
            EVENT_HANDLER.set_unit_tag(point, TAG_Hp, unit.get(TAG_Hp) + 10);
        }
    }
}
