
fn uf_can_repair() {
    let terrain = BOARD.get_terrain(POSITION);
    UNIT.get(TAG_Hp) ?? 100 < 100 
        && UNIT.owner_id == terrain.owner_id
        && CONFIG.table_entry("TERRAIN_REPAIR", UNIT.type, terrain.type)
        // TODO: SludgeMonster can't repair
        // TODO: check if unit can have "Repairing" status
}

fn ua_self_30() {
    repair_unit(EVENT_HANDLER, UNIT, POSITION, 30)
}




fn repair_unit(EVENT_HANDLER, unit, position, amount) {
    let hp = unit.get(TAG_Hp) ?? 100;
    amount = min(amount, 100 - hp);
    let unit2 = unit;
    unit2.set(TAG_Hp, 100);
    let full_price = unit2.value(position);
    if full_price > 0 {
        let funds = BOARD.get(unit.owner_id, TAG_Funds) ?? 0;
        amount = min(amount, funds * 100 / full_price);
    }
    if amount > 0 {
        let cost = full_price * amount / 100;
        EVENT_HANDLER.set(unit.owner_id, TAG_Funds, (BOARD.get(unit.owner_id, TAG_Funds) ?? 0) - cost);
        EVENT_HANDLER.set_unit_flag(position, FLAG_Repairing);
        EVENT_HANDLER.effect(change_unit_hp(position, amount));
    }
    EVENT_HANDLER.set_unit_flag(position, FLAG_Exhausted);
}
