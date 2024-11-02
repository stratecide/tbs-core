
fn buy_airport_input() {
    terrain_buy_unit_input(POSITION, TERRAIN_Airport, TOKEN.owner_id, ())
}

fn buy_airport(data) {
    terrain_buy_unit(POSITION, TOKEN.owner_id, data, false);
    EVENT_HANDLER.remove_token(POSITION, TOKEN);
}

fn buy_factory_input() {
    terrain_buy_unit_input(POSITION, TERRAIN_Factory, TOKEN.owner_id, ())
}

fn buy_factory(data) {
    terrain_buy_unit(POSITION, TOKEN.owner_id, data, false);
    EVENT_HANDLER.remove_token(POSITION, TOKEN);
}

fn buy_port_input() {
    terrain_buy_unit_input(POSITION, TERRAIN_Port, TOKEN.owner_id, ())
}

fn buy_port(data) {
    terrain_buy_unit(POSITION, TOKEN.owner_id, data, false);
    EVENT_HANDLER.remove_token(POSITION, TOKEN);
}
