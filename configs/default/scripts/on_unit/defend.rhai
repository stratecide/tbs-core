
fn flee(attacker_id, defender_id, damage, direction) {
    let distance = (damage / Fraction(10)).floor();
    let position = EVENT_HANDLER.get_unit_position(defender_id);
    if distance > 0 && position != () {
        let unit = EVENT_HANDLER.get_unit(defender_id);
        let direction = EVENT_HANDLER.get_unit_distortion(defender_id).update_straight_direction(direction);
        let path = Path(position);
        while path.len < distance && path.add(path.distortion.update_straight_direction(direction)) {
            let terrain = BOARD.get_terrain(path.end);
            let obstacle = BOARD.get_unit(path.end);
            if obstacle != () || terrain.movement_cost(unit.movement_type) == () {
                path.pop();
                break;
            }
        }
        EVENT_HANDLER.move_unit(path);
    }
}

fn attack_self(attacker_id, defender_id, damage, direction) {
    let position = EVENT_HANDLER.get_unit_position(defender_id);
    if position != () {
        add_attack(
            Attack(defender_id, defender_id)
            .with_priority(ATTACK_PRIORITY + 1)
            //.damage_factor(Fraction(damage, 100))
        );
    }
}
