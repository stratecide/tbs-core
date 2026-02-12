
fn multiply_hp(CONTEXT, value) {
    let hp = CONTEXT.unit.get(TAG_Hp);
    if hp != () {
        value * hp / 100
    } else {
        value
    }
}

fn add_half_missing_hp(CONTEXT, value) {
    let hp = CONTEXT.unit.get(TAG_Hp);
    if hp != () {
        value + Fraction(100 - hp, 200)
    } else {
        value
    }
}

fn terrain_defense(CONTEXT, value) {
    let terrain = CONTEXT.board.get_terrain(CONTEXT.position);
    value + CONTEXT.config.table_entry("TERRAIN_DEFENSE", CONTEXT.unit.movement_type, terrain.type)
}
