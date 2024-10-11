
fn ua_power_input() {
    if PATH.len == 0 {
        success()
    } else {
        failure()
    }
}

fn ua_power(data) {
    EVENT_HANDLER.set_unit_status(POSITION, status_ready());
}
