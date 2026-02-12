
fn uf_can_repair(CONTEXT) {
    let terrain = CONTEXT.board.get_terrain(CONTEXT.position);
    CONTEXT.unit.get(TAG_Hp) ?? 100 < 100 
        && CONTEXT.unit.owner_id == terrain.owner_id
        && CONTEXT.config.table_entry("TERRAIN_REPAIR", CONTEXT.unit.type, terrain.type)
        // TODO: SludgeMonster can't repair
        // TODO: check if unit can have "Repairing" status
}

fn ua_self_30(CONTEXT) {
    repair_unit(CONTEXT, CONTEXT.unit, CONTEXT.position, 30)
}




fn repair_unit(CONTEXT, unit, position, amount) {
    let hp = unit.get(TAG_Hp) ?? 100;
    amount = min(amount, 100 - hp);
    let full_price = unit_value(unit);
    if full_price > 0 {
        let funds = CONTEXT.event_handler.board.get(unit.owner_id, TAG_Funds) ?? 0;
        amount = min(amount, funds * 100 / full_price);
    }
    if amount > 0 {
        let cost = full_price * amount / 100;
        CONTEXT.event_handler.set(unit.owner_id, TAG_Funds, (CONTEXT.event_handler.board.get(unit.owner_id, TAG_Funds) ?? 0) - cost);
        CONTEXT.event_handler.set_unit_flag(position, FLAG_Repairing);
        CONTEXT.event_handler.effect(change_unit_hp(CONTEXT.event_handler, position, amount));
    }
    CONTEXT.event_handler.set_unit_flag(position, FLAG_Exhausted);
}
