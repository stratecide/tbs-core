
fn uf_can_build(CONTEXT) {
    unit_can_build(CONTEXT.board, CONTEXT.unit, CONTEXT.position)
}

fn ua_exhausted_input(CONTEXT) {
    build_unit_input(CONTEXT.board, CONTEXT.player, CONTEXT.unit, CONTEXT.position)
}

fn ua_exhausted(CONTEXT, data) {
    build_unit(CONTEXT.event_handler, CONTEXT.unit, CONTEXT.position, true, data)
}
