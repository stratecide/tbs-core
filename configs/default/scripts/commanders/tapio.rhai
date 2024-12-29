
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
    let terrain = Terrain("FairyForest");
    terrain.owner_id = OWNER_ID;
    EVENT_HANDLER.replace_terrain(target, terrain);
}

fn co_fairy_fury() {
    // 20 damage to all enemies near fairy forests (2 distance)
    // fairy forests vanish
    let damages = [];
    for point in BOARD.all_positions() {
        let terrain = BOARD.get_terrain(point);
        if terrain.type == "FairyForest" && terrain.owner_id == OWNER_ID {
            let effects = [];
            for point in BOARD.positions_in_range(point, 2) {
                let unit = BOARD.get_unit(point);
                if unit?.owner_id >= 0 && unit.team != TEAM && !damages.contains(point) {
                    damages.push(point);
                    effects.push(change_unit_hp(point, -20));
                }
            }
            EVENT_HANDLER.effects(effects);
            EVENT_HANDLER.replace_terrain(point, Terrain("Grass"));
        }
    }
}
