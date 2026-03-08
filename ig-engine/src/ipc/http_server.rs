use axum::{
    extract::{ws::{Message, WebSocket, WebSocketUpgrade}, Query, State},
    routing::{get, post, put},
    Json, Router,
};
use tracing::{info, error, info_span};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tower_http::cors::CorsLayer;

use crate::engine::state::{EngineState, EngineStatus};
use crate::engine::config::EngineMode;
use crate::ipc::events::EngineEvent;
use crate::notifications::telegram::get_instrument_name;
use crate::engine::optimizer::{Optimizer, OptimizationResult};
use crate::engine::backtester::{BacktestEngine};
use crate::strategy::{
    ma_crossover::MACrossoverStrategy,
    rsi_reversal::RSIReversalStrategy,
    macd_momentum::MACDMomentumStrategy,
    bollinger::BollingerStrategy,
};

/// Application state shared across all handlers
#[derive(Clone)]
pub struct AppState {
    pub engine_state: Arc<RwLock<EngineState>>,
    pub event_tx: broadcast::Sender<EngineEvent>,
    pub last_optimization_result: Arc<RwLock<Option<OptimizationResult>>>,
}

/// Query parameters for limiting results
#[derive(Debug, Deserialize)]
pub struct LimitQuery {
    limit: Option<usize>,
}

/// Query parameters for prices endpoint
#[derive(Debug, Deserialize)]
pub struct PricesQuery {
    epic: String,
    resolution: Option<String>,
    max: Option<usize>,
}

/// Query parameters for indicators endpoint
#[derive(Debug, Deserialize)]
pub struct IndicatorsQuery {
    epic: String,
}

/// Query parameters for scan endpoint
#[derive(Debug, Deserialize)]
pub struct ScanQuery {
    limit: Option<usize>,
}

/// Mode switch request body
#[derive(Debug, Deserialize)]
pub struct ModeRequest {
    mode: String, // "paper" | "live"
}

/// Control request body (start/stop/pause)
#[derive(Debug, Deserialize)]
pub struct ControlRequest {
    action: String,
}

#[derive(Debug, Deserialize)]
pub struct ConfigUpdateRequest {
    #[serde(flatten)]
    pub updates: serde_json::Map<String, Value>,
}

#[derive(Debug, Deserialize)]
pub struct OptimizeRequest {
    pub epic: String,
    pub short_range: (usize, usize),
    pub long_range: (usize, usize),
}

#[derive(Debug, Deserialize)]
pub struct BacktestRequest {
    pub epic: String,
    pub strategy_name: String,
    pub initial_balance: f64,
    pub risk_pct: f64,
}

#[derive(Debug, Deserialize)]
pub struct TriggerRequest {
    pub epic: String,
    pub direction: String, // "buy" | "sell"
}

#[derive(Debug, Serialize)]
pub struct StatusResponse {
    mode: String,
    status: String,
    uptime_secs: u64,
    account: AccountSummary,
    open_positions: usize,
    daily_stats: DailyStatsSummary,
    circuit_breaker: CircuitBreakerStatus,
}

#[derive(Debug, Serialize)]
pub struct AccountSummary {
    balance: f64,
    available: f64,
    margin_used: f64,
    pnl: f64,
}

#[derive(Debug, Serialize)]
pub struct DailyStatsSummary {
    trades_today: u32,
    winning: u32,
    losing: u32,
    net_pnl: f64,
    max_drawdown_pct: f64,
}

#[derive(Debug, Serialize)]
pub struct CircuitBreakerStatus {
    consecutive_losses: u32,
    is_paused: bool,
    size_multiplier: f64,
}

#[derive(Debug, Serialize)]
pub struct ConfigSummary {
    mode: String,
    max_risk_per_trade: f64,
    max_daily_loss_pct: f64,
    max_open_positions: usize,
    markets: Vec<String>,
    strategies: StrategiesSummary,
}

#[derive(Debug, Serialize)]
pub struct StrategiesSummary {
    ma_crossover: bool,
    rsi_divergence: bool,
    macd_momentum: bool,
    bollinger_reversion: bool,
    min_consensus: usize,
    min_avg_strength: f64,
}

/// Start the HTTP server
///
/// Spawns a tokio task to run the Axum server on the specified port.
/// Takes ownership of the engine state and event channel for broadcast updates.
pub async fn start(
    engine_state: Arc<RwLock<EngineState>>,
    event_tx: broadcast::Sender<EngineEvent>,
    port: u16,
) -> anyhow::Result<()> {
    let app_state = AppState {
        engine_state,
        event_tx: event_tx.clone(),
        last_optimization_result: Arc::new(RwLock::new(None)),
    };

    let app = Router::new()
        .route("/api/health", get(get_health))
        .route("/api/ready", get(get_ready))
        .route("/api/status", get(get_status))
        .route("/api/positions", get(get_positions))
        .route("/api/signals", get(get_signals))
        .route("/api/signals-history", get(get_signals))
        .route("/api/trades", get(get_trades))
        .route("/api/config", get(get_config))
        .route("/api/markets", get(get_markets))
        .route("/api/prices", get(get_prices))
        .route("/api/indicators", get(get_indicators))
        .route("/api/stats", get(get_stats))
        .route("/api/scan", get(get_scan))
        .route("/api/learning", get(get_learning))
        .route("/api/config/mode", post(post_config_mode))
        .route("/api/optimize", post(post_optimize))
        .route("/api/backtest", post(post_backtest))
        .route("/api/optimizer/results", get(get_optimizer_results))
        .route("/api/control", post(post_control))
        .route("/api/trigger", post(post_trigger))
        .route("/api/config", put(put_config))
        .route("/api/ws", get(ws_handler))
        .layer(CorsLayer::permissive())
        .with_state(app_state);

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
    let local_addr = listener.local_addr()?;
    tracing::info!("IPC HTTP Server listening on http://{}", local_addr);

    let mut shutdown_rx = event_tx.subscribe();
    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            while let Ok(event) = shutdown_rx.recv().await {
                if let crate::ipc::events::EventVariant::Shutdown { reason } = event.event {
                    tracing::info!("HTTP server receiving shutdown signal: {}. Closing listener...", reason);
                    break;
                }
            }
        })
        .await?;

    Ok(())
}

/// Health check endpoint with system metrics
async fn get_health(State(app_state): State<AppState>) -> Json<Value> {
    let state = app_state.engine_state.read().await;

    let uptime_secs = state
        .started_at
        .map(|started| (Utc::now() - started).num_seconds() as u64)
        .unwrap_or(0);

    Json(json!({
        "status": "healthy",
        "uptime_secs": uptime_secs,
        "engine_status": format!("{:?}", state.status).to_lowercase(),
        "connected_to_ig": state.session.ig_session_token.is_some(),
        "timestamp": Utc::now().to_rfc3339(),
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

/// Readiness check endpoint: returns 200 when engine status is Running
/// Returns 503 when engine is not yet ready
async fn get_ready(State(app_state): State<AppState>) -> Result<Json<Value>, (axum::http::StatusCode, Json<Value>)> {
    let state = app_state.engine_state.read().await;

    match state.status {
        EngineStatus::Running => {
            Ok(Json(json!({
                "status": "ready",
                "timestamp": Utc::now().to_rfc3339(),
            })))
        }
        _ => {
            Err((
                axum::http::StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({
                    "status": "not_ready",
                    "reason": "Engine is not running",
                    "timestamp": Utc::now().to_rfc3339(),
                }))
            ))
        }
    }
}

/// Status endpoint: mode, status, uptime, account info, positions, daily stats, circuit breaker
async fn get_status(State(app_state): State<AppState>) -> Json<StatusResponse> {
    let state = app_state.engine_state.read().await;

    // Calculate uptime in seconds
    let uptime_secs = state
        .started_at
        .map(|started| (Utc::now() - started).num_seconds() as u64)
        .unwrap_or(0);

    let status_str = match state.status {
        EngineStatus::Starting => "starting",
        EngineStatus::Running => "running",
        EngineStatus::Paused => "paused",
        EngineStatus::Stopped => "stopped",
        EngineStatus::Error => "error",
    };

    let mode_str = format!("{:?}", state.config.general.mode).to_lowercase();

    Json(StatusResponse {
        mode: mode_str,
        status: status_str.to_string(),
        uptime_secs,
        account: AccountSummary {
            balance: state.account.balance,
            available: state.account.available,
            margin_used: state.account.margin,
            pnl: state.account.pnl,
        },
        open_positions: state.trades.active.len(),
        daily_stats: DailyStatsSummary {
            trades_today: state.metrics.daily.trades,
            winning: state.metrics.daily.wins,
            losing: state.metrics.daily.losses,
            net_pnl: state.metrics.daily.pnl,
            max_drawdown_pct: if state.account.balance > 0.0 {
                (state.metrics.daily.max_drawdown / state.account.balance) * 100.0
            } else {
                0.0
            },
        },
        circuit_breaker: CircuitBreakerStatus {
            consecutive_losses: state.metrics.daily.consecutive_losses,
            is_paused: state.metrics.circuit_breaker_active,
            size_multiplier: if state.metrics.daily.consecutive_losses > 3 {
                0.5
            } else {
                1.0
            },
        },
    })
}

/// Positions endpoint: return all open positions
async fn get_positions(State(app_state): State<AppState>) -> Json<Value> {
    let state = app_state.engine_state.read().await;

    let positions: Vec<Value> = state
        .trades.active
        .iter()
        .map(|pos| {
            json!({
                "deal_id": pos.deal_id,
                "epic": pos.epic,
                "name": get_instrument_name(&pos.epic),
                "direction": pos.direction.to_string().to_lowercase(),
                "size": pos.size,
                "entry_price": pos.open_price,
                "current_price": pos.current_price,
                "unrealised_pnl": pos.pnl,
                "stop_loss": pos.stop_loss,
                "take_profit": pos.take_profit,
                "strategy": pos.strategy,
                "opened_at": pos.opened_at.to_rfc3339(),
                "is_virtual": pos.is_virtual,
            })
        })
        .collect();

    Json(json!({
        "count": positions.len(),
        "positions": positions,
    }))
}

/// Signals endpoint: return recent signals with execution status (default limit 50)
async fn get_signals(
    State(app_state): State<AppState>,
    Query(params): Query<LimitQuery>,
) -> Json<Value> {
    let state = app_state.engine_state.read().await;
    let limit = params.limit.unwrap_or(50);

    // Prefer signal_records (with execution tracking) if available
    let signals: Vec<Value> = if !state.trades.signal_records.is_empty() {
        state
            .trades.signal_records
            .iter()
            .rev()
            .take(limit)
            .map(|record| {
                let signal = &record.signal;
                json!({
                    "epic": signal.epic,
                    "name": get_instrument_name(&signal.epic),
                    "direction": signal.direction.to_string().to_lowercase(),
                    "strength": signal.strength,
                    "strategy": signal.strategy,
                    "reason": signal.reason,
                    "price": signal.price,
                    "stop_loss": signal.stop_loss,
                    "take_profit": signal.take_profit,
                    "was_executed": record.was_executed,
                    "rejection_reason": record.rejection_reason,
                    "timestamp": signal.timestamp.to_rfc3339(),
                })
            })
            .collect()
    } else {
        // Fallback to raw signals (backward compat)
        state
            .trades.signals
            .iter()
            .rev()
            .take(limit)
            .map(|signal| {
                json!({
                    "epic": signal.epic,
                    "name": get_instrument_name(&signal.epic),
                    "direction": signal.direction.to_string().to_lowercase(),
                    "strength": signal.strength,
                    "strategy": signal.strategy,
                    "reason": signal.reason,
                    "price": signal.price,
                    "stop_loss": signal.stop_loss,
                    "take_profit": signal.take_profit,
                    "was_executed": false,
                    "rejection_reason": serde_json::Value::Null,
                    "timestamp": signal.timestamp.to_rfc3339(),
                })
            })
            .collect()
    };

    Json(json!({
        "count": signals.len(),
        "limit": limit,
        "signals": signals,
    }))
}

/// Trades endpoint: return closed trade history (limit default 100)
async fn get_trades(
    State(app_state): State<AppState>,
    Query(params): Query<LimitQuery>,
) -> Json<Value> {
    let state = app_state.engine_state.read().await;
    let limit = params.limit.unwrap_or(100);

    let trades: Vec<Value> = state
        .trades.history
        .iter()
        .rev()
        .take(limit)
        .map(|trade| {
            json!({
                "deal_id": trade.deal_id,
                "epic": trade.epic,
                "name": get_instrument_name(&trade.epic),
                "direction": trade.direction.to_string().to_lowercase(),
                "size": trade.size,
                "entry_price": trade.entry_price,
                "exit_price": trade.exit_price,
                "stop_loss": trade.stop_loss,
                "take_profit": trade.take_profit,
                "pnl": trade.pnl,
                "strategy": trade.strategy,
                "status": trade.status,
                "opened_at": trade.opened_at.to_rfc3339(),
                "closed_at": trade.closed_at.to_rfc3339(),
                "is_virtual": trade.is_virtual,
            })
        })
        .collect();

    Json(json!({
        "count": trades.len(),
        "limit": limit,
        "trades": trades,
    }))
}

/// Config endpoint: return configuration summary
async fn get_config(State(app_state): State<AppState>) -> Json<ConfigSummary> {
    let state = app_state.engine_state.read().await;

    let mode_str = format!("{:?}", state.config.general.mode).to_lowercase();

    let strategies = &state.config.strategies;
    Json(ConfigSummary {
        mode: mode_str,
        max_risk_per_trade: state.config.risk.max_risk_per_trade,
        max_daily_loss_pct: state.config.risk.max_daily_loss_pct,
        max_open_positions: state.config.risk.max_open_positions,
        markets: state.config.markets.epics.clone(),
        strategies: StrategiesSummary {
            ma_crossover: strategies
                .ma_crossover
                .as_ref()
                .map(|s| s.enabled)
                .unwrap_or(false),
            rsi_divergence: strategies
                .rsi_divergence
                .as_ref()
                .map(|s| s.enabled)
                .unwrap_or(false),
            macd_momentum: strategies
                .macd_momentum
                .as_ref()
                .map(|s| s.enabled)
                .unwrap_or(false),
            bollinger_reversion: strategies
                .bollinger_reversion
                .as_ref()
                .map(|s| s.enabled)
                .unwrap_or(false),
            min_consensus: strategies.min_consensus,
            min_avg_strength: strategies.min_avg_strength,
        },
    })
}

/// Markets endpoint: return all configured markets with current prices
/// Prefers live market_states from Lightstreamer, falls back to candle_store
async fn get_markets(State(app_state): State<AppState>) -> Json<Value> {
    let state = app_state.engine_state.read().await;

    let markets: Vec<Value> = state
        .config
        .markets
        .epics
        .iter()
        .map(|epic| {
            let name = get_instrument_name(epic);
            // Prefer live market state from Lightstreamer
            if let Some(ms) = state.markets.live.get(epic) {
                let mid = (ms.bid + ms.ask) / 2.0;
                let net_change = mid * ms.change_pct / 100.0;
                json!({
                    "epic": epic,
                    "name": name,
                    "bid": ms.bid,
                    "offer": ms.ask,
                    "high": ms.high,
                    "low": ms.low,
                    "change": net_change,
                    "changePercent": ms.change_pct,
                    "timestamp": ms.last_update.to_rfc3339(),
                })
            } else {
                // Fallback to latest candle from historical data
                let latest = state.markets.history.get_latest(epic, "HOUR");
                let (bid, offer, change, change_percent) = if let Some(candle) = latest {
                    let prev_close = candle.open;
                    let ch = candle.close - prev_close;
                    let ch_pct = if prev_close > 0.0 { (ch / prev_close) * 100.0 } else { 0.0 };
                    (candle.close, candle.close, ch, ch_pct)
                } else {
                    (0.0, 0.0, 0.0, 0.0)
                };

                let high = latest.map(|c| c.high).unwrap_or(0.0);
                let low = latest.map(|c| c.low).unwrap_or(0.0);

                json!({
                    "epic": epic,
                    "name": name,
                    "bid": bid,
                    "offer": offer,
                    "high": high,
                    "low": low,
                    "change": change,
                    "changePercent": change_percent,
                    "timestamp": Utc::now().to_rfc3339(),
                })
            }
        })
        .collect();

    Json(json!({
        "count": markets.len(),
        "markets": markets,
        "timestamp": Utc::now().to_rfc3339(),
    }))
}

/// Prices endpoint: return price history for a specific epic
async fn get_prices(
    State(app_state): State<AppState>,
    Query(params): Query<PricesQuery>,
) -> Json<Value> {
    let state = app_state.engine_state.read().await;
    let resolution = params.resolution.as_deref().unwrap_or("HOUR");
    let max = params.max.unwrap_or(100);

    let candles = state
        .markets.history
        .get_candles(&params.epic, resolution);

    let prices: Vec<Value> = if let Some(candles) = candles {
        candles
            .iter()
            .rev()
            .take(max)
            .rev()
            .map(|c| {
                json!({
                    "open": c.open,
                    "high": c.high,
                    "low": c.low,
                    "close": c.close,
                    "volume": c.volume,
                    "timestamp": chrono::DateTime::from_timestamp(c.timestamp, 0)
                        .map(|t| t.to_rfc3339())
                        .unwrap_or_default(),
                })
            })
            .collect()
    } else {
        Vec::new()
    };

    Json(json!({
        "epic": params.epic,
        "name": get_instrument_name(&params.epic),
        "resolution": resolution,
        "count": prices.len(),
        "prices": prices,
        "timestamp": Utc::now().to_rfc3339(),
    }))
}

/// Control endpoint: accept action to start/stop/pause the engine
async fn post_control(
    State(app_state): State<AppState>,
    Json(payload): Json<ControlRequest>,
) -> Json<Value> {
    let mut state = app_state.engine_state.write().await;

    let new_status = match payload.action.as_str() {
        "start" => EngineStatus::Running,
        "stop" => EngineStatus::Stopped,
        "pause" => EngineStatus::Paused,
        _ => {
            return Json(json!({
                "success": false,
                "message": format!("Unknown action: {}", payload.action),
            }))
        }
    };

    let old_status = state.status.clone();
    state.status = new_status.clone();

    let status_str = match new_status {
        EngineStatus::Starting => "starting",
        EngineStatus::Running => "running",
        EngineStatus::Paused => "paused",
        EngineStatus::Stopped => "stopped",
        EngineStatus::Error => "error",
    };

    // Broadcast the status change event
    let old_status_str = format!("{:?}", old_status).to_lowercase();
    let new_status_str = status_str.to_string();
    let event = EngineEvent::status_change(old_status_str, new_status_str.clone());
    let _ = app_state.event_tx.send(event);

    Json(json!({
        "success": true,
        "message": format!("Engine {} successfully", payload.action),
        "status": status_str,
    }))
}

/// Indicators endpoint: return current IndicatorSnapshot for a specific epic
/// Dashboard uses this to render chart overlays (SMA, EMA, BB, RSI, MACD, ATR, ADX)
/// without needing to recalculate them from raw candles.
async fn get_indicators(
    State(app_state): State<AppState>,
    Query(params): Query<IndicatorsQuery>,
) -> Json<Value> {
    let state = app_state.engine_state.read().await;

    match state.markets.indicators.get(&params.epic) {
        Some(indicator_set_map) => {
            match indicator_set_map.get("HOUR").and_then(|i| i.snapshot()) {
                Some(snap) => Json(json!({
                    "epic": params.epic,
                    "available": true,
                    "indicators": {
                        // Moving averages
                        "sma_short": snap.sma_short,
                        "sma_long": snap.sma_long,
                        "ema_short": snap.ema_short,
                        "ema_long": snap.ema_long,
                        "ema_200": snap.ema_200,
                        "prev_ema_short": snap.prev_ema_short,
                        "prev_ema_long": snap.prev_ema_long,
                        // RSI
                        "rsi": snap.rsi,
                        // MACD
                        "macd": snap.macd,
                        "macd_signal": snap.macd_signal,
                        "macd_histogram": snap.macd_histogram,
                        "prev_macd": snap.prev_macd,
                        "prev_macd_histogram": snap.prev_macd_histogram,
                        // ATR
                        "atr": snap.atr,
                        // Bollinger Bands
                        "bollinger_upper": snap.bollinger_upper,
                        "bollinger_middle": snap.bollinger_middle,
                        "bollinger_lower": snap.bollinger_lower,
                        "bollinger_bandwidth": snap.bollinger_bandwidth,
                        "bollinger_percent_b": snap.bollinger_percent_b,
                        // ADX / Directional Movement
                        "adx": snap.adx,
                        "plus_di": snap.plus_di,
                        "minus_di": snap.minus_di,
                        // Stochastic
                        "stochastic_k": snap.stochastic_k,
                        "stochastic_d": snap.stochastic_d,
                    },
                    "timestamp": Utc::now().to_rfc3339(),
                })),
                None => Json(json!({
                    "epic": params.epic,
                    "available": false,
                    "message": "Indicator set is still warming up — not enough candles yet",
                    "timestamp": Utc::now().to_rfc3339(),
                })),
            }
        }
        None => Json(json!({
            "epic": params.epic,
            "available": false,
            "message": format!("Epic '{}' not found in indicator registry", params.epic),
            "timestamp": Utc::now().to_rfc3339(),
        })),
    }
}

/// Stats endpoint: detailed daily stats, equity curve points, and win/loss breakdown
/// Provides everything the dashboard performance panel needs.
async fn get_stats(State(app_state): State<AppState>) -> Json<Value> {
    let state = app_state.engine_state.read().await;
    let ds = &state.metrics.daily;

    // Compute gross profit/loss from closed trade history (wins and losses separately)
    let (gross_profit, gross_loss) = state.trades.history.iter().fold((0.0f64, 0.0f64), |(profit, loss), t| {
        if t.pnl > 0.0 {
            (profit + t.pnl, loss)
        } else {
            (profit, loss + t.pnl)
        }
    });

    let win_rate = if ds.trades > 0 {
        (ds.wins as f64 / ds.trades as f64) * 100.0
    } else {
        0.0
    };

    let avg_win = if ds.wins > 0 && gross_profit > 0.0 {
        gross_profit / ds.wins as f64
    } else {
        0.0
    };

    let avg_loss = if ds.losses > 0 && gross_loss < 0.0 {
        gross_loss / ds.losses as f64
    } else {
        0.0
    };

    let profit_factor = if gross_loss.abs() > 0.0 {
        gross_profit / gross_loss.abs()
    } else if gross_profit > 0.0 {
        999.0 // Represent "infinity" as a large number for JSON serialization
    } else {
        0.0
    };

    // Build equity curve from closed trades (cumulative PnL)
    let equity_curve: Vec<Value> = {
        let mut cumulative = 0.0;
        state
            .trades.history
            .iter()
            .map(|t| {
                cumulative += t.pnl;
                json!({
                    "timestamp": t.closed_at.to_rfc3339(),
                    "pnl": t.pnl,
                    "cumulative_pnl": cumulative,
                    "epic": t.epic,
                    "name": get_instrument_name(&t.epic),
                    "strategy": t.strategy,
                })
            })
            .collect()
    };

    Json(json!({
        "daily": {
            "trades": ds.trades,
            "wins": ds.wins,
            "losses": ds.losses,
            "win_rate_pct": win_rate,
            "net_pnl": ds.pnl,
            "gross_profit": gross_profit,
            "gross_loss": gross_loss,
            "avg_win": avg_win,
            "avg_loss": avg_loss,
            "profit_factor": profit_factor,
            "max_drawdown": ds.max_drawdown,
            "consecutive_losses": ds.consecutive_losses,
        },
        "all_time": {
            "total_trades": state.trades.history.len(),
            "equity_curve": equity_curve,
        },
        "circuit_breaker": {
            "active": state.metrics.circuit_breaker_active,
            "consecutive_losses": ds.consecutive_losses,
            "size_multiplier": if ds.consecutive_losses > 3 { 0.5 } else { 1.0 },
        },
        "timestamp": Utc::now().to_rfc3339(),
    }))
}

/// Market scan endpoint: scan all configured markets for trading opportunities
/// Returns markets ranked by signal strength (strongest first)
async fn get_scan(
    State(app_state): State<AppState>,
    Query(params): Query<ScanQuery>,
) -> Json<Value> {
    let state = app_state.engine_state.read().await;
    let limit = params.limit.unwrap_or(10);

    // Scan each configured market
    let mut opportunities: Vec<Value> = state
        .config
        .markets
        .epics
        .iter()
        .filter_map(|epic| {
            // Get live market state
            let market_state = state.markets.live.get(epic)?;

            // Get indicators for this market
            let indicators = state.markets.indicators.get(epic)?.get("HOUR")?.snapshot()?;

            // Calculate trend and strength
            let price = market_state.bid;
            let sma20 = indicators.sma_short.unwrap_or(price);
            let sma50 = indicators.sma_long.unwrap_or(price);
            let rsi = indicators.rsi.unwrap_or(50.0);

            // Trend detection
            let trend = if price > sma20 && sma20 > sma50 {
                "bullish"
            } else if price < sma20 && sma20 < sma50 {
                "bearish"
            } else {
                "neutral"
            };

            // Signal strength (0-10)
            let strength = ((rsi - 50.0).abs() / 5.0
                + if trend == "bullish" { 2.0 } else if trend == "bearish" { 2.0 } else { 0.0 }
                + ((price - sma20).abs() / sma20 * 100.0).min(10.0))
                .min(10.0)
                .max(0.0);

            // Generate signal
            let signal = if trend == "bullish" && rsi < 70.0 {
                "BUY"
            } else if trend == "bearish" && rsi > 30.0 {
                "SELL"
            } else {
                "HOLD"
            };

            Some(json!({
                "epic": epic,
                "name": get_instrument_name(epic),
                "price": price,
                "bid": market_state.bid,
                "ask": market_state.ask,
                "spread": market_state.spread,
                "change": market_state.ask - market_state.bid,
                "changePercent": market_state.change_pct,
                "trend": trend,
                "strength": (strength as i32),
                "rsi": (rsi * 100.0) as i32 / 100,
                "signal": signal,
                "timestamp": market_state.last_update.to_rfc3339(),
            }))
        })
        .collect();

    // Sort by strength (descending) and return top N
    opportunities.sort_by(|a, b| {
        let strength_a = b["strength"].as_i64().unwrap_or(0);
        let strength_b = a["strength"].as_i64().unwrap_or(0);
        strength_a.cmp(&strength_b)
    });

    let top = opportunities.into_iter().take(limit).collect::<Vec<_>>();

    Json(json!({
        "success": true,
        "count": top.len(),
        "limit": limit,
        "opportunities": top,
        "timestamp": Utc::now().to_rfc3339(),
    }))
}

/// Adaptive learning system endpoint: strategy scorecards, session heatmap, and adjustment log.
/// Reads directly from the `LearningSnapshot` written by the event loop after each trade close.
async fn get_learning(State(app_state): State<AppState>) -> Json<Value> {
    let state = app_state.engine_state.read().await;

    let (scorecard, weight_manager) = match (state.learning.scorecard.as_ref(), state.learning.weight_manager.as_ref()) {
        (Some(s), Some(w)) => (s, w),
        _ => return Json(json!({
            "total_trades_processed": 0,
            "strategies": [],
            "recent_adjustments": [],
            "timestamp": Utc::now().to_rfc3339(),
        })),
    };

    let mut strategies = Vec::new();
    let current_multipliers = weight_manager.get_multipliers();
    let effective_weights = weight_manager.get_effective_weights();

    for strategy in scorecard.strategies() {
        if let Some(perf) = scorecard.get_performance(&strategy) {
            let mut sessions = serde_json::Map::new();
            
            for &session in &[
                crate::engine::state::Session::Asia,
                crate::engine::state::Session::London,
                crate::engine::state::Session::UsOverlap
            ] {
                if let Some(s_perf) = scorecard.get_session_performance(&strategy, session) {
                    sessions.insert(
                        session.label().to_string(),
                        json!({
                            "win_rate": s_perf.win_rate,
                            "profit_factor": s_perf.profit_factor,
                        })
                    );
                }
            }

            let mult = current_multipliers.get(&strategy).copied().unwrap_or(1.0);
            let ew = effective_weights.get(&strategy).copied().unwrap_or(1.0);

            strategies.push(json!({
                "name": strategy,
                "win_rate": perf.win_rate,
                "profit_factor": perf.profit_factor,
                "current_multiplier": mult,
                "effective_weight": ew,
                "max_consecutive_losses": perf.max_consecutive_losses,
                "trades_in_window": perf.total_trades,
                "sessions": sessions,
            }));
        }
    }

    Json(json!({
        "total_trades_processed": scorecard.total_trades_processed,
        "strategies": strategies,
        "recent_adjustments": weight_manager.adjustment_log,
        "timestamp": Utc::now().to_rfc3339(),
    }))
}

/// Mode switch endpoint: switch between PAPER and LIVE trading at runtime
async fn post_config_mode(
    State(app_state): State<AppState>,
    Json(payload): Json<ModeRequest>,
) -> Json<Value> {
    let mut state = app_state.engine_state.write().await;

    let new_mode = match payload.mode.to_lowercase().as_str() {
        "paper" => EngineMode::Paper,
        "live" => EngineMode::Live,
        other => {
            return Json(json!({
                "success": false,
                "message": format!("Unknown mode '{}'. Use 'paper' or 'live'.", other),
            }))
        }
    };

    let old_mode = format!("{:?}", state.config.general.mode).to_lowercase();
    state.config.general.mode = new_mode;
    let new_mode_str = payload.mode.to_lowercase();

    // Broadcast config change
    let event = EngineEvent::config_changed(format!("mode: {} -> {}", old_mode, new_mode_str));
    let _ = app_state.event_tx.send(event);

    Json(json!({
        "success": true,
        "message": format!("Mode switched to {}", new_mode_str),
        "old_mode": old_mode,
        "new_mode": new_mode_str,
    }))
}

/// Optimization results endpoint
pub async fn get_optimizer_results(State(app_state): State<AppState>) -> Json<Value> {
    let result = app_state.last_optimization_result.read().await;
    match &*result {
        Some(res) => Json(json!({ "success": true, "result": res })),
        None => Json(json!({ "success": false, "message": "No optimization runs found" })),
    }
}

/// Trigger optimization run
pub async fn post_optimize(
    State(app_state): State<AppState>,
    Json(payload): Json<OptimizeRequest>,
) -> Json<Value> {
    let epic = payload.epic.clone();
    
    // Get real candles from the store if available
    let mut candles = {
        let s = app_state.engine_state.read().await;
        s.markets.history.get_candles(&epic, "HOUR")
            .cloned()
            .unwrap_or_default()
    };

    if candles.is_empty() {
        info!("No real candles found for {}, generating mock data for demo", epic);
        let mut current_price = 2000.0;
        for i in 0..200 {
            let change = (rand::random::<f64>() - 0.5) * 5.0;
            current_price += change;
            candles.push(crate::indicators::Candle {
                timestamp: Utc::now().timestamp() - (200 - i) * 3600,
                open: current_price - 1.0,
                high: current_price + 2.0,
                low: current_price - 2.0,
                close: current_price,
                volume: 1000,
            });
        }
    } else {
        info!("Using {} real candles for optimization of {}", candles.len(), epic);
    }

    let balance = {
        let s = app_state.engine_state.read().await;
        s.account.balance
    }.max(1000.0); // Fallback to 1000 if state is empty

    let optimizer = Optimizer::new(candles, balance);
    
    // Run optimization in a separate task so we don't block the API
    let state_clone = app_state.clone();
    tokio::spawn(async move {
        let result = optimizer.optimize_ma_crossover(
            &epic,
            payload.short_range.0..payload.short_range.1,
            payload.long_range.0..payload.long_range.1,
            vec![20.0, 25.0, 30.0],
        ).await;

        let mut last_res = state_clone.last_optimization_result.write().await;
        *last_res = Some(result);
    });

    Json(json!({ "success": true, "message": "Optimization started" }))
}

/// Config update endpoint: partially update configuration
async fn put_config(
    State(app_state): State<AppState>,
    Json(payload): Json<ConfigUpdateRequest>,
) -> Json<Value> {
    let mut state = app_state.engine_state.write().await;
    let mut updated_fields = Vec::new();

    // Update risk settings
    if let Some(max_risk_per_trade) = payload.updates.get("max_risk_per_trade") {
        if let Some(val) = max_risk_per_trade.as_f64() {
            state.config.risk.max_risk_per_trade = val;
            updated_fields.push("max_risk_per_trade");
        }
    }

    if let Some(max_daily_loss_pct) = payload.updates.get("max_daily_loss_pct") {
        if let Some(val) = max_daily_loss_pct.as_f64() {
            state.config.risk.max_daily_loss_pct = val;
            updated_fields.push("max_daily_loss_pct");
        }
    }

    if let Some(max_open_positions) = payload.updates.get("max_open_positions") {
        if let Some(val) = max_open_positions.as_u64() {
            state.config.risk.max_open_positions = val as usize;
            updated_fields.push("max_open_positions");
        }
    }

    if let Some(max_margin_usage_pct) = payload.updates.get("max_margin_usage_pct") {
        if let Some(val) = max_margin_usage_pct.as_f64() {
            state.config.risk.max_margin_usage_pct = val;
            updated_fields.push("max_margin_usage_pct");
        }
    }

    // Update strategy settings
    if let Some(min_consensus) = payload.updates.get("min_consensus") {
        if let Some(val) = min_consensus.as_u64() {
            state.config.strategies.min_consensus = val as usize;
            updated_fields.push("min_consensus");
        }
    }

    if let Some(min_avg_strength) = payload.updates.get("min_avg_strength") {
        if let Some(val) = min_avg_strength.as_f64() {
            state.config.strategies.min_avg_strength = val;
            updated_fields.push("min_avg_strength");
        }
    }

    // Nested strategy updates
    if let Some(strategies) = payload.updates.get("strategies").and_then(|s| s.as_object()) {
        // MA Crossover
        if let Some(ma_config) = strategies.get("ma_crossover").and_then(|ma| ma.as_object()) {
            if let Some(config) = &mut state.config.strategies.ma_crossover {
                if let Some(enabled) = ma_config.get("enabled").and_then(|e| e.as_bool()) {
                    config.enabled = enabled;
                    updated_fields.push("ma_crossover.enabled");
                }
                if let Some(short) = ma_config.get("short_period").and_then(|s| s.as_u64()) {
                    config.short_period = short as usize;
                    updated_fields.push("ma_crossover.short_period");
                }
                if let Some(long) = ma_config.get("long_period").and_then(|l| l.as_u64()) {
                    config.long_period = long as usize;
                    updated_fields.push("ma_crossover.long_period");
                }
                if let Some(adx) = ma_config.get("require_adx_above").and_then(|a| a.as_f64()) {
                    config.require_adx_above = adx;
                    updated_fields.push("ma_crossover.require_adx_above");
                }
            }
        }

        // RSI Divergence
        if let Some(rsi_config) = strategies.get("rsi_divergence").and_then(|r| r.as_object()) {
            if let Some(config) = &mut state.config.strategies.rsi_divergence {
                if let Some(enabled) = rsi_config.get("enabled").and_then(|e| e.as_bool()) {
                    config.enabled = enabled;
                    updated_fields.push("rsi_divergence.enabled");
                }
                if let Some(period) = rsi_config.get("period").and_then(|p| p.as_u64()) {
                    config.period = period as usize;
                    updated_fields.push("rsi_divergence.period");
                }
                if let Some(overbought) = rsi_config.get("overbought").and_then(|o| o.as_f64()) {
                    config.overbought = overbought;
                    updated_fields.push("rsi_divergence.overbought");
                }
                if let Some(oversold) = rsi_config.get("oversold").and_then(|o| o.as_f64()) {
                    config.oversold = oversold;
                    updated_fields.push("rsi_divergence.oversold");
                }
            }
        }

        // MACD Momentum
        if let Some(macd_config) = strategies.get("macd_momentum").and_then(|m| m.as_object()) {
            if let Some(config) = &mut state.config.strategies.macd_momentum {
                if let Some(enabled) = macd_config.get("enabled").and_then(|e| e.as_bool()) {
                    config.enabled = enabled;
                    updated_fields.push("macd_momentum.enabled");
                }
            }
        }

        // Bollinger Reversion
        if let Some(boll_config) = strategies.get("bollinger_reversion").and_then(|b| b.as_object()) {
            if let Some(config) = &mut state.config.strategies.bollinger_reversion {
                if let Some(enabled) = boll_config.get("enabled").and_then(|e| e.as_bool()) {
                    config.enabled = enabled;
                    updated_fields.push("bollinger_reversion.enabled");
                }
                if let Some(period) = boll_config.get("period").and_then(|p| p.as_u64()) {
                    config.period = period as usize;
                    updated_fields.push("bollinger_reversion.period");
                }
                if let Some(std_dev) = boll_config.get("std_dev").and_then(|s| s.as_f64()) {
                    config.std_dev = std_dev;
                    updated_fields.push("bollinger_reversion.std_dev");
                }
            }
        }
    }

    // Update markets list
    if let Some(markets) = payload.updates.get("markets").and_then(|m| m.as_array()) {
        let new_markets: Vec<String> = markets
            .iter()
            .filter_map(|m| m.as_str().map(|s| s.to_string()))
            .collect();
        if !new_markets.is_empty() {
            state.config.markets.epics = new_markets;
            updated_fields.push("markets");
        }
    }

    // Update trading hours
    if let Some(trading_hours) = payload.updates.get("trading_hours").and_then(|th| th.as_object()) {
        if let Some(start) = trading_hours.get("start").and_then(|s| s.as_str()) {
            state.config.trading_hours.start = start.to_string();
            updated_fields.push("trading_hours.start");
        }
        if let Some(end) = trading_hours.get("end").and_then(|e| e.as_str()) {
            state.config.trading_hours.end = end.to_string();
            updated_fields.push("trading_hours.end");
        }
    }

    // Update general settings
    if let Some(heartbeat_interval_secs) = payload.updates.get("heartbeat_interval_secs") {
        if let Some(val) = heartbeat_interval_secs.as_u64() {
            state.config.general.heartbeat_interval_secs = val;
            updated_fields.push("heartbeat_interval_secs");
        }
    }

    // Broadcast config change event
    for field in &updated_fields {
        let event = EngineEvent::config_changed(field.to_string());
        let _ = app_state.event_tx.send(event);
    }

    Json(json!({
        "success": !updated_fields.is_empty(),
        "message": if updated_fields.is_empty() {
            "No valid fields to update".to_string()
        } else {
            format!("Updated fields: {}", updated_fields.join(", "))
        },
        "updated_fields": updated_fields,
    }))
}

/// WebSocket handler: upgrade connection and stream events
async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl axum::response::IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

/// Handle a single WebSocket connection
async fn handle_socket(mut socket: WebSocket, state: AppState) {
    let mut receiver = state.event_tx.subscribe();

    info_span!("ws_session").in_scope(|| {
        tracing::debug!("New WebSocket connection established");
    });

    while let Ok(event) = receiver.recv().await {
        let msg = match serde_json::to_string(&event) {
            Ok(json) => Message::Text(json),
            Err(e) => {
                error!("Failed to serialize event: {}", e);
                continue;
            }
        };

        if socket.send(msg).await.is_err() {
            // Client disconnected
            break;
        }
    }

    tracing::debug!("WebSocket connection closed");
}

/// Backtest endpoint: run a historical strategy backtest
async fn post_backtest(
    State(app_state): State<AppState>,
    Json(payload): Json<BacktestRequest>,
) -> Json<Value> {
    let epic = payload.epic.clone();
    
    // Get historical candles for the backtest
    let candles = {
        let s = app_state.engine_state.read().await;
        s.markets.history.get_candles(&epic, "HOUR")
            .cloned()
            .unwrap_or_default()
    };

    if candles.len() < 50 {
        return Json(json!({
            "success": false,
            "message": format!("Insufficient historical data for backtest of {}. Need at least 50 candles. Try running for a different market or wait for more data collection.", epic),
        }));
    }

    // Instantiate selected strategy
    let strategy: Box<dyn crate::strategy::traits::Strategy + Send + Sync> = match payload.strategy_name.to_lowercase().as_str() {
        "ma_crossover" => Box::new(MACrossoverStrategy::default()),
        "rsi_reversal" => Box::new(RSIReversalStrategy::default()),
        "macd_momentum" => Box::new(MACDMomentumStrategy::default()),
        "bollinger" => Box::new(BollingerStrategy::default()),
        _ => return Json(json!({
            "success": false,
            "message": format!("Unknown strategy '{}'.", payload.strategy_name),
        })),
    };

    info!("Running backtest for {} using {} ({} candles)", epic, payload.strategy_name, candles.len());

    let mut engine = BacktestEngine::new(payload.initial_balance, payload.risk_pct);
    let result = engine.run(&epic, &candles, &*strategy);

    Json(json!({
        "success": true,
        "strategy": payload.strategy_name,
        "epic": epic,
        "result": result,
        "candle_count": candles.len(),
        "timestamp": Utc::now().to_rfc3339(),
    }))
}

/// Trigger endpoint: manually request a trade for an epic
async fn post_trigger(
    State(app_state): State<AppState>,
    Json(payload): Json<TriggerRequest>,
) -> Json<Value> {
    info!("Manual trigger received for {} {}", payload.epic, payload.direction);

    let event = EngineEvent::trigger_trade(payload.epic.clone(), payload.direction.clone());
    let _ = app_state.event_tx.send(event);

    Json(json!({
        "success": true,
        "message": format!("Trigger request for {} {} sent to engine", payload.epic, payload.direction),
        "timestamp": Utc::now().to_rfc3339(),
    }))
}
