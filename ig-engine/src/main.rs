//! IG Trading Engine — High-performance autonomous trading system
//!
//! Architecture:
//!   - Async event loop (tokio) processing real-time price data
//!   - Technical indicators computed in-memory with ring buffers
//!   - Strategy ensemble voting for trade decisions
//!   - Hard-gated risk manager (no trade bypasses it)
//!   - IG REST API for execution + deal confirmation
//!   - Internal HTTP + WebSocket API for dashboard integration

use anyhow::Result;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tracing::{info, error};

use ig_engine::engine::config::EngineConfig;
use ig_engine::engine::state::EngineState;
use ig_engine::ipc::EngineEvent;
use ig_engine::ipc;
use ig_engine::engine;

#[tokio::main]
async fn main() -> Result<()> {
    // Automatically load environment variables from .env file
    dotenvy::dotenv().ok();

    // Initialize structured logging (Stdout JSON + Rolling File)
    let file_appender = tracing_appender::rolling::daily("logs", "engine.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    use tracing_subscriber::prelude::*;
    let stdout_layer = tracing_subscriber::fmt::layer()
        .with_target(true)
        .with_thread_ids(true)
        .json();

    let file_layer = tracing_subscriber::fmt::layer()
        .with_target(true)
        .with_thread_ids(true)
        .with_ansi(false)
        .with_writer(non_blocking);

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| "ig_engine=info,tower_http=info".into()))
        .with(stdout_layer)
        .with(file_layer)
        .init();

    info!("=== IG Trading Engine v{} ===", env!("CARGO_PKG_VERSION"));

    // Load configuration
    let config_path = std::env::var("CONFIG_PATH")
        .unwrap_or_else(|_| "config/default.toml".to_string());
    let config = EngineConfig::load(&config_path)?;
    info!("Configuration loaded from {}", config_path);
    info!("Mode: {:?}, Markets: {}", config.general.mode, config.markets.epics.len());

    // Create shared state
    let state = Arc::new(RwLock::new(EngineState::new(config.clone())));

    // Event broadcast channel (engine → dashboard)
    let (event_tx, _) = broadcast::channel::<EngineEvent>(1024);

    // Start internal HTTP + WebSocket API for dashboard
    let api_state = state.clone();
    let api_event_tx = event_tx.clone();
    let api_port = config.general.api_port.unwrap_or(9090);
    tokio::spawn(async move {
        info!("Starting internal API on port {}", api_port);
        if let Err(e) = ipc::http_server::start(api_state, api_event_tx, api_port).await {
            error!("API server error: {}", e);
        }
    });

    // Run the main engine loop with graceful shutdown handling
    info!("Starting engine event loop...");
    let loop_event_tx = event_tx.clone();
    
    tokio::select! {
        res = engine::event_loop::run(state, event_tx) => {
            if let Err(e) = res {
                error!("Engine event loop error: {}", e);
            }
        }
        _ = tokio::signal::ctrl_c() => {
            info!("Ctrl+C received. Initiating graceful shutdown...");
            let _ = loop_event_tx.send(EngineEvent::shutdown("User terminal interrupt (Ctrl+C)".into()));
        }
        _ = async {
            #[cfg(unix)]
            {
                use tokio::signal::unix::{signal, SignalKind};
                let mut term = signal(SignalKind::terminate()).unwrap();
                term.recv().await;
            }
            #[cfg(not(unix))]
            {
                tokio::signal::ctrl_c().await.unwrap();
            }
        } => {
            info!("SIGTERM received. Initiating graceful shutdown...");
            let _ = loop_event_tx.send(EngineEvent::shutdown("System terminate signal (SIGTERM)".into()));
        }
    }

    info!("Engine main process exiting.");
    Ok(())
}
