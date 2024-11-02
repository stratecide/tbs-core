
fn multiply_hp(value) {
    let hp = UNIT.get(TAG_Hp);
    if hp != () {
        value * hp / 100
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

fn terrain_defense(value) {
    let terrain = BOARD.get_terrain(POSITION);
    value + CONFIG.table_entry("TERRAIN_DEFENSE", UNIT.movement_type, terrain.type)
}
