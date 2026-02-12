
fn co_rockets_input(CONTEXT) {
    let options = new_point_selection();
    for point in CONTEXT.board.all_positions() {
        options += point
    }
    CONTEXT.player.choice(options, false);
    CONTEXT.player.choice(options, false);
    CONTEXT.player.choice(options, false);
    success()
}

fn co_rockets(CONTEXT, data) {
    for position in data {
        let effects = [];
        for position in CONTEXT.board.positions_in_range(position, 1) {
            let unit = CONTEXT.board.get_unit(position);
            if unit != () && unit.has(TAG_Hp) {
                effects.push(change_unit_hp(CONTEXT.event_handler, position, -20));
            }
        }
        CONTEXT.event_handler.effects(effects);
    }
}

fn co_stun_rockets_input(CONTEXT) {
    let options = new_point_selection();
    for point in CONTEXT.board.all_positions() {
        options += point
    }
    CONTEXT.player.choice(options, false);
    success()
}

fn co_stun_rockets(CONTEXT, data) {
    for position in data {
        let effects = [];
        for position in CONTEXT.board.positions_in_range(position, 1) {
            let unit = CONTEXT.board.get_unit(position);
            if unit != () {
                if unit.has(TAG_Hp) {
                    effects.push(change_unit_hp(CONTEXT.event_handler, position, -30));
                }
                if unit.owner_id == CONTEXT.owner_id && !unit.has(FLAG_Exhausted) {
                    CONTEXT.event_handler.set_unit_flag(position, FLAG_Exhausted);
                } else if unit.owner_id >= 0 {
                    CONTEXT.event_handler.set_unit_flag(position, FLAG_Stunned);
                }
            }
        }
        CONTEXT.event_handler.effects(effects);
    }
}
