
fn ua_power_input(CONTEXT) {
    let options = new_point_selection();
    for point in CONTEXT.board.positions_in_range(CONTEXT.position, 3) {
        if CONTEXT.board.get_unit(point) == () && CONTEXT.board.get_terrain(point).movement_cost(CONTEXT.unit.movement_type) != () {
            options += point;
        }
    }
    let jump_target = CONTEXT.player.choice(options, false);
    let options = new_direction_selection(jump_target);
    for n in CONTEXT.board.get_neighbors_with_direction(jump_target) {
        options += n.direction
    }
    CONTEXT.player.choice(options, false);
    success()
}

fn ua_power(CONTEXT, data) {
    let jump_target = data[0];
    let direction = data[1];
    if CONTEXT.board.get_unit(jump_target) != () {
        // TODO: give true-sight at jump_target, show alert and give charge cost back
        let vision = new_vision_map();
        vision.add(jump_target, FOG_TrueSight);
        CONTEXT.event_handler.grant_vision(CONTEXT.unit.team, vision);
        CONTEXT.event_handler.set_hero_charge(CONTEXT.position, CONTEXT.unit.get_max_charge())
    } else {
        // jump to selected position
        let self = CONTEXT.event_handler.take_unit(CONTEXT.position);
        CONTEXT.event_handler.place_unit(jump_target, self);

        // attack in selected direction and its neighboring directions
        CONTEXT.event_handler.attack(self, jump_target, direction.rotate_counter_clockwise());
        CONTEXT.event_handler.attack(self, jump_target, direction);
        CONTEXT.event_handler.attack(self, jump_target, direction.rotate_clockwise());
    }
    exhaust_all_on_chess_board(CONTEXT.event_handler, CONTEXT.path.start, CONTEXT.unit.owner_id);
    CONTEXT.event_handler.set_unit_flag(CONTEXT.position, FLAG_Exhausted);
}
