#[no_mangle]
pub extern "C" fn algoram_checked_triple(value: i32) -> i32 {
    if value < 0 {
        -1
    } else {
        value * 3
    }
}
