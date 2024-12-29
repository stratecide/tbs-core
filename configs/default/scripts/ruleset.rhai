
fn unit_death_test(unit, position) {
    (unit.get(TAG_Hp) ?? 100) <= 0
}

fn unit_movable_test() {
    !UNIT.has(FLAG_Exhausted) && !UNIT.has(FLAG_Stunned)
}

fn calculate_attack_damage(base_damage) {
    let defense = defender_bonus("Defense", Fraction(1));
    let damage = base_damage * attacker_bonus("Attack", Fraction(1));
    if defense > Fraction(1) {
        damage / defense
    } else {
        damage * (Fraction(2) - defense)
    }
}

fn deal_damage_to_unit(unit, damage) {
    let hp = unit.get(TAG_Hp);
    if hp != () {
        set(TAG_Hp, hp - damage);
    }
    effect(FX_HpChange(-damage))
}

fn close_range_weapon_effects(BOARD, ATTACKER_POSITION, DEFENDER_POSITIONS, fx_weapon) {
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
    effects(effects);
}

fn weapon_effects() {
    let weapon = ATTACKER.get_custom("Weapon");
    switch weapon {
        "Flame" => {
            // fire follows a path
            let path = Path(ATTACKER_POSITION);
            path.add(ATTACK_DIRECTION);
            path.add(path.distortion.update_straight_direction(ATTACK_DIRECTION));
            effect(path, FX_Flame());
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
            effects(effects);
            // and afterwards effects at defender positions
            for p in DEFENDER_POSITIONS {
                effects.push(FX_Shells().at(p))
            }
            effects(effects);
        }
        "MachineGun" | "Rifle" | "AntiAir" => {
            close_range_weapon_effects(BOARD, ATTACKER_POSITION, DEFENDER_POSITIONS, Fn("FX_Rifle"))
        }
        "Shells" | "Rocket" => {
            close_range_weapon_effects(BOARD, ATTACKER_POSITION, DEFENDER_POSITIONS, Fn("FX_Shells"))
        }
    }
}

fn attack_target_rook() {
    UNIT.owner_id == OTHER_UNIT.owner_id && UNIT.has(FLAG_Unmoved) && OTHER_UNIT.has(FLAG_Unmoved)
}
