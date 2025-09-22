
fn ua_power_input() {
    let options = new_direction_selection(POSITION);
    let summoned = Unit(CONFIG, UNIT_LaserShark);
    for n in BOARD.get_neighbors_with_direction(POSITION) {
        if BOARD.get_unit(n.point) == ()
        && BOARD.get_terrain(n.point).movement_cost(summoned.movement_type) != () {
            options += n.direction
        }
    }
    PLAYER.choice(options, false);
    success()
}

fn ua_power(data) {
    let direction = data[0];
    let target = EVENT_HANDLER.board.get_neighbor(POSITION, direction);
    if BOARD.get_unit(target) != () {
        // TODO: show alert and give charge cost back
        EVENT_HANDLER.set_hero_charge(POSITION, UNIT.get_max_charge())
    } else {
        let summoned = Unit(CONFIG, UNIT_LaserShark);
        summoned.owner_id = UNIT.owner_id;
        summoned.set(TAG_Hp, 100);
        // TODO: show summoning effect
        EVENT_HANDLER.place_unit(target, summoned);
    }
    exhaust_all_on_chess_board(EVENT_HANDLER, PATH.start, UNIT.owner_id);
    EVENT_HANDLER.set_unit_flag(POSITION, FLAG_Exhausted);
}
