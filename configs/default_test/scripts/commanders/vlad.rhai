
fn co_red_dance() {
    let damages = new_mass_damage();
    for point in BOARD().all_positions() {
        let unit = BOARD().get_unit(point);
        if unit?.owner_id == OWNER_ID {
            for point in BOARD().positions_in_range(point, 4) {
                let unit = BOARD().get_unit(point);
                if unit != () && unit.owner_id >= 0 && unit.team != TEAM && !damages.contains(point) {
                    damages.add(point, 10);
                }
            }
        }
    }
    EVENT_HANDLER.damage_units(damages);
}

fn co_crimson_rain() {
    let damages = new_mass_damage();
    let heals = new_mass_heal();
    for point in BOARD().all_positions() {
        let unit = BOARD().get_unit(point);
        if unit?.owner_id == OWNER_ID {
            for point in BOARD().positions_in_range(point, 4) {
                let unit = BOARD().get_unit(point);
                if unit != () && unit.owner_id >= 0 && unit.team != TEAM && !damages.contains(point) {
                    damages.add(point, 10);
                }
            }
            heals.add(point, 10);
        }
    }
    EVENT_HANDLER.damage_units(damages);
    EVENT_HANDLER.heal_units(heals);
}
