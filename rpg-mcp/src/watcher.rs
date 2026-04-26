use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use notify::{RecursiveMode, Watcher};
use rpg_encoder::{generate_diff, ParserRegistry, RpgEvolution, RpgSnapshot, RpgStore};
use tokio::sync::mpsc;

use crate::state::{compute_dir_hash, save_dir_hash, AppState};

const DEBOUNCE_MS: u64 = 2000;

pub struct FileWatcher {
    _shutdown_tx: mpsc::Sender<()>,
}

impl Drop for FileWatcher {
    fn drop(&mut self) {
        tracing::info!("File watcher dropped");
    }
}

impl FileWatcher {
    pub fn start(
        app_state: Arc<AppState>,
        parser_registry: Arc<ParserRegistry>,
    ) -> anyhow::Result<Self> {
        let workspace = app_state.config.workspace.clone();
        let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);
        let (event_tx, mut event_rx) = mpsc::channel::<()>(256);

        let mut watcher =
            notify::recommended_watcher(move |_res: Result<notify::Event, notify::Error>| {
                let _ = event_tx.blocking_send(());
            })?;

        watcher.watch(&workspace, RecursiveMode::Recursive)?;

        let state = app_state.clone();
        let registry = parser_registry.clone();

        tokio::spawn(async move {
            let mut debounce = tokio::time::interval(Duration::from_millis(DEBOUNCE_MS));
            debounce.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            debounce.tick().await;

            loop {
                tokio::select! {
                    _ = event_rx.recv() => {
                        debounce.reset();
                    }
                    _ = debounce.tick() => {
                        let ws = workspace.clone();
                        let st = state.clone();
                        let reg = registry.clone();
                        if let Err(e) = tokio::spawn(async move {
                            process_changes(&ws, &st, &reg).await
                        }).await.expect("spawn failed") {
                            tracing::error!("Failed to process file changes: {}", e);
                        }
                    }
                    Some(_) = shutdown_rx.recv() => {
                        tracing::info!("File watcher shutting down");
                        break;
                    }
                }
            }

            drop(watcher);
        });

        Ok(Self {
            _shutdown_tx: shutdown_tx,
        })
    }
}

async fn process_changes(
    workspace: &Path,
    app_state: &AppState,
    registry: &ParserRegistry,
) -> anyhow::Result<()> {
    let diff = {
        let snapshot = app_state.snapshot.read().expect("snapshot lock poisoned");
        generate_diff(&snapshot, workspace, registry)?
    };

    if diff.is_empty() {
        return Ok(());
    }

    tracing::info!(
        added = diff.added.len(),
        deleted = diff.deleted.len(),
        modified = diff.modified.len(),
        "Processing incremental update"
    );

    let (new_snapshot, summary) = {
        let mut snapshot = app_state.snapshot.write().expect("snapshot lock poisoned");
        let mut evolution = RpgEvolution::new(&mut snapshot, registry);
        let summary = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(evolution.process_diff(diff, None))
        })?;
        let new_snapshot = snapshot.clone();
        (new_snapshot, summary)
    };

    tracing::info!(
        nodes_created = summary.nodes_created,
        nodes_removed = summary.nodes_removed,
        "Evolution complete"
    );

    persist_snapshot(workspace, app_state, &new_snapshot)?;
    app_state.update(new_snapshot);

    Ok(())
}

fn persist_snapshot(
    workspace: &Path,
    app_state: &AppState,
    snapshot: &RpgSnapshot,
) -> anyhow::Result<()> {
    let data_dir = &app_state.config.data_dir;
    let mut store_guard = app_state.store.write().expect("store lock poisoned");

    let store = if store_guard.is_none() {
        let s = match RpgStore::open(workspace) {
            Ok(s) => s,
            Err(_) => {
                tracing::info!("Initializing new RPG store");
                RpgStore::init(workspace)?
            }
        };
        *store_guard = Some(s);
        store_guard.as_mut().unwrap()
    } else {
        store_guard.as_mut().unwrap()
    };

    store.save_base(snapshot)?;

    if let Err(e) = save_dir_hash(
        data_dir,
        &compute_dir_hash(workspace, app_state.config.hash_mode)?,
    ) {
        tracing::warn!("Failed to save dir hash: {}", e);
    }

    Ok(())
}
