
fn co_red_dance(CONTEXT) {
    let damages = [];
    let effects = [];
    for uwp in CONTEXT.board.all_units() {
        if uwp.unit.owner_id == CONTEXT.owner_id {
            for point in CONTEXT.board.positions_in_range(uwp.position, 4) {
                let unit = CONTEXT.board.get_unit(point);
                if unit != () && unit.owner_id >= 0 && unit.team != CONTEXT.owner_id && !damages.contains(point) {
                    damages.push(point);
                    effects.push(change_unit_hp(CONTEXT.event_handler, point, -10));
                }
            }
        }
    }
    CONTEXT.event_handler.effects(effects);
}

fn co_crimson_rain(CONTEXT) {
    let damages = [];
    let effects = [];
    for uwp in CONTEXT.board.all_units() {
        if uwp.unit.owner_id == CONTEXT.owner_id {
            for point in CONTEXT.board.positions_in_range(uwp.position, 4) {
                let unit = CONTEXT.board.get_unit(point);
                if unit != () && unit.owner_id >= 0 && unit.team != CONTEXT.owner_id && !damages.contains(point) {
                    damages.push(point);
                    effects.push(change_unit_hp(CONTEXT.event_handler, point, -20));
                }
            }
            effects.push(change_unit_hp(CONTEXT.event_handler, uwp.position, 10));
        }
    }
    CONTEXT.event_handler.effects(effects);
}
