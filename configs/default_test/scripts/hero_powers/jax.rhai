
fn ua_power_input() {
    let options = new_point_selection();
    for point in BOARD.positions_in_range(POSITION, 3) {
        if BOARD.get_unit(point) == () && BOARD.get_terrain(point).movement_cost(UNIT.movement_type) != () {
            options += point;
        }
    }
    let jump_target = PLAYER.choice(options, false);
    let options = new_direction_selection(jump_target);
    for n in BOARD.get_neighbors_with_direction(jump_target) {
        options += n.direction
    }
    PLAYER.choice(options, false);
    success()
}

fn ua_power(data) {
    let jump_target = data[0];
    let direction = data[1];
    if BOARD.get_unit(jump_target) != () {
        // TODO: give true-sight at jump_target, show alert and give charge cost back
        let vision = new_vision_map();
        vision.add(jump_target, FOG_TrueSight);
        EVENT_HANDLER.grant_vision(UNIT.team, vision);
        EVENT_HANDLER.set_hero_charge(POSITION, UNIT.get_max_charge())
    } else {
        // jump to selected position
        let self = EVENT_HANDLER.take_unit(POSITION);
        EVENT_HANDLER.place_unit(jump_target, self);

        // attack in selected direction and its neighboring directions
        EVENT_HANDLER.attack(self, jump_target, direction.rotate_counter_clockwise());
        EVENT_HANDLER.attack(self, jump_target, direction);
        EVENT_HANDLER.attack(self, jump_target, direction.rotate_clockwise());
    }
    exhaust_all_on_chess_board(EVENT_HANDLER, PATH.start, UNIT.owner_id);
    EVENT_HANDLER.set_unit_flag(POSITION, FLAG_Exhausted);
}
