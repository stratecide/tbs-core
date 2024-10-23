
fn multiply_hp(value) {
    let hp = UNIT.get(Hp);
    if hp != () {
        value * hp / 100
    } else {
        value
    }
}
