
fn uf_can_buy_hero() {
    for uwp in BOARD.all_units() {
        if uwp.unit.owner_id == UNIT.owner_id && uwp.unit.get(TAG_HeroOrigin) == POSITION {
            return false;
        }
    }
    true
}

fn ua_buy_hero_input() {
    let options = new_shop("Summon Hero");
    let funds = BOARD.get(UNIT.owner_id, TAG_Funds) ?? 0;
    for hero_type in CONFIG.table_column("UNIT_HEROES", UNIT.type, true) {
        let item = ShopItem(hero_type);
        // TODO: could read cost (modification) from another table or even the same one
        let cost = 500 + UNIT.value(POSITION) * 100 / (UNIT.get(TAG_Hp) ?? 100);
        item.costs.push(cost);
        if cost > funds {
            item.enabled = false;
        }
        if item.enabled {
            for uwp in BOARD.all_units() {
                if uwp.unit.owner_id == UNIT.owner_id && uwp.unit.hero?.type == hero_type {
                    item.enabled = false;
                    break;
                }
            }
        }
        options.add(item);
    }
    user_selection(options, false);
    success()
}

fn ua_buy_hero(data) {
    EVENT_HANDLER.set_unit_flag(POSITION, FLAG_Exhausted);
    let hero_type = data[0].key;
    let cost = data[0].costs[0];
    EVENT_HANDLER.set(UNIT.owner_id, TAG_Funds, (BOARD.get(UNIT.owner_id, TAG_Funds) ?? 0) - cost);
    EVENT_HANDLER.set_hero(POSITION, Hero(hero_type));
    EVENT_HANDLER.set_unit_tag(POSITION, TAG_HeroOrigin, POSITION);
}
