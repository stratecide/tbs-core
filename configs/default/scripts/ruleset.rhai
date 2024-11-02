
fn unit_death_test(unit, position) {
    (unit.get(TAG_Hp) ?? 100) <= 0
}

fn unit_movable_test() {
    !UNIT.has(FLAG_Exhausted) && !UNIT.has(FLAG_Stunned)
}

fn calculate_attack_damage(base_damage) {
    base_damage * attacker_bonus("Attack", Fraction(1)) / defender_bonus("Defense", Fraction(1))
}

fn deal_damage_to_unit(unit, damage) {
    let hp = unit.get(TAG_Hp);
    if hp != () {
        unit.set(TAG_Hp, hp - damage);
    }
    unit
}
