use std::path::{Path, PathBuf};
use std::sync::Arc;

use clap::{Parser, Subcommand};
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

/// rpg-mcp: Code graph intelligence for AI coding agents.
#[derive(Parser)]
#[command(name = "rpg-mcp", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Initialize the .rpg/ store without encoding.
    Init {
        /// Path to the workspace root.
        workspace: PathBuf,
    },
    /// Verify the committed graph is up-to-date with source.
    Verify {
        /// Path to the workspace root.
        workspace: PathBuf,
    },
    /// Encode the workspace and persist the graph, without starting the MCP server.
    Encode {
        /// Path to the workspace root.
        workspace: PathBuf,
        /// Enable LLM semantic enrichment (requires OPENAI_* config).
        #[arg(short, long)]
        semantic: bool,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .with_writer(std::io::stderr)
        .with_target(false)
        .init();

    let cli = Cli::parse();

    match cli.command {
        Some(Command::Init { workspace }) => cmd_init(&workspace),
        Some(Command::Verify { workspace }) => cmd_verify(&workspace),
        Some(Command::Encode { workspace, semantic }) => {
            cmd_encode(&workspace, semantic).await
        }
        None => cmd_serve().await,
    }
}

/// `rpg-mcp init <workspace>` — create the .rpg/ store.
fn cmd_init(workspace: &Path) -> anyhow::Result<()> {
    if !workspace.is_dir() {
        anyhow::bail!("'{}' is not a directory", workspace.display());
    }
    let _store = RpgStore::init(workspace)?;
    println!("Initialized .rpg/ store at: {}", workspace.join(".rpg").display());
    println!("  manifest.json: v{}", 1);
    println!("  patches/: created");
    println!("\nNext: run `rpg-mcp encode {}` to build the graph.", workspace.display());
    Ok(())
}

/// `rpg-mcp verify <workspace>` — check if the committed graph is current.
fn cmd_verify(workspace: &Path) -> anyhow::Result<()> {
    if !workspace.is_dir() {
        anyhow::bail!("'{}' is not a directory", workspace.display());
    }

    let data_dir = workspace.join(".rpg");
    let current_hash = compute_dir_hash(workspace, rpg_mcp::state::HashMode::Mtime)?;

    match load_dir_hash(&data_dir) {
        Some(stored_hash) if stored_hash == current_hash => {
            match RpgStore::open(workspace) {
                Ok(store) => match store.load() {
                    Ok(snapshot) => {
                        println!("✓ Graph is up-to-date");
                        println!("  Nodes: {}", snapshot.graph.node_count());
                        println!("  Edges: {}", snapshot.graph.edge_count());
                        Ok(())
                    }
                    Err(e) => {
                        println!("✗ Store exists but failed to load: {}", e);
                        std::process::exit(1);
                    }
                },
                Err(e) => {
                    println!("✗ No store found: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Some(_stored_hash) => {
            println!("✗ Graph is STALE (source files changed since last encode)");
            println!("  Run `rpg-mcp encode {}` to update.", workspace.display());
            std::process::exit(1);
        }
        None => {
            println!("✗ No graph found — run `rpg-mcp encode {}` first.", workspace.display());
            std::process::exit(1);
        }
    }
}

/// `rpg-mcp encode <workspace>` — encode and persist without starting the server.
async fn cmd_encode(workspace: &Path, semantic: bool) -> anyhow::Result<()> {
    if !workspace.is_dir() {
        anyhow::bail!("'{}' is not a directory", workspace.display());
    }

    // Set env vars for McpConfig compatibility.
    std::env::set_var("RPG_WORKSPACE", workspace);
    if semantic {
        std::env::set_var("RPG_SEMANTIC", "true");
    }

    let config = McpConfig::from_env()?;
    let snapshot = encode_workspace(workspace, &config)?;

    // Persist.
    let data_dir = &config.data_dir;
    std::fs::create_dir_all(data_dir).ok();
    if let Ok(mut store) = RpgStore::init(workspace) {
        if let Err(e) = store.save_base(&snapshot) {
            tracing::warn!("Failed to save graph to store: {}", e);
        }
    } else if let Ok(mut store) = RpgStore::open(workspace) {
        if let Err(e) = store.save_base(&snapshot) {
            tracing::warn!("Failed to save graph to store: {}", e);
        }
    }
    if let Err(e) = save_dir_hash(data_dir, &compute_dir_hash(workspace, config.hash_mode)?) {
        tracing::warn!("Failed to save dir hash: {}", e);
    }

    println!(
        "Encoded: {} nodes, {} edges, {} files",
        snapshot.graph.node_count(),
        snapshot.graph.edge_count(),
        snapshot.file_hashes.len()
    );
    println!("Graph saved to: {}", workspace.join(".rpg").display());
    Ok(())
}

/// Default: start the MCP server (stdio transport).
async fn cmd_serve() -> anyhow::Result<()> {
    let config = McpConfig::from_env()?;
    let workspace = &config.workspace;
    let registry = Arc::new(create_parser_registry()?);

    let snapshot = match load_existing_store(workspace, &config) {
        Some(s) => {
            info!("Loaded existing snapshot from store");
            s
        }
        None => {
            let snapshot = encode_workspace(workspace, &config)?;

            std::fs::create_dir_all(&config.data_dir).ok();
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

/// Encode the workspace, optionally with semantic enrichment.
fn encode_workspace(workspace: &Path, config: &McpConfig) -> anyhow::Result<RpgSnapshot> {
    let mut encoder = RpgEncoder::new()?;

    if config.semantic {
        info!("Encoding fresh with LLM semantic enrichment");
        let semantic_config =
            rpg_encoder::SemanticConfig::new(rpg_encoder::LlmConfig::from_env()?)
                .with_scope(rpg_encoder::ExtractionScope::File);
        // Block on the async encode — this is a sync function called from
        // either cmd_serve (async) or cmd_encode (async).
        let result = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current()
                .block_on(encoder.encode_with_semantics(workspace, semantic_config))
        })?;
        let mut snapshot = RpgSnapshot::new("repo", workspace);
        snapshot.graph = result.graph;
        Ok(snapshot)
    } else {
        info!("No existing snapshot found, encoding fresh");
        let result = encoder.encode(workspace)?;
        let mut snapshot = RpgSnapshot::new("repo", workspace);
        snapshot.graph = result.graph;
        Ok(snapshot)
    }
}

/// Try to load an existing store if the dir-hash matches.
fn load_existing_store(workspace: &Path, config: &McpConfig) -> Option<RpgSnapshot> {
    let current_hash = compute_dir_hash(workspace, config.hash_mode).ok()?;

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
