
fn buy_airport_input(CONTEXT) {
    terrain_buy_unit_input(CONTEXT.board, CONTEXT.player, TERRAIN_Airport, CONTEXT.token.owner_id, ())
}

fn buy_airport(CONTEXT, data) {
    CONTEXT.event_handler.remove_token(CONTEXT.position, CONTEXT.token);
    terrain_buy_unit(CONTEXT.event_handler, CONTEXT.position, CONTEXT.token.owner_id, data, false);
}

fn buy_factory_input(CONTEXT) {
    terrain_buy_unit_input(CONTEXT.board, CONTEXT.player, TERRAIN_Factory, CONTEXT.token.owner_id, ())
}

fn buy_factory(CONTEXT, data) {
    CONTEXT.event_handler.remove_token(CONTEXT.position, CONTEXT.token);
    terrain_buy_unit(CONTEXT.event_handler, CONTEXT.position, CONTEXT.token.owner_id, data, false);
}

fn buy_port_input(CONTEXT) {
    terrain_buy_unit_input(CONTEXT.board, CONTEXT.player, TERRAIN_Port, CONTEXT.token.owner_id, ())
}

fn buy_port(CONTEXT, data) {
    CONTEXT.event_handler.remove_token(CONTEXT.position, CONTEXT.token);
    terrain_buy_unit(CONTEXT.event_handler, CONTEXT.position, CONTEXT.token.owner_id, data, false);
}
