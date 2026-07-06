//! Agent memory: cross-session notes tied to graph nodes or files.
//!
//! Memories are persisted as a JSONL sidecar (`.rpg/memories.jsonl` by default,
//! or `RPG_MEMORY_FILE` for a personal out-of-repo location). Each line is a
//! self-contained JSON object.
//!
//! When the graph is committed to the repo, memories in `.rpg/memories.jsonl`
//! are shared with the team (good for "this module is deprecated" notes). For
//! personal memories, set `RPG_MEMORY_FILE` to a path outside the repo.

use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;

use parking_lot::Mutex;

use serde::{Deserialize, Serialize};

/// A single memory record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Memory {
    /// Unique id (UUID-like short string).
    pub id: String,
    /// Node this memory is attached to (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_id: Option<u64>,
    /// File path this memory is attached to (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    /// Free-form tags for filtering.
    #[serde(default)]
    pub tags: Vec<String>,
    /// The memory content (free-form text).
    pub content: String,
    /// Creation timestamp (unix seconds).
    pub created_at: u64,
    /// Last update timestamp (unix seconds).
    pub updated_at: u64,
}

/// Append-only JSONL memory store. Loads all records on first access, then
/// appends new memories and rewrites on edit/delete.
pub struct MemoryStore {
    memories: Mutex<HashMap<String, Memory>>,
    path: PathBuf,
}

impl std::fmt::Debug for MemoryStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MemoryStore")
            .field("path", &self.path)
            .field("count", &self.memories.lock().len())
            .finish_non_exhaustive()
    }
}

impl MemoryStore {
    /// Open (or create) a memory store at `path`. Reads existing records.
    pub fn open(path: PathBuf) -> Self {
        let memories = Self::load_file(&path);
        Self {
            memories: Mutex::new(memories),
            path,
        }
    }

    /// Create from env: `RPG_MEMORY_FILE` if set, else `workspace/.rpg/memories.jsonl`.
    pub fn from_env(workspace: &std::path::Path) -> Self {
        let path = match std::env::var("RPG_MEMORY_FILE").ok().filter(|s| !s.is_empty()) {
            Some(p) => PathBuf::from(p),
            None => workspace.join(".rpg").join("memories.jsonl"),
        };
        Self::open(path)
    }

    fn load_file(path: &std::path::Path) -> HashMap<String, Memory> {
        let mut map = HashMap::new();
        if let Ok(contents) = std::fs::read_to_string(path) {
            for line in contents.lines() {
                if line.trim().is_empty() {
                    continue;
                }
                if let Ok(mem) = serde_json::from_str::<Memory>(line) {
                    map.insert(mem.id.clone(), mem);
                }
            }
        }
        map
    }

    /// Persist while holding the memories lock. The caller must already hold
    /// the lock to avoid the drop-then-relock race where a concurrent write
    /// can clobber the tmp file.
    fn persist_locked(&self, guard: &HashMap<String, Memory>) {
        // Ensure parent dir exists.
        if let Some(parent) = self.path.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                tracing::warn!("Failed to create memory dir {}: {}", parent.display(), e);
                return;
            }
        }
        // Unique tmp name (PID + counter) to avoid collisions.
        use std::sync::atomic::{AtomicU64, Ordering};
        static TMP_COUNTER: AtomicU64 = AtomicU64::new(0);
        let tmp = self.path.with_extension(format!(
            "jsonl.tmp.{}.{}",
            std::process::id(),
            TMP_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let write_ok = {
            if let Ok(mut file) = std::fs::File::create(&tmp) {
                let mut all: Vec<&Memory> = guard.values().collect();
                all.sort_by_key(|m| m.created_at);
                for mem in all {
                    if let Ok(line) = serde_json::to_string(mem) {
                        let _ = writeln!(file, "{line}");
                    }
                }
                file.flush().is_ok()
            } else {
                false
            }
        };
        if write_ok {
            if let Err(e) = std::fs::rename(&tmp, &self.path) {
                tracing::warn!("Failed to rename memory file: {}", e);
                let _ = std::fs::remove_file(&tmp);
            }
        } else {
            let _ = std::fs::remove_file(&tmp);
        }
    }

    fn now() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }

    fn next_id(&self) -> String {
        // Unique id: timestamp + PID + counter. The PID disambiguates across
        // processes (two MCP servers sharing the same memories.jsonl).
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let pid = std::process::id();
        format!("mem_{}_{}_{}", Self::now(), pid, n)
    }

    /// Create or update a memory. Returns the memory's id.
    pub fn write(
        &self,
        content: String,
        node_id: Option<u64>,
        file: Option<String>,
        tags: Vec<String>,
    ) -> String {
        let mut guard = self.memories.lock();
        let id = self.next_id();
        let now = Self::now();
        let mem = Memory {
            id: id.clone(),
            node_id,
            file,
            tags,
            content,
            created_at: now,
            updated_at: now,
        };
        guard.insert(id.clone(), mem);
        // Persist WHILE holding the lock — dropping then re-locking allows
        // concurrent writes to clobber the tmp file.
        self.persist_locked(&guard);
        id
    }

    /// Read a memory by id.
    pub fn read(&self, id: &str) -> Option<Memory> {
        self.memories.lock().get(id).cloned()
    }

    /// List memories, optionally filtered by node_id, file, or tag.
    pub fn list(
        &self,
        node_id: Option<u64>,
        file: Option<&str>,
        tag: Option<&str>,
    ) -> Vec<Memory> {
        let guard = self.memories.lock();
        let mut results: Vec<Memory> = guard
            .values()
            .filter(|m| {
                if let Some(nid) = node_id {
                    if m.node_id != Some(nid) {
                        return false;
                    }
                }
                if let Some(f) = file {
                    if m.file.as_deref() != Some(f) {
                        return false;
                    }
                }
                if let Some(t) = tag {
                    if !m.tags.iter().any(|mt| mt == t) {
                        return false;
                    }
                }
                true
            })
            .cloned()
            .collect();
        results.sort_by_key(|m| m.created_at);
        results
    }

    /// Delete a memory by id. Returns true if it existed.
    pub fn delete(&self, id: &str) -> bool {
        let mut guard = self.memories.lock();
        let removed = guard.remove(id).is_some();
        if removed {
            self.persist_locked(&guard);
        }
        removed
    }

    /// Number of stored memories.
    pub fn len(&self) -> usize {
        self.memories.lock().len()
    }

    /// `true` if the store holds no memories.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_read_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let store = MemoryStore::open(dir.path().join("mem.jsonl"));

        let id = store.write(
            "This function is the rate limiter".to_string(),
            Some(42),
            None,
            vec!["perf".to_string()],
        );

        let mem = store.read(&id).unwrap();
        assert_eq!(mem.content, "This function is the rate limiter");
        assert_eq!(mem.node_id, Some(42));
        assert!(mem.tags.contains(&"perf".to_string()));
    }

    #[test]
    fn list_filters_by_node() {
        let dir = tempfile::tempdir().unwrap();
        let store = MemoryStore::open(dir.path().join("mem.jsonl"));
        store.write("node 1 note".to_string(), Some(1), None, vec![]);
        store.write("node 2 note".to_string(), Some(2), None, vec![]);

        let filtered = store.list(Some(1), None, None);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].content, "node 1 note");
    }

    #[test]
    fn list_filters_by_tag() {
        let dir = tempfile::tempdir().unwrap();
        let store = MemoryStore::open(dir.path().join("mem.jsonl"));
        store.write("a".to_string(), None, None, vec!["todo".to_string()]);
        store.write("b".to_string(), None, None, vec!["note".to_string()]);

        let filtered = store.list(None, None, Some("todo"));
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].content, "a");
    }

    #[test]
    fn delete_removes_memory() {
        let dir = tempfile::tempdir().unwrap();
        let store = MemoryStore::open(dir.path().join("mem.jsonl"));
        let id = store.write("temp".to_string(), None, None, vec![]);
        assert_eq!(store.len(), 1);
        assert!(store.delete(&id));
        assert_eq!(store.len(), 0);
        assert!(!store.delete(&id)); // already deleted
    }

    #[test]
    fn persists_across_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mem.jsonl");

        {
            let store = MemoryStore::open(path.clone());
            store.write("persistent".to_string(), Some(5), None, vec![]);
        }

        // Reopen — the memory should survive.
        let store = MemoryStore::open(path);
        let all = store.list(None, None, None);
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].content, "persistent");
        assert_eq!(all[0].node_id, Some(5));
    }

    #[test]
    fn from_env_defaults_to_workspace() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".rpg")).unwrap();
        // Without RPG_MEMORY_FILE set, it uses workspace/.rpg/memories.jsonl.
        std::env::remove_var("RPG_MEMORY_FILE");
        let store = MemoryStore::from_env(dir.path());
        store.write("test".to_string(), None, None, vec![]);
        assert!(dir.path().join(".rpg/memories.jsonl").exists());
    }
}
