//! Shared utility functions for the RPG encoder.

pub mod case;
pub mod similarity;

pub use case::{to_pascal_case, to_title_case};
pub use similarity::*;
