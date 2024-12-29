
fn co_red_dance() {
    let damages = [];
    let effects = [];
    for point in BOARD.all_positions() {
        let unit = BOARD.get_unit(point);
        if unit?.owner_id == OWNER_ID {
            for point in BOARD.positions_in_range(point, 4) {
                let unit = BOARD.get_unit(point);
                if unit != () && unit.owner_id >= 0 && unit.team != TEAM && !damages.contains(point) {
                    damages.push(point);
                    effects.push(change_unit_hp(point, -10));
                }
            }
        }
    }
    EVENT_HANDLER.effects(effects);
}

fn co_crimson_rain() {
    let damages = [];
    let effects = [];
    for point in BOARD.all_positions() {
        let unit = BOARD.get_unit(point);
        if unit?.owner_id == OWNER_ID {
            for point in BOARD.positions_in_range(point, 4) {
                let unit = BOARD.get_unit(point);
                if unit != () && unit.owner_id >= 0 && unit.team != TEAM && !damages.contains(point) {
                    damages.push(point);
                    effects.push(change_unit_hp(point, -20));
                }
            }
            effects.push(change_unit_hp(point, 10));
        }
    }
    EVENT_HANDLER.effects(effects);
}
