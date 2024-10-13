
fn co_spirit_seed_input() {
    let points = BOARD.all_positions();
    let options = new_point_selection();
    for point in points {
        if BOARD.get_terrain(point).type == "Grass" {
            options += point
        }
    }
    user_selection(options, false);
    success()
}

fn co_spirit_seed(data) {
    let target = data[0];
    let terrain = CONFIG.build_terrain("FairyForest")
        .owner_id(OWNER_ID)
        .build();
    EVENT_HANDLER.replace_terrain(target, terrain);
}

fn co_fairy_fury() {
    // 20 damage to all enemies near fairy forests (2 distance)
    // fairy forests vanish
    let damages = new_mass_damage();
    for point in BOARD.all_positions() {
        let terrain = BOARD.get_terrain(point);
        if terrain.type == "FairyForest" && terrain.owner_id == OWNER_ID {
            for point in BOARD.positions_in_range(point, 2) {
                let unit = BOARD.get_unit(point);
                if unit?.owner_id >= 0 && unit.team != TEAM && !damages.contains(point) {
                    damages.add(point, 20);
                }
            }
            EVENT_HANDLER.replace_terrain(point, CONFIG.build_terrain("Grass").build());
        }
    }
    EVENT_HANDLER.damage_units(damages);
}
