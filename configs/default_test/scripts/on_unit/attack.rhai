
fn life_steal_20(CONTEXT) {
    life_steal_percentage(CONTEXT, 20)
}

fn life_steal_percentage(CONTEXT, percentage) {
    let victims = [];
    for argument in ATTACK_ARGUMENTS[0] {
        let defender_id = argument[0];
        let damage = argument[1];
        if damage > 0 && defender_id.get_unit().team != CONTEXT.attacker.team {
            victims.push(argument);
        }
    }
    if victims.len > 0 {
        CONTEXT.attack.add_script(
            Script("do_life_steal", [percentage, victims])
            .sticky()
            .priority(-1)
        );
    }
}

fn do_life_steal(CONTEXT, percentage, victims) {
    let attacker_position = CONTEXT.event_handler.get_unit_position(CONTEXT.attacker_id)?.point;
    if CONTEXT.event_handler.get(CONTEXT.attacker_id, TAG_Hp) == () {
        return;
    }
    let total_damage = 0;
    for arg in victims {
        let defender_id = argument[0];
        let damage = argument[1];
        let hp = CONTEXT.event_handler.get(defender_id, TAG_Hp);
        if hp != () {
            total_damage += min(hp, damage);
        }
        effects.push(FX_HpChange(-damage).at(position));
    }
    let heal = total_damage * percentage / 100;
    CONTEXT.event_handler.set(CONTEXT.attacker_id, TAG_Hp, CONTEXT.event_handler.get(CONTEXT.attacker_id, TAG_Hp) + heal);
    CONTEXT.event_handler.effect(FX_HpChange(heal).at(CONTEXT.event_handler.get_unit_position(CONTEXT.attacker_id).point));
}
