//! Tool-call telemetry: JSONL log of every tool invocation.
//!
//! When `RPG_TELEMETRY_FILE` is set, each tool call appends a JSON line with
//! the tool name, params (keys only — values are not logged to avoid leaking
//! source/code in telemetry), duration, and success/error status.
//!
//! The file is opened once at construction and writes are line-buffered. Each
//! record is a self-contained JSON object on its own line.
//!
//! # Format
//!
//! ```jsonc
//! {"ts":1719900000,"tool":"search_nodes","param_keys":["query","limit"],"duration_ms":3,"ok":true}
//! ```

use std::io::Write;
use std::path::PathBuf;

use parking_lot::Mutex;
use std::time::Instant;

use serde::Serialize;

/// A single telemetry record.
#[derive(Serialize)]
struct Record {
    ts: u64,
    tool: &'static str,
    param_keys: Vec<String>,
    duration_ms: u64,
    ok: bool,
}

/// Optional telemetry writer. When `None` (the default), all calls are no-ops.
/// When `Some`, writes one JSON line per tool invocation.
pub struct Telemetry {
    file: Mutex<Option<std::fs::File>>,
}

impl std::fmt::Debug for Telemetry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Telemetry").finish_non_exhaustive()
    }
}

impl Telemetry {
    /// Create a disabled telemetry (no logging). The default.
    pub fn disabled() -> Self {
        Self {
            file: Mutex::new(None),
        }
    }

    /// Create from env var `RPG_TELEMETRY_FILE`. If unset, returns disabled.
    pub fn from_env() -> Self {
        match std::env::var("RPG_TELEMETRY_FILE").ok().filter(|s| !s.is_empty()) {
            Some(path) => Self::to_file(PathBuf::from(path)),
            None => Self::disabled(),
        }
    }

    /// Enable telemetry, writing JSONL to `path`. Creates or appends.
    pub fn to_file(path: PathBuf) -> Self {
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|e| {
                tracing::warn!("Failed to open telemetry file {}: {}", path.display(), e);
                e
            })
            .ok();
        Self {
            file: Mutex::new(file),
        }
    }

    /// Log a tool call. Records param keys (not values), duration, and status.
    /// Best-effort: if the file isn't open or the write fails, this is a silent
    /// no-op — telemetry must never break a tool call.
    pub fn log(&self, tool: &'static str, params: &serde_json::Map<String, serde_json::Value>, duration: std::time::Duration, ok: bool) {
        let mut guard = self.file.lock();
        let Some(ref mut file) = *guard else {
            return; // disabled
        };

        let record = Record {
            ts: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
            tool,
            param_keys: params.keys().cloned().collect(),
            duration_ms: duration.as_millis() as u64,
            ok,
        };

        // serialize + append newline; ignore errors (telemetry is best-effort)
        if let Ok(line) = serde_json::to_string(&record) {
            let _ = writeln!(file, "{line}");
            let _ = file.flush();
        }
    }
}

/// Helper to time a tool call and log it.
///
/// ```ignore
/// let _guard = telemetry::timed(&self.telemetry, "search_nodes", &params);
/// // ... tool body ...
/// // guard logs on drop
/// ```
pub fn timed(
    telemetry: &std::sync::Arc<Telemetry>,
    tool: &'static str,
    params: &serde_json::Map<String, serde_json::Value>,
) -> TimingGuard {
    TimingGuard {
        telemetry: std::sync::Arc::clone(telemetry),
        tool,
        param_keys: params.keys().cloned().collect(),
        start: Instant::now(),
        ok: std::sync::atomic::AtomicBool::new(true),
    }
}

/// RAII guard that logs the tool call when dropped.
pub struct TimingGuard {
    telemetry: std::sync::Arc<Telemetry>,
    tool: &'static str,
    param_keys: Vec<String>,
    start: Instant,
    ok: std::sync::atomic::AtomicBool,
}

impl TimingGuard {
    /// Mark the tool call as failed (e.g. it returned an Err).
    pub fn fail(&self) {
        self.ok.store(false, std::sync::atomic::Ordering::Relaxed);
    }
}

impl Drop for TimingGuard {
    fn drop(&mut self) {
        // Reconstruct a minimal param map for logging (we only need keys, but
        // the Telemetry::log signature takes the full map — pass an empty one
        // since we already captured the keys). Simpler: inline the log here.
        let ok = self.ok.load(std::sync::atomic::Ordering::Relaxed);
        let record = Record {
            ts: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
            tool: self.tool,
            param_keys: std::mem::take(&mut self.param_keys),
            duration_ms: self.start.elapsed().as_millis() as u64,
            ok,
        };
        let mut guard = self.telemetry.file.lock();
        if let Some(ref mut file) = *guard {
            if let Ok(line) = serde_json::to_string(&record) {
                let _ = writeln!(file, "{line}");
                let _ = file.flush();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use serde_json::json;

    #[test]
    fn disabled_telemetry_is_noop() {
        let t = Telemetry::disabled();
        let params = serde_json::from_value(json!({"query": "x"})).unwrap();
        t.log("test", &params, std::time::Duration::from_millis(5), true);
        // No panic, no file — that's the assertion.
    }

    #[test]
    fn writes_jsonl_to_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("telemetry.jsonl");
        let t = Telemetry::to_file(path.clone());

        let params = serde_json::from_value(json!({"query": "foo", "limit": 10})).unwrap();
        t.log("search_nodes", &params, std::time::Duration::from_millis(3), true);

        let params2 = serde_json::from_value(json!({"file": "src/lib.rs"})).unwrap();
        t.log("get_source", &params2, std::time::Duration::from_millis(1), false);

        // Drop to flush.
        drop(t);

        let contents = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = contents.lines().filter(|l| !l.is_empty()).collect();
        assert_eq!(lines.len(), 2);

        // First record.
        let r1: HashMap<String, serde_json::Value> = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(r1["tool"], "search_nodes");
        assert_eq!(r1["ok"], true);
        assert_eq!(r1["duration_ms"], 3);
        // Param keys are logged, not values.
        let keys = r1["param_keys"].as_array().unwrap();
        let key_strs: Vec<&str> = keys.iter().map(|v| v.as_str().unwrap()).collect();
        assert!(key_strs.contains(&"query"));
        assert!(key_strs.contains(&"limit"));

        // Second record (failed).
        let r2: HashMap<String, serde_json::Value> = serde_json::from_str(lines[1]).unwrap();
        assert_eq!(r2["tool"], "get_source");
        assert_eq!(r2["ok"], false);
    }

    #[test]
    fn timing_guard_logs_on_drop() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("guard.jsonl");
        let telemetry = std::sync::Arc::new(Telemetry::to_file(path.clone()));
        let params = serde_json::from_value(json!({"q": 1})).unwrap();

        {
            let _guard = timed(&telemetry, "test_tool", &params);
            std::thread::sleep(std::time::Duration::from_millis(2));
        }

        let contents = std::fs::read_to_string(&path).unwrap();
        assert!(contents.contains("test_tool"));
        assert!(contents.contains("\"ok\":true"));
    }
}
