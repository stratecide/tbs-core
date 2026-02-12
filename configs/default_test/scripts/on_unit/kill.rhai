
fn leave_skull(CONTEXT, damage, direction) {
    if CONTEXT.event_handler.get(CONTEXT.defender_id, TAG_Hp) == 0 {
        let position = CONTEXT.event_handler.get_unit_position(CONTEXT.defender_id);
        let defender = CONTEXT.event_handler.get_unit(CONTEXT.defender_id);
        let skull = Token(CONTEXT.config, TOKEN_Skull);
        skull.copy_from(defender);
        skull.owner_id = CONTEXT.attacker.owner_id;
        skull.set(TAG_UnitType, defender.type);
        if defender.movement_type != defender.movement_types[0] {
            skull.set(TAG_MovementType, defender.movement_type);
        }
        skull.remove(FLAG_Exhausted);
        skull.remove(FLAG_Capturing);
        skull.remove(FLAG_Repairing);
        skull.remove(TAG_Hp);
        CONTEXT.event_handler.place_token(position, skull);
    }
}

fn resurrect(CONTEXT, damage, direction) {
    if CONTEXT.event_handler.get(CONTEXT.defender_id, TAG_Hp) == 0 && CONTEXT.event_handler.get_unit_transport_index(CONTEXT.defender_id) == () {
        let position = CONTEXT.event_handler.get_unit_position(CONTEXT.defender_id);
        let defender = CONTEXT.event_handler.get_unit(CONTEXT.defender_id);
        let unit = Unit(CONTEXT.config, defender.type);
        unit.copy_from(defender);
        unit.owner_id = CONTEXT.attacker.owner_id;
        unit.movement_type = defender.movement_type;
        unit.set(TAG_Hp, 50);
        unit.set(FLAG_Zombified);
        unit.remove(FLAG_Exhausted);
        unit.remove(FLAG_Capturing);
        unit.remove(FLAG_Repairing);
        CONTEXT.event_handler.take_unit(position);
        CONTEXT.event_handler.place_unit(position, unit);
    }
}

fn level_up(CONTEXT, damage, direction) {
    let defender_hp = CONTEXT.event_handler.get(CONTEXT.defender_id, TAG_Hp);
    if defender_hp == 0 {
        CONTEXT.event_handler.set(CONTEXT.attacker_id, TAG_Level, (CONTEXT.event_handler.get(CONTEXT.attacker_id, TAG_Level) ?? 0) + 1);
    }
}

fn capture_neutral(CONTEXT, damage, direction) {
    if CONTEXT.event_handler.get(CONTEXT.defender_id, TAG_Hp) == 0 {
        let position = CONTEXT.event_handler.get_unit_position(CONTEXT.defender_id);
        let defender = CONTEXT.event_handler.get_unit(CONTEXT.defender_id);
        let new_unit = Unit(CONTEXT.config, defender.type);
        new_unit.copy_from(defender);
        if defender.owner_id == -1 {
            new_unit.owner_id = CONTEXT.attacker.owner_id;
        } else {
            new_unit.owner_id = -1;
        }
        new_unit.set(TAG_Hp, 50);
        // TODO: show effect?
        CONTEXT.event_handler.take_unit(position);
        CONTEXT.event_handler.place_unit(position, new_unit);
    }
}
