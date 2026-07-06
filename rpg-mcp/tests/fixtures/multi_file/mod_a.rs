//! Multi-file fixture: module A declares module B and calls B's function.
//! Used to test cross-file Imports/References edges, multi-file skeleton,
//! and get_impact affected_files spanning files.

pub mod b;

pub fn caller_in_a() -> u64 {
    b::helper_in_b()
}

pub fn another_caller() -> bool {
    caller_in_a() > 0
}
