
fn uf_can_repair() {
    let terrain = BOARD.get_terrain(POSITION);
    UNIT.hp < 100 
        && UNIT.owner_id == terrain.owner_id
        && CONFIG.table_entry("TERRAIN_REPAIR", UNIT.type, terrain.type)
        // TODO: SludgeMonster can't repair
        // TODO: check if unit can have "Repairing" status
}

fn ua_self_30() {
    repair_self(30, BOARD, POSITION, UNIT, EVENT_HANDLER)
}




fn repair_self(amount, BOARD, POSITION, UNIT, EVENT_HANDLER) {
    amount = min(amount, 100 - UNIT.hp);
    let full_price = UNIT.full_price(BOARD, POSITION);
    if full_price > 0 {
        let funds = BOARD.player_funds(UNIT.owner_id);
        amount = min(amount, funds * 100 / full_price);
    }
    if amount > 0 {
        let cost = full_price * amount / 100;
        EVENT_HANDLER.spend_money(UNIT.owner_id, cost);
        EVENT_HANDLER.heal_unit(POSITION, amount);
        EVENT_HANDLER.set_unit_status(POSITION, status_repairing());
    }
}
