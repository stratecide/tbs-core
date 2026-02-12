
fn ua_summon_obelisk_input(CONTEXT) {
    let points = CONTEXT.board.get_neighbors(CONTEXT.position);
    let options = new_point_selection();
    for point in points {
        if CONTEXT.board.get_unit(point) == () {
            options += point
        }
    }
    CONTEXT.player.choice(options, false);
    success()
}

fn ua_summon_obelisk(CONTEXT, data) {
    let target = data[0];
    if CONTEXT.board.get_unit(target) != () {
        // TODO: show alert
        CONTEXT.event_handler.set_hero_charge(CONTEXT.position, CONTEXT.unit.get_max_charge())
    } else {
        let obelisk = Unit(CONTEXT.config, UNIT_HeroCrystal);
        obelisk.owner_id = CONTEXT.unit.owner_id;
        obelisk.set(TAG_Hp, 100);
        obelisk.hero = Hero(HERO_CrystalObelisk);
        // TODO: show summoning effect
        //CONTEXT.event_handler.effect(ef_summon_unit(target, obelisk))
        CONTEXT.event_handler.place_unit(target, obelisk);
    }
    exhaust_all_on_chess_board(CONTEXT.event_handler, CONTEXT.path.start, CONTEXT.unit.owner_id);
    CONTEXT.event_handler.set_unit_flag(CONTEXT.position, FLAG_Exhausted);
}
