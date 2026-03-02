use std::os::raw::{c_int, c_void};

#[no_mangle]
pub extern "C" fn add_numbers(a: c_int, b: c_int) -> c_int {
    a + b
}

#[no_mangle]
pub extern "C" fn process_data(data: *const c_void, len: usize) -> c_int {
    if data.is_null() {
        return -1;
    }
    0
}

extern "C" {
    fn external_function(x: c_int) -> c_int;

    fn another_external(ptr: *const c_void) -> c_int;
}

pub fn call_external() -> c_int {
    unsafe { external_function(42) }
}
