//! Plexus CLI — knowledge graph engine with MCP server.
//!
//! Usage:
//!   plexus mcp [--transport stdio] [--db path]
//!   plexus context <subcommand> [--db path]

use clap::{Parser, Subcommand};
use plexus::{Context, ContextId, OpenStore, PlexusEngine, Source, SqliteStore};
use plexus::adapter::{
    Adapter, GraphAnalysisAdapter, run_analysis,
    AdapterInput, EngineSink, FrameworkContext,
};
use plexus::llm_orc::SubprocessClient;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

#[derive(Parser)]
#[command(
    name = "plexus",
    version,
    about = "Network-aware knowledge graph engine"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the MCP (Model Context Protocol) server
    Mcp {
        /// Transport type (currently only stdio)
        #[arg(long, default_value = "stdio")]
        transport: String,
        /// Path to SQLite database file
        #[arg(long)]
        db: Option<PathBuf>,
    },
    /// Run on-demand external enrichment on a context via llm-orc (ADR-024)
    Analyze {
        /// Name of the context to analyze
        name: String,
        /// llm-orc ensemble name for external enrichment
        #[arg(long, default_value = "graph-analysis")]
        ensemble: String,
        /// Path to SQLite database file
        #[arg(long)]
        db: Option<PathBuf>,
    },
    /// Manage contexts
    Context {
        #[command(subcommand)]
        action: ContextAction,
        /// Path to SQLite database file
        #[arg(long, global = true)]
        db: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum ContextAction {
    /// Create a new context
    Create {
        /// Name for the new context
        name: String,
    },
    /// Delete a context by name
    Delete {
        /// Name of the context to delete
        name: String,
    },
    /// List all contexts
    List,
    /// Rename a context
    Rename {
        /// Current context name
        old: String,
        /// New context name
        new: String,
    },
    /// Add a source to a context
    AddSource {
        /// Name of the context
        name: String,
        /// Path to file or directory to add
        #[arg(required = true)]
        path: PathBuf,
    },
    /// Remove a source from a context
    RemoveSource {
        /// Name of the context
        name: String,
        /// Path to file or directory to remove
        #[arg(required = true)]
        path: PathBuf,
    },
}

/// Get the default database path (~/.local/share/plexus/plexus.db)
fn default_db_path() -> PathBuf {
    let data_dir = dirs::data_dir()
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_default().join(".local/share"));
    let plexus_dir = data_dir.join("plexus");
    std::fs::create_dir_all(&plexus_dir).ok();
    plexus_dir.join("plexus.db")
}

fn open_engine(db: Option<PathBuf>) -> Result<PlexusEngine, String> {
    let db_path = db.unwrap_or_else(default_db_path);
    let store = SqliteStore::open(&db_path).map_err(|e| format!("Failed to open database: {}", e))?;
    let engine = PlexusEngine::with_store(Arc::new(store));
    engine.load_all().map_err(|e| format!("Failed to load contexts: {}", e))?;
    Ok(engine)
}

/// Find a context by name, returning its ID
fn find_context_by_name(engine: &PlexusEngine, name: &str) -> Option<ContextId> {
    engine.list_contexts().into_iter().find(|id| {
        engine.get_context(id).map(|c| c.name == name).unwrap_or(false)
    })
}

fn cmd_context_create(engine: &PlexusEngine, name: &str) -> i32 {
    if find_context_by_name(engine, name).is_some() {
        eprintln!("Error: context '{}' already exists", name);
        return 1;
    }
    let context = Context::new(name);
    match engine.upsert_context(context) {
        Ok(id) => {
            println!("Created context '{}' ({})", name, id);
            0
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            1
        }
    }
}

fn cmd_context_delete(engine: &PlexusEngine, name: &str) -> i32 {
    let id = match find_context_by_name(engine, name) {
        Some(id) => id,
        None => {
            eprintln!("Error: context '{}' not found", name);
            return 1;
        }
    };
    match engine.delete_context(&id) {
        Ok(true) => {
            println!("Deleted context '{}'", name);
            0
        }
        Ok(false) => {
            eprintln!("Error: context '{}' not found", name);
            1
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            1
        }
    }
}

fn cmd_context_list(engine: &PlexusEngine) -> i32 {
    let ids = engine.list_contexts();
    if ids.is_empty() {
        println!("No contexts defined.");
        return 0;
    }
    println!("{:<36}  {:<24}  {:>7}", "ID", "NAME", "SOURCES");
    println!("{}", "-".repeat(72));
    for id in ids {
        if let Some(ctx) = engine.get_context(&id) {
            println!(
                "{:<36}  {:<24}  {:>7}",
                id,
                ctx.name,
                ctx.metadata.sources.len()
            );
        }
    }
    0
}

fn cmd_context_rename(engine: &PlexusEngine, old: &str, new: &str) -> i32 {
    let id = match find_context_by_name(engine, old) {
        Some(id) => id,
        None => {
            eprintln!("Error: context '{}' not found", old);
            return 1;
        }
    };
    if find_context_by_name(engine, new).is_some() {
        eprintln!("Error: context '{}' already exists", new);
        return 1;
    }
    match engine.rename_context(&id, new) {
        Ok(()) => {
            println!("Renamed context '{}' to '{}'", old, new);
            0
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            1
        }
    }
}

fn cmd_context_add_source(engine: &PlexusEngine, name: &str, path: &PathBuf) -> i32 {
    let id = match find_context_by_name(engine, name) {
        Some(id) => id,
        None => {
            eprintln!("Error: context '{}' not found", name);
            return 1;
        }
    };
    let canonical = match path.canonicalize() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Error: cannot resolve '{}': {}", path.display(), e);
            return 1;
        }
    };
    let path_str = canonical.to_string_lossy().to_string();
    let source = if canonical.is_dir() {
        Source::Directory { path: path_str, recursive: false }
    } else {
        Source::File { path: path_str }
    };
    match engine.add_source(&id, source) {
        Ok(()) => {
            println!("Added source to context '{}'", name);
            0
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            1
        }
    }
}

fn cmd_context_remove_source(engine: &PlexusEngine, name: &str, path: &PathBuf) -> i32 {
    let id = match find_context_by_name(engine, name) {
        Some(id) => id,
        None => {
            eprintln!("Error: context '{}' not found", name);
            return 1;
        }
    };
    let canonical = match path.canonicalize() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Error: cannot resolve '{}': {}", path.display(), e);
            return 1;
        }
    };
    let path_str = canonical.to_string_lossy().to_string();
    // Try both file and directory variants
    let file_source = Source::File { path: path_str.clone() };
    let dir_source = Source::Directory { path: path_str, recursive: false };

    match engine.remove_source(&id, &file_source) {
        Ok(true) => {
            println!("Removed source from context '{}'", name);
            return 0;
        }
        Ok(false) => {}
        Err(e) => {
            eprintln!("Error: {}", e);
            return 1;
        }
    }
    match engine.remove_source(&id, &dir_source) {
        Ok(true) => {
            println!("Removed source from context '{}'", name);
            0
        }
        Ok(false) => {
            eprintln!("Warning: source not found in context '{}'", name);
            1
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            1
        }
    }
}

async fn cmd_analyze(engine: &PlexusEngine, context_name: &str, ensemble: &str) -> i32 {
    let ctx_id = match find_context_by_name(engine, context_name) {
        Some(id) => id,
        None => {
            eprintln!("Error: context '{}' not found", context_name);
            return 1;
        }
    };

    let ctx = match engine.get_context(&ctx_id) {
        Some(c) => c,
        None => {
            eprintln!("Error: context '{}' not found", context_name);
            return 1;
        }
    };

    println!(
        "Analyzing context '{}' ({} nodes, {} edges)...",
        context_name,
        ctx.node_count(),
        ctx.edge_count()
    );

    let project_dir = std::env::current_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    let client = SubprocessClient::new().with_project_dir(project_dir);

    let results = match run_analysis(&client, ensemble, &ctx).await {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Error: {}", e);
            return 1;
        }
    };

    if results.is_empty() {
        println!("No analysis results returned.");
        return 0;
    }

    // Apply each algorithm's results via its adapter
    let shared_ctx = Arc::new(Mutex::new(ctx.clone()));
    let mut total_updates = 0;

    for (algo_name, input) in &results {
        let adapter = GraphAnalysisAdapter::new(algo_name.as_str());
        let sink = EngineSink::new(shared_ctx.clone()).with_framework_context(FrameworkContext {
            adapter_id: adapter.id().to_string(),
            context_id: ctx_id.to_string(),
            input_summary: Some(format!("analysis:{}", algo_name)),
        });

        let adapter_input = AdapterInput::new(adapter.input_kind(), input.clone(), &ctx_id.to_string());
        match adapter.process(&adapter_input, &sink).await {
            Ok(()) => {
                println!("  {} — {} node updates", algo_name, input.results.len());
                total_updates += input.results.len();
            }
            Err(e) => {
                eprintln!("  {} — failed: {}", algo_name, e);
            }
        }
    }

    // Save the updated context back to the engine
    let updated_ctx = shared_ctx.lock().unwrap().clone();
    if let Err(e) = engine.upsert_context(updated_ctx) {
        eprintln!("Error saving context: {}", e);
        return 1;
    }

    println!("Done. {} property updates applied.", total_updates);
    0
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Commands::Mcp { transport, db } => {
            if transport != "stdio" {
                eprintln!("error: only 'stdio' transport is currently supported");
                std::process::exit(1);
            }
            let db_path = db.unwrap_or_else(default_db_path);
            let code = plexus::mcp::run_mcp_server(db_path);
            std::process::exit(code);
        }
        Commands::Analyze { name, ensemble, db } => {
            let engine = match open_engine(db) {
                Ok(e) => e,
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            };
            let code = tokio::runtime::Runtime::new()
                .unwrap()
                .block_on(cmd_analyze(&engine, &name, &ensemble));
            std::process::exit(code);
        }
        Commands::Context { action, db } => {
            let engine = match open_engine(db) {
                Ok(e) => e,
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            };
            let code = match action {
                ContextAction::Create { name } => cmd_context_create(&engine, &name),
                ContextAction::Delete { name } => cmd_context_delete(&engine, &name),
                ContextAction::List => cmd_context_list(&engine),
                ContextAction::Rename { old, new } => cmd_context_rename(&engine, &old, &new),
                ContextAction::AddSource { name, path } => cmd_context_add_source(&engine, &name, &path),
                ContextAction::RemoveSource { name, path } => cmd_context_remove_source(&engine, &name, &path),
            };
            std::process::exit(code);
        }
    }
}
