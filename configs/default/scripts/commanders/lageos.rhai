
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
        let damages = new_mass_damage();
        for position in BOARD.positions_in_range(position, 1) {
            let unit = BOARD.get_unit(position);
            if unit != () {
                damages.add(position, 30);
            }
        }
        EVENT_HANDLER.damage_units(damages);
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
        let damages = new_mass_damage();
        let stuns = [];
        for position in BOARD.positions_in_range(position, 1) {
            let unit = BOARD.get_unit(position);
            if unit != () {
                damages.add(position, 30);
                stuns.push(position);
            }
        }
        EVENT_HANDLER.damage_units(damages);
        for position in stuns {
            EVENT_HANDLER.set_unit_status(position, status_exhausted());
        }
    }
}
