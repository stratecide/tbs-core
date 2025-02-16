
fn life_steal_20() {
    life_steal_percentage(20)
}

fn life_steal_percentage(percentage) {
    let victims = [];
    for argument in ATTACK_ARGUMENTS[0] {
        let defender_id = argument[0];
        let damage = argument[1];
        if damage > 0 && defender_id.get_unit().team != ATTACKER.team {
            victims.push(argument);
        }
    }
    if victims.len > 0 {
        add_script(
            Script("do_life_steal", [percentage, victims])
            .sticky()
            .priority(-1)
        );
    }
}

fn do_life_steal(percentage, victims) {
    let attacker_position = EVENT_HANDLER.get_unit_position(ATTACKER_ID)?.point;
    if EVENT_HANDLER.get(ATTACKER_ID, TAG_Hp) == () {
        return;
    }
    let total_damage = 0;
    for arg in victims {
        let defender_id = argument[0];
        let damage = argument[1];
        let hp = EVENT_HANDLER.get(defender_id, TAG_Hp);
        if hp != () {
            total_damage += min(hp, damage);
        }
        effects.push(FX_HpChange(-damage).at(position));
    }
    let heal = total_damage * percentage / 100;
    EVENT_HANDLER.set(ATTACKER_ID, TAG_Hp, EVENT_HANDLER.get(ATTACKER_ID, TAG_Hp) + heal);
    EVENT_HANDLER.effect(FX_HpChange(heal).at(EVENT_HANDLER.get_unit_position(ATTACKER_ID).point));
}
