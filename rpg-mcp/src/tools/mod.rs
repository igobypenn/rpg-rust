//! Tool modules for rpg-mcp.
//!
//! - `format`: shared response-shaping helpers (node/edge → JSON, the
//!   detail_level ladder, source-line reading).
//! - `telemetry`: optional JSONL tool-call logging (enable via
//!   `RPG_TELEMETRY_FILE`).
//! - `memory`: agent memory (cross-session notes tied to nodes/files).
//! - `diff`: minimal unified-diff parser for change analysis.

pub mod diff;
pub mod format;
pub mod memory;
pub mod telemetry;
