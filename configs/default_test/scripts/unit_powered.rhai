
fn multiply_hp(value) {
    let hp = UNIT.get(TAG_Hp);
    if hp != () {
        value * hp / 100
    } else {
        value
    }
}
