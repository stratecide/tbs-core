
fn co_rockets_input() {
    let options = new_point_selection();
    for point in BOARD.all_positions() {
        options += point
    }
    user_selection(options, false);
    user_selection(options, false);
    user_selection(options, false);
    success()
}

fn co_rockets(data) {
    for position in data {
        let effects = [];
        for position in BOARD.positions_in_range(position, 1) {
            let unit = BOARD.get_unit(position);
            if unit != () && unit.has(TAG_Hp) {
                effects.push(change_unit_hp(position, -20));
            }
        }
        EVENT_HANDLER.effects(effects);
    }
}

fn co_stun_rockets_input() {
    let options = new_point_selection();
    for point in BOARD.all_positions() {
        options += point
    }
    user_selection(options, false);
    success()
}

fn co_stun_rockets(data) {
    for position in data {
        let effects = [];
        for position in BOARD.positions_in_range(position, 1) {
            let unit = BOARD.get_unit(position);
            if unit != () {
                if unit.has(TAG_Hp) {
                    effects.push(change_unit_hp(position, -30));
                }
                if unit.owner_id == OWNER_ID && !unit.has(FLAG_Exhausted) {
                    EVENT_HANDLER.set_unit_flag(position, FLAG_Exhausted);
                } else if unit.owner_id >= 0 {
                    EVENT_HANDLER.set_unit_flag(position, FLAG_Stunned);
                }
            }
        }
        EVENT_HANDLER.effects(effects);
    }
}
