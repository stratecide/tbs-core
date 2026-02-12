
// on unit death
fn put_sludge(CONTEXT) {
    _put_sludge(CONTEXT, CONTEXT.position);
}

fn _put_sludge(CONTEXT, position) {
    CONTEXT.event_handler.place_token(position, Token(CONTEXT.config, TOKEN_Sludge));
}

// on unit death
fn spread_sludge(CONTEXT) {
    _spread_sludge(CONTEXT, [CONTEXT.position])
}

fn _spread_sludge(CONTEXT, positions) {
    // TODO: use set to avoid duplicate positions in 'targets'
    let targets = [];
    for origin in positions {
        let sludge_exists = for t in CONTEXT.event_handler.board.get_tokens(origin) {
            if t.type == TOKEN_Sludge {
                // there's already a sludge token here, spread to surrounding tiles
                break true;
                return;
            }
        };
        if sludge_exists == true {
            targets.append(CONTEXT.board.get_neighbors(origin))
        } else {
            targets.push(origin);
        }
    }
    for p in targets {
        _put_sludge(CONTEXT, p);
    }
}

fn co_noxious_spread(CONTEXT) {
    let targets = [];
    for p in CONTEXT.board.all_positions() {
        let unit = CONTEXT.board.get_unit(p);
        if unit?.owner_id == CONTEXT.owner_id {
            targets.push(p);
        }
    }
    _spread_sludge(CONTEXT, targets);
}

fn uf_can_clean_sludge(CONTEXT) {
    // any unit that can capture can clean sludge
    CONTEXT.config.table_entry("TERRAIN_CAPTURE", CONTEXT.unit.type, TERRAIN_Port)
}

fn ua_clean_sludge(CONTEXT) {
    CONTEXT.event_handler.remove_token(CONTEXT.position, TOKEN_Sludge, -1);
    CONTEXT.event_handler.set_unit_flag(CONTEXT.position, FLAG_Exhausted);
}
