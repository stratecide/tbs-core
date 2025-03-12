
// TODO
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
        "Shells" => {
            close_range_weapon_effects(BOARD, ATTACKER_POSITION, DEFENDER_POSITIONS, Fn("FX_Shells"))
        }
        "Rocket" => {
            close_range_weapon_effects(BOARD, ATTACKER_POSITION, DEFENDER_POSITIONS, Fn("FX_Shells"))
        }
    }
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
