
fn co_resurrection() {
    for point in BOARD().all_positions() {
        if BOARD().get_unit(point) != () {
            continue;
        }
        let skull = BOARD().get_skull(point, OWNER_ID);
        if skull != () {
            EVENT_HANDLER.remove_skull(point, OWNER_ID);
            let unit = skull.build_unit()
                .hp(50)
                .zombified()
                .build();
            EVENT_HANDLER.place_unit(point, unit);
        }
    }
}
