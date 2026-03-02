//! RPG Test Repository
//!
//! A synthetic multi-language repository for testing FFI bindings
//! and semantic analysis features.

pub mod math;

/// Application version
pub const VERSION: &str = "0.1.0";

/// Application name
pub const NAME: &str = "rpg-test-repo";

/// Initializes the library.
///
/// This should be called before any other operations.
pub fn init() -> bool {
    true
}

/// Shuts down the library.
pub fn shutdown() {
    // Cleanup resources
}
