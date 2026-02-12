
fn ua_power_input(CONTEXT) {
    let options = new_direction_selection(CONTEXT.position);
    let summoned = Unit(CONTEXT.config, UNIT_LaserShark);
    for n in CONTEXT.board.get_neighbors_with_direction(CONTEXT.position) {
        if CONTEXT.board.get_unit(n.point) == ()
        && CONTEXT.board.get_terrain(n.point).movement_cost(summoned.movement_type) != () {
            options += n.direction
        }
    }
    CONTEXT.player.choice(options, false);
    success()
}

fn ua_power(CONTEXT, data) {
    let direction = data[0];
    let target = CONTEXT.event_handler.board.get_neighbor(CONTEXT.position, direction);
    if CONTEXT.board.get_unit(target) != () {
        // TODO: show alert and give charge cost back
        CONTEXT.event_handler.set_hero_charge(CONTEXT.position, CONTEXT.unit.get_max_charge())
    } else {
        let summoned = Unit(CONTEXT.config, UNIT_LaserShark);
        summoned.owner_id = CONTEXT.unit.owner_id;
        summoned.set(TAG_Hp, 100);
        // TODO: show summoning effect
        CONTEXT.event_handler.place_unit(target, summoned);
    }
    exhaust_all_on_chess_board(CONTEXT.event_handler, CONTEXT.path.start, CONTEXT.unit.owner_id);
    CONTEXT.event_handler.set_unit_flag(CONTEXT.position, FLAG_Exhausted);
}
