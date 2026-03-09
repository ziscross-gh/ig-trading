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

    // Write PID file so weekly_reoptimise.sh can send SIGUSR1 for hot-reload
    let pid = std::process::id();
    match std::fs::write("ig-engine.pid", pid.to_string()) {
        Ok(_)  => info!("PID {} written to ig-engine.pid", pid),
        Err(e) => tracing::warn!("Could not write ig-engine.pid: {}", e),
    }

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

    // Spawn SIGUSR1 handler — hot-reloads non-risk strategy config in place.
    // Triggered by weekly_reoptimise.sh after auto-applying improved parameters.
    // Only updates instrument_overrides and consensus thresholds; risk params
    // require a full restart and are never changed by this handler.
    #[cfg(unix)]
    {
        let state_reload      = state.clone();
        let config_path_reload = config_path.clone();
        tokio::spawn(async move {
            use tokio::signal::unix::{signal, SignalKind};
            let mut usr1 = match signal(SignalKind::user_defined1()) {
                Ok(s)  => s,
                Err(e) => {
                    tracing::error!("Failed to register SIGUSR1 handler: {}", e);
                    return;
                }
            };
            loop {
                usr1.recv().await;
                info!("SIGUSR1 received — reloading strategy configuration...");
                match EngineConfig::load(&config_path_reload) {
                    Ok(new_config) => {
                        let mut s = state_reload.write().await;
                        s.reload_strategy_config(new_config.strategies);
                        info!("Strategy config hot-reloaded successfully via SIGUSR1");
                    }
                    Err(e) => {
                        tracing::error!(
                            "Config reload failed — keeping existing config: {}",
                            e
                        );
                    }
                }
            }
        });
    }

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
                if let Ok(mut term) = signal(SignalKind::terminate()) {
                    term.recv().await;
                } else {
                    let _ = tokio::signal::ctrl_c().await;
                }
            }
            #[cfg(not(unix))]
            {
                let _ = tokio::signal::ctrl_c().await;
            }
        } => {
            info!("SIGTERM received. Initiating graceful shutdown...");
            let _ = loop_event_tx.send(EngineEvent::shutdown("System terminate signal (SIGTERM)".into()));
        }
    }

    info!("Engine main process exiting.");
    Ok(())
}
