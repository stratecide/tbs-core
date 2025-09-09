
fn set_moved() {
    let arguments = [];
    let defender_ids = [];
    for target in TARGETS {
        // check if there's a unit at target position
        let p = target.point;
        let defender_id = ATTACK_CONTEXT.remember_unit(p);
        if defender_id == () {
            continue;
        }
        // check if that unit has the Unmoved flag set
        let defender = BOARD.get_unit(p);
        if defender.has(FLAG_Unmoved) {
            ATTACK_CONTEXT.add_script(
                Script("remove_flag", [defender_id, FLAG_Unmoved])
            );
        }
    }
}

fn remove_flag(defender_id, flag) {
    EVENT_HANDLER.remove(defender_id, flag);
}
