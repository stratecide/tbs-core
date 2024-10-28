
fn ua_power_input() {
    let points = BOARD.get_neighbors(POSITION);
    let options = new_point_selection();
    for point in points {
        if point != POSITION && BOARD.get_unit(point) != () {
            options += point;
        }
    }
    let target1 = user_selection(options, false);
    let options = new_point_selection();
    for point in BOARD.positions_in_range(POSITION, 5) {
        if point != target1 && point != POSITION && BOARD.get_unit(point) != () {
            options += point;
        }
    }
    user_selection(options, false);
    success()
}

fn ua_power(data) {
    EVENT_HANDLER.set_unit_flag(POSITION, FLAG_Exhausted);
    let target1 = data[0];
    let target2 = data[1];
    let unit1 = EVENT_HANDLER.take_unit(target1);
    let unit2 = EVENT_HANDLER.take_unit(target2);
    EVENT_HANDLER.place_unit(target1, unit2);
    EVENT_HANDLER.place_unit(target2, unit1);
}
