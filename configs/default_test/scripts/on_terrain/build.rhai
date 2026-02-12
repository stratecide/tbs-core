
fn replace_with_grass(CONTEXT) {
    CONTEXT.event_handler.replace_terrain(CONTEXT.position, CONTEXT.config.build_terrain(TERRAIN_Grass).build());
}
