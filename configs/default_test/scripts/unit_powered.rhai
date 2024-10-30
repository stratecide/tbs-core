
fn multiply_hp(value) {
    let hp = UNIT.get(TAG_Hp);
    if hp != () {
        value * Fraction(hp, 100)
    } else {
        value
    }
}

fn add_half_missing_hp(value) {
    let hp = UNIT.get(TAG_Hp);
    if hp != () {
        value + Fraction(100 - hp, 200)
    } else {
        value
    }
}
