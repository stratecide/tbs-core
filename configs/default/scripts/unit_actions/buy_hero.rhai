
fn uf_can_buy_hero() {
    for p in BOARD.all_positions() {
        let unit = BOARD.get_unit(p);
        if unit?.owner_id == UNIT.owner_id && unit.get(TAG_HeroOrigin) == POSITION {
            return false;
        }
    }
    true
}

fn ua_buy_hero_input() {
    let options = new_shop("Summon Hero");
    for hero_type in CONFIG.table_column("UNIT_HEROES", UNIT.type, true) {
        let item = ShopItem(hero_type);
        // TODO: could read cost (modification) from another table or even the same one
        item.costs.push(500 + UNIT.value(POSITION) * 100 / (UNIT.get(TAG_Hp) ?? 100));
        options.add(item);
    }
    user_selection(options, false);
    success()
}

fn ua_buy_hero(data) {
    EVENT_HANDLER.set_unit_flag(POSITION, FLAG_Exhausted);
    let hero_type = data[0].key;
    let cost = data[0].costs[0];
    EVENT_HANDLER.spend_money(UNIT.owner_id, cost);
    EVENT_HANDLER.set_hero(POSITION, Hero(hero_type));
    EVENT_HANDLER.set_unit_tag(POSITION, TAG_HeroOrigin, POSITION);
}
