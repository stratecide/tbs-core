
fn deal_damage_to_unit() {
    let arguments = [];
    let defender_ids = [];
    for target in TARGETS {
        // check if there's a unit at target position
        let p = target.point;
        let defender_id = remember_unit(p);
        if defender_id == () {
            continue;
        }
        // check if that unit can be damaged by the attacker
        let defender = BOARD.get_unit(p);
        let base_attack = CONFIG.table_entry("ATTACK_DAMAGE", defender.type, ATTACKER.type);
        if base_attack == 0 {
            continue;
        }
        // calculate damage dealt
        let defense = defender_bonus(defender_id, "Defense", Fraction(1));
        let attack = Fraction(base_attack) * attacker_bonus(defender_id, "Attack", Fraction(1));
        let damage = if defense > Fraction(1) {
            attack / defense
        } else {
            attack * (Fraction(2) - defense)
        };
        // collect defender-id + damage pairs for later execution
        let index = defender_ids.index_of(defender_id);
        if index < 0 {
            arguments.push([defender_id, damage]);
            defender_ids.push(defender_id);
        } else {
            argument[index][1] += damage;
        }
    }
    if arguments.len > 0 {
        add_script(
            Script("do_deal_damage_to_units", [arguments])
        );
    }
}

fn do_deal_damage_to_units(arguments) {
    let effects = [];
    for argument in arguments {
        let defender_id = argument[0];
        let damage = argument[1].ceil();
        let position = EVENT_HANDLER.get_unit_position(defender_id);
        if position == () {
            // unit has been removed in the mean time
            continue;
        }
        let hp = EVENT_HANDLER.get(defender_id, TAG_Hp);
        if hp != () {
            EVENT_HANDLER.set(defender_id, TAG_Hp, hp - damage);
        }
        effects.push(FX_HpChange(-damage).at(position));
    }
    EVENT_HANDLER.effects(effects);
}
