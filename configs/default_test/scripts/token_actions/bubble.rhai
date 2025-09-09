
fn buy_airport_input() {
    terrain_buy_unit_input(BOARD, PLAYER, POSITION, TERRAIN_Airport, TOKEN.owner_id, ())
}

fn buy_airport(data) {
    EVENT_HANDLER.remove_token(POSITION, TOKEN);
    terrain_buy_unit(EVENT_HANDLER, POSITION, TOKEN.owner_id, data, false);
}

fn buy_factory_input() {
    terrain_buy_unit_input(BOARD, PLAYER, POSITION, TERRAIN_Factory, TOKEN.owner_id, ())
}

fn buy_factory(data) {
    EVENT_HANDLER.remove_token(POSITION, TOKEN);
    terrain_buy_unit(EVENT_HANDLER, POSITION, TOKEN.owner_id, data, false);
}

fn buy_port_input() {
    terrain_buy_unit_input(BOARD, PLAYER, POSITION, TERRAIN_Port, TOKEN.owner_id, ())
}

fn buy_port(data) {
    EVENT_HANDLER.remove_token(POSITION, TOKEN);
    terrain_buy_unit(EVENT_HANDLER, POSITION, TOKEN.owner_id, data, false);
}
