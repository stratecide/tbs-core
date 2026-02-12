
fn flee(CONTEXT, damage, direction) {
    let distance = damage / 10;
    let position = CONTEXT.event_handler.get_unit_position(CONTEXT.defender_id);
    if distance > 0 && position != () {
        let unit = CONTEXT.event_handler.get_unit(CONTEXT.defender_id);
        let direction = CONTEXT.event_handler.get_unit_distortion(CONTEXT.defender_id).update_straight_direction(direction);
        let path = Path(position);
        while path.len < distance && path.add(CONTEXT.board, path.distortion(CONTEXT.board).update_straight_direction(direction)) {
            let terrain = CONTEXT.board.get_terrain(path.end(CONTEXT.board));
            let obstacle = CONTEXT.board.get_unit(path.end(CONTEXT.board));
            if obstacle != () || terrain.movement_cost(unit.movement_type) == () {
                path.pop();
                break;
            }
        }
        CONTEXT.event_handler.move_unit(path);
    }
}

fn attack_self(CONTEXT, damage, direction) {
    let position = CONTEXT.event_handler.get_unit_position(CONTEXT.defender_id);
    if position != () {
        [
            Attack(CONTEXT.defender_id, CONTEXT.defender_id)
            .with_priority(CONTEXT.attack_priority + 1)
            //.damage_factor(Fraction(damage, 100))
        ]
    }
}
