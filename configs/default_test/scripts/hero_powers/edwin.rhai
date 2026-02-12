
fn ua_power_input(CONTEXT) {
    let points = CONTEXT.board.get_neighbors(CONTEXT.position);
    let options = new_point_selection();
    for point in points {
        if point != CONTEXT.position && CONTEXT.board.get_unit(point) != () {
            options += point;
        }
    }
    let target1 = CONTEXT.player.choice(options, false);
    let options = new_point_selection();
    for point in CONTEXT.board.positions_in_range(CONTEXT.position, 5) {
        if point != target1 && point != CONTEXT.position && CONTEXT.board.get_unit(point) != () {
            options += point;
        }
    }
    CONTEXT.player.choice(options, false);
    success()
}

fn ua_power(CONTEXT, data) {
    exhaust_all_on_chess_board(CONTEXT.event_handler, CONTEXT.path.start, CONTEXT.unit.owner_id);
    CONTEXT.event_handler.set_unit_flag(CONTEXT.position, FLAG_Exhausted);
    let target1 = data[0];
    let target2 = data[1];
    let unit1 = CONTEXT.event_handler.take_unit(target1);
    let unit2 = CONTEXT.event_handler.take_unit(target2);
    CONTEXT.event_handler.place_unit(target1, unit2);
    CONTEXT.event_handler.place_unit(target2, unit1);
}
