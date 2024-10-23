
fn multiply_hp(value) {
    let hp = UNIT.get_tag(Hp);
    if hp != () {
        value * hp / 100
    } else {
        value
    }
}
