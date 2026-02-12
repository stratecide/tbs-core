
fn co_resurrection(CONTEXT) {
    for point in CONTEXT.board.all_positions() {
        if CONTEXT.board.get_unit(point) != () {
            continue;
        }
        let skull = CONTEXT.event_handler.remove_token(point, TOKEN_Skull, CONTEXT.owner_id);
        if skull != () {
            let unit = Unit(CONTEXT.config, skull.get(TAG_UnitType));
            unit.copy_from(skull);
            unit.owner_id = skull.owner_id;
            unit.remove(TAG_UnitType);
            unit.remove(TAG_MovementType);
            if skull.has(TAG_MovementType) {
                unit.movement_type = skull.get(TAG_MovementType);
            }
            unit.set(TAG_Hp, 50);
            unit.set(FLAG_Zombified);
            CONTEXT.event_handler.place_unit(point, unit);
        }
    }
}
