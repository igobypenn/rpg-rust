//! Core math library with FFI exports
//!
//! This module provides mathematical operations that can be called
//! from other languages via FFI.

use std::os::raw::c_int;

/// Adds two integers and returns the result.
///
/// # Safety
/// This function is safe to call from any language that supports
/// the C calling convention.
#[no_mangle]
pub extern "C" fn rpg_add(a: c_int, b: c_int) -> c_int {
    a + b
}

/// Multiplies two integers and returns the result.
///
/// # Safety
/// This function is safe to call from any language that supports
/// the C calling convention.
#[no_mangle]
pub extern "C" fn rpg_multiply(a: c_int, b: c_int) -> c_int {
    a * b
}

/// Subtracts b from a and returns the result.
#[no_mangle]
pub extern "C" fn rpg_subtract(a: c_int, b: c_int) -> c_int {
    a - b
}

/// Configuration struct for complex operations.
#[repr(C)]
pub struct Config {
    pub precision: c_int,
    pub rounding_mode: c_int,
}

/// Performs a complex calculation with configuration.
///
/// # Arguments
/// * `value` - The input value to process
/// * `config` - Pointer to a Config struct
///
/// # Returns
/// The processed result as an integer
#[no_mangle]
pub extern "C" fn rpg_process(value: c_int, config: *const Config) -> c_int {
    if config.is_null() {
        return value;
    }
    let cfg = unsafe { &*config };
    match cfg.rounding_mode {
        0 => value * cfg.precision,
        1 => (value as f64 * cfg.precision as f64) as c_int,
        _ => value,
    }
}

/// String processing result.
#[repr(C)]
pub struct StringResult {
    pub data: *mut u8,
    pub len: usize,
    pub capacity: usize,
}

/// Creates a greeting string.
///
/// # Safety
/// The returned StringResult must be freed with rpg_free_string.
#[no_mangle]
pub extern "C" fn rpg_greet(name: *const i8) -> StringResult {
    let name_str = unsafe {
        if name.is_null() {
            "World"
        } else {
            std::ffi::CStr::from_ptr(name).to_str().unwrap_or("World")
        }
    };

    let greeting = format!("Hello, {}!", name_str);
    let bytes = greeting.into_bytes();
    let len = bytes.len();
    let capacity = bytes.capacity();
    let data = bytes.leak().as_mut_ptr();

    StringResult {
        data,
        len,
        capacity,
    }
}

/// Frees a string result.
///
/// # Safety
/// Must only be called with a valid StringResult from rpg_greet.
#[no_mangle]
pub extern "C" fn rpg_free_string(result: StringResult) {
    if !result.data.is_null() && result.capacity > 0 {
        unsafe {
            let _ = Vec::from_raw_parts(result.data, result.len, result.capacity);
        }
    }
}

/// Internal Rust-only function for validation.
fn validate_input(value: i32) -> bool {
    value >= 0 && value <= 10000
}

/// Validates and processes a value.
pub fn safe_process(value: i32, precision: i32) -> Option<i32> {
    if validate_input(value) {
        Some(value * precision)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add() {
        assert_eq!(rpg_add(2, 3), 5);
    }

    #[test]
    fn test_multiply() {
        assert_eq!(rpg_multiply(4, 5), 20);
    }

    #[test]
    fn test_safe_process() {
        assert_eq!(safe_process(50, 2), Some(100));
        assert_eq!(safe_process(20000, 2), None);
    }
}
