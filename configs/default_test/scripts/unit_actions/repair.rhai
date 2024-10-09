
fn uf_can_repair() {
    let terrain = BOARD().get_terrain(POSITION);
    UNIT.hp < 100 
        && UNIT.owner_id == terrain.owner_id
        && CONFIG().table_entry("TERRAIN_REPAIR", UNIT.type, terrain.type)
        // TODO: SludgeMonster can't repair
        // TODO: check if unit can have "Repairing" status
}

fn ua_self_30() {
    repair_unit(EVENT_HANDLER, UNIT, POSITION, 30)
}




fn repair_unit(EVENT_HANDLER, unit, position, amount) {
    amount = min(amount, 100 - unit.hp);
    let full_price = unit.full_price(position);
    if full_price > 0 {
        let funds = BOARD().player_funds(unit.owner_id);
        amount = min(amount, funds * 100 / full_price);
    }
    if amount > 0 {
        let cost = full_price * amount / 100;
        EVENT_HANDLER.spend_money(unit.owner_id, cost);
        EVENT_HANDLER.heal_unit(position, amount);
        EVENT_HANDLER.set_unit_status(position, status_repairing());
    }
}
