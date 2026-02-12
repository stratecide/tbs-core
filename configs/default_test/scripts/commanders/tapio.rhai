
fn co_spirit_seed_input(CONTEXT) {
    let points = CONTEXT.board.all_positions();
    let options = new_point_selection();
    for point in points {
        if CONTEXT.board.get_terrain(point).type == TERRAIN_Grass {
            options += point
        }
    }
    CONTEXT.player.choice(options, false);
    success()
}

fn co_spirit_seed(CONTEXT, data) {
    let target = data[0];
    let terrain = Terrain(CONTEXT.config, TERRAIN_FairyForest);
    terrain.owner_id = CONTEXT.owner_id;
    CONTEXT.event_handler.replace_terrain(target, terrain);
}

fn co_fairy_fury(CONTEXT) {
    // 20 damage to all enemies near fairy forests (2 distance)
    // fairy forests vanish
    let damages = [];
    for point in CONTEXT.board.all_positions() {
        let terrain = CONTEXT.board.get_terrain(point);
        if terrain.type == TERRAIN_FairyForest && terrain.owner_id == CONTEXT.owner_id {
            let effects = [];
            for point in CONTEXT.board.positions_in_range(point, 2) {
                let unit = CONTEXT.board.get_unit(point);
                if unit?.owner_id >= 0 && unit.team != CONTEXT.owner_id && !damages.contains(point) {
                    damages.push(point);
                    effects.push(change_unit_hp(CONTEXT.event_handler, point, -20));
                }
            }
            CONTEXT.event_handler.effects(effects);
            CONTEXT.event_handler.replace_terrain(point, Terrain(CONTEXT.config, TERRAIN_Grass));
        }
    }
}
