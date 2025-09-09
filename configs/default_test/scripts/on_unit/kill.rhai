
fn leave_skull(attacker_id, defender_id, damage, direction) {
    if EVENT_HANDLER.get(defender_id, TAG_Hp) == 0 {
        let position = EVENT_HANDLER.get_unit_position(defender_id);
        let defender = EVENT_HANDLER.get_unit(defender_id);
        let skull = Token(CONFIG, TOKEN_Skull);
        skull.copy_from(defender);
        skull.owner_id = EVENT_HANDLER.get_unit(attacker_id).owner_id;
        skull.set(TAG_UnitType, defender.type);
        if defender.movement_type != defender.movement_types[0] {
            skull.set(TAG_MovementType, defender.movement_type);
        }
        skull.remove(FLAG_Exhausted);
        skull.remove(FLAG_Capturing);
        skull.remove(FLAG_Repairing);
        skull.remove(TAG_Hp);
        EVENT_HANDLER.place_token(position, skull);
    }
}

fn resurrect(attacker_id, defender_id, damage, direction) {
    if EVENT_HANDLER.get(defender_id, TAG_Hp) == 0 && EVENT_HANDLER.get_unit_transport_index(defender_id) == () {
        let position = EVENT_HANDLER.get_unit_position(defender_id);
        let defender = EVENT_HANDLER.get_unit(defender_id);
        let unit = Unit(CONFIG, defender.type);
        unit.copy_from(defender);
        unit.owner_id = EVENT_HANDLER.get_unit(attacker_id).owner_id;
        unit.movement_type = defender.movement_type;
        unit.set(TAG_Hp, 50);
        unit.set(FLAG_Zombified);
        unit.remove(FLAG_Exhausted);
        unit.remove(FLAG_Capturing);
        unit.remove(FLAG_Repairing);
        EVENT_HANDLER.take_unit(position);
        EVENT_HANDLER.place_unit(position, unit);
    }
}

fn level_up(attacker_id, defender_id, damage, direction) {
    if EVENT_HANDLER.get(defender_id, TAG_Hp) == 0 {
        EVENT_HANDLER.set(attacker_id, TAG_Level, (EVENT_HANDLER.get(attacker_id, TAG_Level) ?? 0) + 1);
    }
}

fn capture_neutral(attacker_id, defender_id, damage, direction) {
    if EVENT_HANDLER.get(defender_id, TAG_Hp) == 0 {
        let position = EVENT_HANDLER.get_unit_position(defender_id);
        let defender = EVENT_HANDLER.get_unit(defender_id);
        let new_unit = Unit(CONFIG, defender.type);
        new_unit.copy_from(defender);
        if defender.owner_id == -1 {
            new_unit.owner_id = EVENT_HANDLER.get_unit(attacker_id).owner_id;
        } else {
            new_unit.owner_id = -1;
        }
        new_unit.set(TAG_Hp, 50);
        // TODO: show effect?
        EVENT_HANDLER.take_unit(position);
        EVENT_HANDLER.place_unit(position, new_unit);
    }
}
