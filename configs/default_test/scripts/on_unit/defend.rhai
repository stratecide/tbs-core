
fn puffer_fish() {
    let position = get_unit_position(DEFENDER_ID);
    let damage = 0;
    for argument in ATTACK_ARGUMENTS[0] {
        if damage > 0 && argument[0] == DEFENDER_ID {
            damage = argument[1];
            break;
        }
    }
    for i in 0..TARGETS.len {
        if position == TARGETS[i].point {
            let direction = TARGETS[i].direction;
            if damage > 0 {
                add_script(
                    Script("do_get_pushed", [DEFENDER_ID, direction, Fraction(damage, 10).ceil()])
                    .sticky()
                    .priority(1)
                );
            }
            add_attack(
                Attack(DEFENDER_ID, DEFENDER_ID)
                .with_priority(PRIORITY + 1)
                //.damage_factor(Fraction(damage, 100))
            );
            break;
        }
    }
}

fn do_get_pushed(unit_id, direction, distance) {
    let position = EVENT_HANDLER.get_unit_position(DEFENDER_ID);
    let transport_index = EVENT_HANDLER.get_unit_transport_index(DEFENDER_ID);
    let direction = unit_id.distortion.update_straight_direction(direction);
    if position == () || transport_index != () {
        return;
    }
    let unit = EVENT_HANDLER.get_unit(DEFENDER_ID);
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

/*fn puffer_fish() {
    // Ricochet(10)
    let position = EVENT_HANDLER.get_unit_position(DEFENDER_ID);
    let transport_index = EVENT_HANDLER.get_unit_transport_index(DEFENDER_ID);
    let direction = get_blast_direction();
    if position == () || transport_index != () {
        return;
    }
    let damage_factor = Fraction(DAMAGE, 400);
    if direction != () {
        let path = Path(position);
        while DAMAGE > 10 * path.len && path.add(path.distortion.update_straight_direction(direction)) {
            let terrain = BOARD.get_terrain(path.end);
            let obstacle = BOARD.get_unit(path.end);
            if obstacle != () || terrain.movement_cost(UNIT.movement_type) == () {
                path.pop();
                break;
            }
        }
        damage_factor = Fraction(DAMAGE - 10 * path.len, 100);
        position = path.end;
        EVENT_HANDLER.move_unit(path);
    }
    if damage_factor > 0 {
        // Attack(NoCounter, true, true)
        EVENT_HANDLER.sneak_attack(AttackVector(position), position, UNIT, damage_factor, DEFENDER_ID);
    }
}*/

fn remove_unmoved() {
    add_script(
        Script("do_remove_unmoved", [DEFENDER_ID])
    );
}

fn do_remove_unmoved(unit_id) {
    EVENT_HANDLER.remove(unit_id, FLAG_Unmoved);
}

/*fn remove_unmoved() {
    let position = EVENT_HANDLER.get_unit_position(DEFENDER_ID);
    EVENT_HANDLER.remove_unit_flag(position, FLAG_Unmoved);
}*/
