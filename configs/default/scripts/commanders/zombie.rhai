
fn co_resurrection() {
    for point in BOARD.all_positions() {
        if BOARD.get_unit(point) != () {
            continue;
        }
        let skull = EVENT_HANDLER.remove_token(point, "Skull", OWNER_ID);
        if skull != () {
            let unit = Unit(skull.get(TAG_UnitType));
            unit.copy_from(skull);
            unit.owner_id = skull.owner_id;
            unit.remove(TAG_UnitType);
            unit.remove(TAG_MovementType);
            if skull.has(TAG_MovementType) {
                unit.movement_type = skull.get(TAG_MovementType);
            }
            unit.set(TAG_Hp, 50);
            unit.set(FLAG_Zombified);
            EVENT_HANDLER.place_unit(point, unit);
        }
    }
}
