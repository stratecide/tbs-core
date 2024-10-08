
fn uf_can_repair() {
    let terrain = BOARD.get_terrain(POSITION);
    UNIT.hp < 100 
        && UNIT.owner_id == terrain.owner_id
        && BOARD.table_entry("TERRAIN_REPAIR", UNIT.type, terrain.type)
        // TODO: SludgeMonster can't repair
}

fn ua_self_30() {
    ua_repair_self(30)
}
