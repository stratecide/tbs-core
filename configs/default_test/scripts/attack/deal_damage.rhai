
fn deal_damage_to_unit(CONTEXT) {
    if CONTEXT.splash_distance == 0 {
        let points = [];
        for target in CONTEXT.targets {
            let p = target.point;
            if points.index_of(p) < 0 {
                points.push(p);
            }
        }
        CONTEXT.attack.add_script(
            Script("do_weapon_effects", [CONTEXT.attacker_direction, points])
        );
    }
    let arguments = [];
    let defender_ids = [];
    let lifesteal_hp = Fraction(0);
    let lifesteal = CONTEXT.attack.attack_bonus("Lifesteal", Fraction(0));
    let strength = CONTEXT.attack.attack_bonus("Strength", Fraction(0)) ** CONTEXT.splash_distance;
    let hero_charge_gain = 10;
    let theft_rate = CONTEXT.attack.attack_bonus("DamageTheft", Fraction(0));
    let stolen = 0;
    for target in CONTEXT.targets {
        // check if there's a unit at target position
        let p = target.point;
        let defender_id = CONTEXT.attack.remember_unit(p);
        if defender_id == () || defender_ids.index_of(defender_id) >= 0 {
            // defender doesn't exist or has already been added
            continue;
        }
        // check if that unit can be damaged by the attacker
        let defender = CONTEXT.board.get_unit(p);
        let base_attack = CONTEXT.config.table_entry("ATTACK_DAMAGE", defender.type, CONTEXT.attacker.type);
        if base_attack == 0 {
            continue;
        }
        // calculate damage dealt
        let defense = CONTEXT.attack.defender_bonus(defender_id, "Defense", Fraction(1));
        let attack = Fraction(base_attack) * strength * CONTEXT.attack.attacker_bonus(defender_id, "Attack", Fraction(1));
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
        CONTEXT.attack.on_defend(OnDefendScript("OnDefend", defender_id).with_arguments([damage, target.direction]));
    }
    if arguments.len > 0 {
        CONTEXT.attack.add_script(
            Script("do_deal_damage_to_units", [arguments])
        );
        for hero in CONTEXT.heroes.get(CONTEXT.attacker_position, CONTEXT.attacker.owner_id) {
            CONTEXT.attack.add_script(
                Script("do_charge_hero", [hero.unit_id, hero_charge_gain])
            );
        }
    }
    if lifesteal_hp > Fraction(0) {
        CONTEXT.attack.add_script(
            Script("do_lifesteal", [CONTEXT.attacker_id, lifesteal_hp.ceil()])
        );
    }
    stolen = (Fraction(stolen) * theft_rate).floor();
    if stolen > 0 {
        CONTEXT.attack.add_script(
            Script("do_theft", [CONTEXT.attacker.owner_id, stolen])
        );
    }
}

fn do_charge_hero(CONTEXT, hero_id, gain) {
    CONTEXT.event_handler.add_hero_charge(hero_id, gain);
}

fn do_deal_damage_to_units(CONTEXT, arguments) {
    let effects = [];
    for argument in arguments {
        let defender_id = argument[0];
        let damage = argument[1];
        let defender = CONTEXT.event_handler.get_unit(defender_id);
        if defender == () {
            // unit has been removed in the mean time
            continue;
        }
        let hp = defender.get(TAG_Hp);
        let position = CONTEXT.event_handler.get_unit_position(defender_id);
        if hp != () {
            CONTEXT.event_handler.set(defender_id, TAG_Hp, hp - damage);
            damage = min(hp, damage);
            if damage > 0 && CONTEXT.attacker.owner_id != defender.owner_id {
                let charge = unit_value(defender, damage);
                CONTEXT.event_handler.add_commander_charge(CONTEXT.attacker.owner_id, charge / 2);
                CONTEXT.event_handler.add_commander_charge(defender.owner_id, charge);
            }
        }
        effects.push(FX_HpChange(-damage).at(position));
    }
    CONTEXT.event_handler.effects(effects);
}

fn do_lifesteal(CONTEXT, attacker_id, hp_gain) {
    let position = CONTEXT.event_handler.get_unit_position(attacker_id);
    if position == () {
        // unit has been removed in the mean time
        return;
    }
    let hp = CONTEXT.event_handler.get(attacker_id, TAG_Hp);
    if hp != () {
        CONTEXT.event_handler.set(attacker_id, TAG_Hp, hp + hp_gain);
    }
    CONTEXT.event_handler.effect(FX_HpChange(hp_gain).at(position));
}

fn do_theft(CONTEXT, owner_id, money_gain) {
    let funds = CONTEXT.board.get(owner_id, TAG_Funds) ?? 0;
    CONTEXT.event_handler.set(owner_id, TAG_Funds, funds + money_gain);
}

// send animation events for weapon use
fn do_weapon_effects(CONTEXT, attacker_direction, defender_positions) {
    let weapon = CONTEXT.attacker.config.get_custom_value(CONTEXT.attacker.type, "Weapon");
    switch weapon {
        "Flame" => {
            // fire follows a path
            let path = Path(CONTEXT.attacker_position);
            path.add(CONTEXT.board, attacker_direction);
            path.add(CONTEXT.board, path.distortion(CONTEXT.board).update_straight_direction(attacker_direction));
            CONTEXT.event_handler.effect(path, FX_Flame());
        }
        "SurfaceMissiles" => {
            // show effect at attacker position
            let effects = [FX_Attacker().at(CONTEXT.attacker_position)];
            let teams = [];
            for p in defender_positions {
                let unit = CONTEXT.board.get_unit(p);
                if unit == () {
                    continue;
                }
                let team = unit.team;
                if team >= 0 && !teams.contains(team) && !CONTEXT.board.is_unit_visible(CONTEXT.attacker_position, team) {
                    teams.push(team);
                    effects.push(FX_AttackerTeam(team).at(CONTEXT.attacker_position))
                }
            }
            CONTEXT.event_handler.effects(effects);
            // and afterwards effects at defender positions
            for p in defender_positions {
                effects.push(FX_Shells().at(p))
            }
            CONTEXT.event_handler.effects(effects);
        }
        "MachineGun" | "Rifle" | "AntiAir" => {
            close_range_weapon_effects(CONTEXT, defender_positions, Fn("FX_Rifle"))
        }
        "Shells" => {
            close_range_weapon_effects(CONTEXT, defender_positions, Fn("FX_Shells"))
        }
        "Rocket" => {
            close_range_weapon_effects(CONTEXT, defender_positions, Fn("FX_Shells"))
        }
    }
}

fn close_range_weapon_effects(CONTEXT, defender_positions, fx_weapon) {
    // show effect at attacker position at the same time as effects at defender positions
    let effects = [FX_Attacker().at(CONTEXT.attacker_position)];
    let teams = [];
    for p in defender_positions {
        let unit = CONTEXT.board.get_unit(p);
        if unit == () {
            continue;
        }
        let team = unit.team;
        if team >= 0 && !teams.contains(team) && !CONTEXT.board.is_unit_visible(CONTEXT.attacker_position, team) {
            teams.push(team);
            effects.push(FX_AttackerTeam(team).at(CONTEXT.attacker_position))
        }
        effects.push(fx_weapon.call().at(p))
    }
    CONTEXT.event_handler.effects(effects);
}
