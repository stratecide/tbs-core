
fn leave_skull() {
    let skull = Token("Skull");
    skull.copy_from(OTHER_UNIT);
    skull.owner_id = UNIT.owner_id;
    skull.set(TAG_UnitType, OTHER_UNIT.type);
    if OTHER_UNIT.movement_type != OTHER_UNIT.movement_types[0] {
        skull.set(TAG_MovementType, OTHER_UNIT.movement_type);
    }
    skull.remove(FLAG_Exhausted);
    skull.remove(FLAG_Capturing);
    skull.remove(FLAG_Repairing);
    skull.remove(TAG_Hp);
    EVENT_HANDLER.place_token(OTHER_POSITION, skull);
}

fn resurrect() {
    let unit = Unit(OTHER_UNIT.type);
    unit.copy_from(OTHER_UNIT);
    unit.owner_id = UNIT.owner_id;
    unit.movement_type = OTHER_UNIT.movement_type;
    unit.set(TAG_Hp, 50);
    unit.set(FLAG_Zombified);
    unit.remove(FLAG_Exhausted);
    unit.remove(FLAG_Capturing);
    unit.remove(FLAG_Repairing);
    EVENT_HANDLER.place_unit(OTHER_POSITION, unit);
}

fn level_up(attacker_id, defender_id) {
    if EVENT_HANDLER.get(defender_id, TAG_Hp) == 0 {
        EVENT_HANDLER.set(attacker_id, TAG_Level, (EVENT_HANDLER.get(attacker_id, TAG_Level) ?? 0) + 1);
    }
}
