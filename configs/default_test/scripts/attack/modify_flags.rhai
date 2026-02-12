
fn set_moved(CONTEXT) {
    let arguments = [];
    let defender_ids = [];
    for target in CONTEXT.targets {
        // check if there's a unit at target position
        let p = target.point;
        let defender_id = CONTEXT.attack.remember_unit(p);
        if defender_id == () {
            continue;
        }
        // check if that unit has the Unmoved flag set
        let defender = CONTEXT.board.get_unit(p);
        if defender.has(FLAG_Unmoved) {
            CONTEXT.attack.add_script(
                Script("remove_flag", [defender_id, FLAG_Unmoved])
            );
        }
    }
}

fn remove_flag(CONTEXT, defender_id, flag) {
    CONTEXT.event_handler.remove(defender_id, flag);
}
