//! Chunked JSON storage with global patch layer.
//!
//! Provides `RpgStore` for persisting RPG graphs as a base JSON file
//! with append-only patch files. Supports automatic compaction and
//! optional zstd compression.
//!
//! # Directory Layout
//!
//! ```text
//! .rpg/
//! ├── manifest.json
//! ├── base.json
//! └── patches/
//!     ├── 001.json
//!     └── 002.json
//! ```
//!
//! # Example
//!
//! ```ignore
//! use rpg_encoder::storage::RpgStore;
//! use rpg_encoder::RpgSnapshot;
//! use std::path::Path;
//!
//! // Initialize
//! let mut store = RpgStore::init(Path::new("./my-project"))?;
//!
//! // Save base
//! store.save_base(&snapshot)?;
//!
//! // Write patch
//! store.write_patch(&patch)?;
//!
//! // Load (base + all patches)
//! let loaded = store.load()?;
//!
//! // Compact when needed
//! if store.should_compact() {
//!     store.compact()?;
//! }
//! ```

mod base;
mod manifest;
mod patch;
mod store;

pub use base::BaseSnapshot;
pub use manifest::{BaseInfo, CompactionThreshold, FileEntry, Manifest, PatchInfo};
pub use patch::{FilePatch, Patch, PatchChanges, PatchStats, RemovedEdge};
pub use store::RpgStore;
