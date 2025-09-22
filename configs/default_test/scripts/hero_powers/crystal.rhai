
fn ua_summon_obelisk_input() {
    let points = BOARD.get_neighbors(POSITION);
    let options = new_point_selection();
    for point in points {
        if BOARD.get_unit(point) == () {
            options += point
        }
    }
    PLAYER.choice(options, false);
    success()
}

fn ua_summon_obelisk(data) {
    let target = data[0];
    if BOARD.get_unit(target) != () {
        // TODO: show alert
        EVENT_HANDLER.set_hero_charge(POSITION, UNIT.get_max_charge())
    } else {
        let obelisk = Unit(CONFIG, UNIT_HeroCrystal);
        obelisk.owner_id = UNIT.owner_id;
        obelisk.set(TAG_Hp, 100);
        obelisk.hero = Hero(HERO_CrystalObelisk);
        // TODO: show summoning effect
        //EVENT_HANDLER.effect(ef_summon_unit(target, obelisk))
        EVENT_HANDLER.place_unit(target, obelisk);
    }
    exhaust_all_on_chess_board(EVENT_HANDLER, PATH.start, UNIT.owner_id);
    EVENT_HANDLER.set_unit_flag(POSITION, FLAG_Exhausted);
}
