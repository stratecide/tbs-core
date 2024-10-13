
fn ua_summon_obelisk_input() {
    let points = BOARD.get_neighbors(POSITION);
    let options = new_point_selection();
    for point in points {
        if BOARD.get_unit(point) == () {
            options += point
        }
    }
    user_selection(options, false);
    success()
}

fn ua_summon_obelisk(data) {
    let target = data[0];
    if BOARD.get_unit(target) != () {
        // TODO: show alert and give charge cost back
    } else {
        let obelisk = CONFIG.build_unit("HeroCrystal")
            .owner_id(UNIT.owner_id)
            .hero("CrystalObelisk")
            .build();
        // TODO: show summoning effect
        //EVENT_HANDLER.effect(ef_summon_unit(target, obelisk))
        EVENT_HANDLER.place_unit(target, obelisk);
    }
}
