
fn deal_damage_to_unit() {
    if SPLASH_DISTANCE == 0 {
        let points = [];
        for target in TARGETS {
            let p = target.point;
            if points.index_of(p) < 0 {
                points.push(p);
            }
        }
        ATTACK_CONTEXT.add_script(
            Script("do_weapon_effects", [ATTACK_DIRECTION, points])
        );
    }
    let arguments = [];
    let defender_ids = [];
    let lifesteal_hp = Fraction(0);
    let lifesteal = ATTACK_CONTEXT.attack_bonus("Lifesteal", Fraction(0));
    let strength = ATTACK_CONTEXT.attack_bonus("Strength", Fraction(0)) ** SPLASH_DISTANCE;
    let hero_charge_gain = 10;
    let theft_rate = ATTACK_CONTEXT.attack_bonus("DamageTheft", Fraction(0));
    let stolen = 0;
    for target in TARGETS {
        // check if there's a unit at target position
        let p = target.point;
        let defender_id = ATTACK_CONTEXT.remember_unit(p);
        if defender_id == () || defender_ids.index_of(defender_id) >= 0 {
            // defender doesn't exist or has already been added
            continue;
        }
        // check if that unit can be damaged by the attacker
        let defender = BOARD.get_unit(p);
        let base_attack = CONFIG.table_entry("ATTACK_DAMAGE", defender.type, ATTACKER.type);
        if base_attack == 0 {
            continue;
        }
        // calculate damage dealt
        let defense = ATTACK_CONTEXT.defender_bonus(defender_id, "Defense", Fraction(1));
        let attack = Fraction(base_attack) * strength * ATTACK_CONTEXT.attacker_bonus(defender_id, "Attack", Fraction(1));
        let damage = if defense > Fraction(1) {
            attack / defense
        } else {
            attack * (Fraction(2) - defense)
        };
        if damage == Fraction(0) {
            continue;
        } else if damage > Fraction(0) {
            lifesteal_hp += damage * lifesteal;
        }
        damage = damage.ceil();
        let hp = defender.get(TAG_Hp);
        if damage > 0 && hp != () {
            hero_charge_gain += min(damage, hp);
            if damage >= hp {
                hero_charge_gain += 40;
            }
            stolen += unit_value(defender, min(damage, hp));
        }
        // collect defender-id + damage pairs for later execution
        arguments.push([defender_id, damage]);
        defender_ids.push(defender_id);
        ATTACK_CONTEXT.on_defend(OnDefendScript("OnDefend", defender_id).with_arguments([defender, defender_id, damage, target.direction]));
    }
    if arguments.len > 0 {
        ATTACK_CONTEXT.add_script(
            Script("do_deal_damage_to_units", [arguments])
        );
        for hero in HEROES.get(ATTACKER_POSITION, ATTACKER.owner_id) {
            ATTACK_CONTEXT.add_script(
                Script("do_charge_hero", [hero.unit_id, hero_charge_gain])
            );
        }
    }
    if lifesteal_hp > Fraction(0) {
        ATTACK_CONTEXT.add_script(
            Script("do_lifesteal", [ATTACKER_ID, lifesteal_hp.ceil()])
        );
    }
    stolen = (Fraction(stolen) * theft_rate).floor();
    if stolen > 0 {
        ATTACK_CONTEXT.add_script(
            Script("do_theft", [ATTACKER.owner_id, stolen])
        );
    }
}

fn do_charge_hero(hero_id, gain) {
    EVENT_HANDLER.add_hero_charge(hero_id, gain);
}

fn do_deal_damage_to_units(arguments) {
    let effects = [];
    for argument in arguments {
        let defender_id = argument[0];
        let damage = argument[1];
        let defender = EVENT_HANDLER.get_unit(defender_id);
        if defender == () {
            // unit has been removed in the mean time
            continue;
        }
        let hp = defender.get(TAG_Hp);
        let position = EVENT_HANDLER.get_unit_position(defender_id);
        if hp != () {
            EVENT_HANDLER.set(defender_id, TAG_Hp, hp - damage);
            damage = min(hp, damage);
            if damage > 0 && ATTACKER.owner_id != defender.owner_id {
                let charge = unit_value(defender, damage);
                EVENT_HANDLER.add_commander_charge(ATTACKER.owner_id, charge / 2);
                EVENT_HANDLER.add_commander_charge(defender.owner_id, charge);
            }
        }
        effects.push(FX_HpChange(-damage).at(position));
    }
    EVENT_HANDLER.effects(effects);
}

fn do_lifesteal(attacker_id, hp_gain) {
    let position = EVENT_HANDLER.get_unit_position(attacker_id);
    if position == () {
        // unit has been removed in the mean time
        return;
    }
    let hp = EVENT_HANDLER.get(attacker_id, TAG_Hp);
    if hp != () {
        EVENT_HANDLER.set(attacker_id, TAG_Hp, hp + hp_gain);
    }
    EVENT_HANDLER.effect(FX_HpChange(hp_gain).at(position));
}

fn do_theft(owner_id, money_gain) {
    let funds = BOARD.get(owner_id, TAG_Funds) ?? 0;
    EVENT_HANDLER.set(owner_id, TAG_Funds, funds + money_gain);
}

// send animation events for weapon use
fn do_weapon_effects(ATTACK_DIRECTION, DEFENDER_POSITIONS) {
    let weapon = ATTACKER.config.get_custom_value(ATTACKER.type, "Weapon");
    switch weapon {
        "Flame" => {
            // fire follows a path
            let path = Path(ATTACKER_POSITION);
            path.add(BOARD, ATTACK_DIRECTION);
            path.add(BOARD, path.distortion(BOARD).update_straight_direction(ATTACK_DIRECTION));
            EVENT_HANDLER.effect(path, FX_Flame());
        }
        "SurfaceMissiles" => {
            // show effect at attacker position
            let effects = [FX_Attacker().at(ATTACKER_POSITION)];
            let teams = [];
            for p in DEFENDER_POSITIONS {
                let unit = BOARD.get_unit(p);
                if unit == () {
                    continue;
                }
                let team = unit.team;
                if team >= 0 && !teams.contains(team) && !BOARD.is_unit_visible(ATTACKER_POSITION, team) {
                    teams.push(team);
                    effects.push(FX_AttackerTeam(team).at(ATTACKER_POSITION))
                }
            }
            EVENT_HANDLER.effects(effects);
            // and afterwards effects at defender positions
            for p in DEFENDER_POSITIONS {
                effects.push(FX_Shells().at(p))
            }
            EVENT_HANDLER.effects(effects);
        }
        "MachineGun" | "Rifle" | "AntiAir" => {
            close_range_weapon_effects(EVENT_HANDLER, BOARD, ATTACKER_POSITION, DEFENDER_POSITIONS, Fn("FX_Rifle"))
        }
        "Shells" => {
            close_range_weapon_effects(EVENT_HANDLER, BOARD, ATTACKER_POSITION, DEFENDER_POSITIONS, Fn("FX_Shells"))
        }
        "Rocket" => {
            close_range_weapon_effects(EVENT_HANDLER, BOARD, ATTACKER_POSITION, DEFENDER_POSITIONS, Fn("FX_Shells"))
        }
    }
}

fn close_range_weapon_effects(EVENT_HANDLER, BOARD, ATTACKER_POSITION, DEFENDER_POSITIONS, fx_weapon) {
    // show effect at attacker position at the same time as effects at defender positions
    let effects = [FX_Attacker().at(ATTACKER_POSITION)];
    let teams = [];
    for p in DEFENDER_POSITIONS {
        let unit = BOARD.get_unit(p);
        if unit == () {
            continue;
        }
        let team = unit.team;
        if team >= 0 && !teams.contains(team) && !BOARD.is_unit_visible(ATTACKER_POSITION, team) {
            teams.push(team);
            effects.push(FX_AttackerTeam(team).at(ATTACKER_POSITION))
        }
        effects.push(fx_weapon.call().at(p))
    }
    EVENT_HANDLER.effects(effects);
}
