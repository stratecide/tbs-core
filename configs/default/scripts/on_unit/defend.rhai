
fn puffer_fish() {
    // Ricochet(10)
    let position = EVENT_HANDLER.get_unit_position(UNIT_ID);
    let transport_index = EVENT_HANDLER.get_unit_transport_index(UNIT_ID);
    let direction = get_blast_direction();
    if position == () || transport_index != () {
        return;
    }
    let damage_factor = Fraction(DAMAGE, 400);
    if direction != () {
        let path = Path(position);
        while DAMAGE > 10 * path.len && path.add(path.distortion.update_straight_direction(direction)) {
            let terrain = BOARD().get_terrain(path.end);
            let obstacle = BOARD().get_unit(path.end);
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
        EVENT_HANDLER.sneak_attack(AttackVector(position), position, UNIT, damage_factor, UNIT_ID);
    }
}
