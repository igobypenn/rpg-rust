use std::path::Path;
use std::sync::Arc;

use rmcp::ServiceExt;
use rpg_encoder::{ParserRegistry, RpgEncoder, RpgSnapshot, RpgStore};
use tracing::info;

use rpg_mcp::service::RpgService;
use rpg_mcp::state::{compute_dir_hash, load_dir_hash, save_dir_hash, AppState, McpConfig};
use rpg_mcp::watcher::FileWatcher;

fn create_parser_registry() -> anyhow::Result<ParserRegistry> {
    let mut registry = ParserRegistry::new();
    rpg_encoder::register_parser!(registry, rpg_encoder::languages::RustParser);
    rpg_encoder::register_parser!(registry, rpg_encoder::languages::PythonParser);
    rpg_encoder::register_parser!(registry, rpg_encoder::languages::GoParser);
    rpg_encoder::register_parser!(registry, rpg_encoder::languages::CParser);
    rpg_encoder::register_parser!(registry, rpg_encoder::languages::CppParser);
    rpg_encoder::register_parser!(registry, rpg_encoder::languages::JavaScriptParser);
    rpg_encoder::register_parser!(registry, rpg_encoder::languages::TypeScriptParser);
    rpg_encoder::register_parser!(registry, rpg_encoder::languages::JavaParser);
    rpg_encoder::register_parser!(registry, rpg_encoder::languages::RubyParser);
    rpg_encoder::register_parser!(registry, rpg_encoder::languages::LuaParser);
    rpg_encoder::register_parser!(registry, rpg_encoder::languages::SwiftParser);
    rpg_encoder::register_parser!(registry, rpg_encoder::languages::HaskellParser);
    rpg_encoder::register_parser!(registry, rpg_encoder::languages::CSharpParser);
    rpg_encoder::register_parser!(registry, rpg_encoder::languages::ScalaParser);
    Ok(registry)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .with_writer(std::io::stderr)
        .with_target(false)
        .init();

    let config = McpConfig::from_env()?;
    let workspace = &config.workspace;
    let registry = Arc::new(create_parser_registry()?);

    let snapshot = match load_existing_store(workspace, &config) {
        Some(s) => {
            info!("Loaded existing snapshot from store");
            s
        }
        None => {
            let mut encoder = RpgEncoder::new()?;

            let snapshot = if config.semantic {
                info!("Encoding fresh with LLM semantic enrichment");
                let semantic_config =
                    rpg_encoder::SemanticConfig::new(rpg_encoder::LlmConfig::from_env()?)
                        .with_scope(rpg_encoder::ExtractionScope::File);
                let result = encoder
                    .encode_with_semantics(workspace, semantic_config)
                    .await?;
                let mut snapshot = RpgSnapshot::new("repo", workspace);
                snapshot.graph = result.graph;
                snapshot
            } else {
                info!("No existing snapshot found, encoding fresh");
                let result = encoder.encode(workspace)?;
                let mut snapshot = RpgSnapshot::new("repo", workspace);
                snapshot.graph = result.graph;
                snapshot
            };

            std::fs::create_dir_all(&config.data_dir).ok();
            // RpgStore::init/open append ".rpg" to the path internally, so
            // pass the workspace root (not data_dir, which IS ".rpg" already).
            if let Ok(mut store) = RpgStore::init(workspace) {
                if let Err(e) = store.save_base(&snapshot) {
                    tracing::warn!("Failed to save graph to store: {}", e);
                }
            } else if let Ok(mut store) = RpgStore::open(workspace) {
                if let Err(e) = store.save_base(&snapshot) {
                    tracing::warn!("Failed to save graph to store: {}", e);
                }
            }

            if let Err(e) = save_dir_hash(
                &config.data_dir,
                &compute_dir_hash(workspace, config.hash_mode)?,
            ) {
                tracing::warn!("Failed to save dir hash: {}", e);
            }

            info!(
                nodes = snapshot.graph.node_count(),
                edges = snapshot.graph.edge_count(),
                "Encoding complete"
            );

            snapshot
        }
    };

    let app_state = Arc::new(AppState::new(config.clone(), snapshot, registry.clone()));

    let _watcher = FileWatcher::start(app_state.clone(), registry)?;

    let service = RpgService::new(app_state);
    let server = service.serve(rmcp::transport::stdio()).await?;
    server.waiting().await?;

    Ok(())
}

fn load_existing_store(workspace: &Path, config: &McpConfig) -> Option<RpgSnapshot> {
    let current_hash = compute_dir_hash(workspace, config.hash_mode).ok()?;

    // No stored hash → no prior encode, fresh start.
    let stored_hash = match load_dir_hash(&config.data_dir) {
        Some(h) => h,
        None => {
            tracing::info!("No dir_hash found, encoding fresh");
            return None;
        }
    };

    if stored_hash != current_hash {
        tracing::info!("Directory hash mismatch (source changed), re-encoding");
        return None;
    }

    // Hash matches — try to load the store.
    match RpgStore::open(workspace) {
        Ok(store) => match store.load() {
            Ok(snapshot) => {
                tracing::info!(
                    nodes = snapshot.graph.node_count(),
                    edges = snapshot.graph.edge_count(),
                    "Store loaded"
                );
                Some(snapshot)
            }
            Err(e) => {
                tracing::warn!(
                    "Store exists but failed to load (corrupt?): {}. Re-encoding.",
                    e
                );
                None
            }
        },
        Err(e) => {
            tracing::info!(
                "No store found at {}: {}. Encoding fresh.",
                workspace.display(),
                e
            );
            None
        }
    }
}
