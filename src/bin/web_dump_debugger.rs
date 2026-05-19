use clap::Parser;
use mcp_windbg_rs::http::{CleanupTask, HttpServer, ServerConfig};
use std::path::PathBuf;
use tracing::{error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// Web Dump Debugger - HTTP server for crash dump analysis
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to configuration file (TOML format)
    #[arg(short, long)]
    config: Option<PathBuf>,
    
    /// HTTP server port (overrides config file)
    #[arg(short, long)]
    port: Option<u16>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,mcp_windbg_rs=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();
    
    info!("Starting Web Dump Debugger");
    
    // Parse command line arguments
    let args = Args::parse();
    
    // Load configuration
    let mut config = if let Some(config_path) = args.config {
        info!("Loading configuration from {:?}", config_path);
        ServerConfig::from_file(&config_path)?
    } else {
        info!("Loading configuration from environment variables");
        ServerConfig::from_env()?
    };
    
    // Override port if specified
    if let Some(port) = args.port {
        info!("Overriding port to {}", port);
        config.port = port;
    }
    
    info!("Configuration loaded successfully");
    info!("  Port: {}", config.port);
    info!("  Max upload size: {} MB", config.max_upload_size / (1024 * 1024));
    info!("  Max concurrent sessions: {}", config.max_concurrent_sessions);
    info!("  Session TTL: {} hours", config.session_ttl_secs / 3600);
    info!("  Cleanup interval: {} hours", config.cleanup_interval_secs / 3600);
    info!("  Workspace root: {:?}", config.paths.workspace_root);
    
    // Create HTTP server
    let server = HttpServer::new(config.clone());
    
    // Get session manager for cleanup task
    let session_manager = server.session_manager();
    
    // Start cleanup task in background
    let cleanup_task = CleanupTask::new(session_manager, config.cleanup_interval());
    tokio::spawn(async move {
        cleanup_task.run().await;
    });
    
    info!("Cleanup task started");
    
    // Set up graceful shutdown
    let (shutdown_tx, mut shutdown_rx) = tokio::sync::mpsc::channel::<()>(1);
    
    // Spawn signal handler
    tokio::spawn(async move {
        match tokio::signal::ctrl_c().await {
            Ok(()) => {
                info!("Received Ctrl+C signal, initiating graceful shutdown");
                let _ = shutdown_tx.send(()).await;
            }
            Err(err) => {
                error!("Failed to listen for Ctrl+C signal: {}", err);
            }
        }
    });
    
    // Run server
    let server_handle = tokio::spawn(async move {
        if let Err(e) = server.run().await {
            error!("Server error: {}", e);
        }
    });
    
    // Wait for shutdown signal or server completion
    tokio::select! {
        _ = shutdown_rx.recv() => {
            info!("Shutdown signal received");
        }
        _ = server_handle => {
            info!("Server task completed");
        }
    }
    
    info!("Web Dump Debugger stopped");
    
    Ok(())
}
