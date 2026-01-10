
// on unit death
fn put_sludge() {
    _put_sludge(EVENT_HANDLER, POSITION);
}

fn _put_sludge(EVENT_HANDLER, POSITION) {
    EVENT_HANDLER.place_token(POSITION, Token(EVENT_HANDLER.config, TOKEN_Sludge));
}

// on unit death
fn spread_sludge() {
    _spread_sludge(EVENT_HANDLER, [POSITION])
}

fn _spread_sludge(EVENT_HANDLER, positions) {
    // TODO: use set to avoid duplicate positions in 'targets'
    let targets = [];
    for origin in positions {
        let sludge_exists = for t in EVENT_HANDLER.board.get_tokens(origin) {
            if t.type == TOKEN_Sludge {
                // there's already a sludge token here, spread to surrounding tiles
                break true;
                return;
            }
        };
        if sludge_exists == true {
            targets.append(EVENT_HANDLER.board.get_neighbors(origin))
        } else {
            targets.push(origin);
        }
    }
    for p in targets {
        _put_sludge(EVENT_HANDLER, p);
    }
}

fn co_noxious_spread() {
    let targets = [];
    for p in BOARD.all_positions() {
        let unit = BOARD.get_unit(p);
        if unit?.owner_id == OWNER_ID {
            targets.push(p);
        }
    }
    _spread_sludge(EVENT_HANDLER, targets);
}

fn uf_can_clean_sludge() {
    // any unit that can capture can clean sludge
    CONFIG.table_entry("TERRAIN_CAPTURE", UNIT.type, TERRAIN_Port)
}

fn ua_clean_sludge() {
    EVENT_HANDLER.remove_token(POSITION, TOKEN_Sludge, -1);
    EVENT_HANDLER.set_unit_flag(POSITION, FLAG_Exhausted);
}
