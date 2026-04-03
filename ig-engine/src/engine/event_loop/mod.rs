pub mod analysis;
pub mod handlers;
pub mod learning;
pub mod validation;

use anyhow::Result;
use chrono::{Timelike, Utc};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tokio::time::{interval, Duration};
use tracing::{debug, error, info, warn};

use crate::api::rest_client::IGRestClient;
use crate::api::streaming_client::IGStreamingClient;
use crate::api::traits::TraderAPI;
use crate::data::candle_store;
use crate::engine::state::{AccountState, EngineState, EngineStatus};
use crate::indicators::Candle;
use crate::ipc::events::EngineEvent;
use crate::notifications::telegram::TelegramNotifier;
use crate::risk::RiskManager;
use crate::strategy::traits::{M15Strategy, Strategy};
use crate::strategy::{
    bollinger::BollingerStrategy, ensemble::EnsembleVoter,
    m15_bollinger_reversion::M15BollingerReversionStrategy,
    m15_ema_microtrend::M15EmaMicrotrendStrategy, m15_momentum_burst::M15MomentumBurstStrategy,
    ma_crossover::MACrossoverStrategy, macd_momentum::MACDMomentumStrategy,
    rsi_reversal::RSIReversalStrategy, stochastic_momentum::StochasticMomentumStrategy,
};

pub use analysis::analyze_market;
pub use analysis::analyze_market_m15;
pub use handlers::handle_position_monitoring;
pub use learning::build_learning_snapshot;
pub use validation::{validate_config, validate_live_readiness};

/// Main engine event loop
pub async fn run(
    state: Arc<RwLock<EngineState>>,
    event_tx: broadcast::Sender<EngineEvent>,
) -> Result<()> {
    let config = {
        let s = state.read().await;
        s.config.clone()
    };

    info!(
        "Engine starting in {:?} mode with {} markets",
        config.general.mode,
        config.markets.epics.len()
    );

    if let Err(e) = validate_config(&config) {
        error!("❌ Invalid engine configuration: {}", e);
        return Err(e);
    }
    info!("✅ Config validation passed");

    let telegram = TelegramNotifier::new(&config.notifications.telegram);

    let api_key = std::env::var("IG_API_KEY")
        .map(|v| v.trim().to_string())
        .map_err(|_| anyhow::anyhow!("IG_API_KEY environment variable not set"))?;

    let identifier = std::env::var("IG_IDENTIFIER")
        .map(|v| v.trim().to_string())
        .map_err(|_| anyhow::anyhow!("IG_IDENTIFIER environment variable not set"))?;

    let password = std::env::var("IG_PASSWORD")
        .map(|v| v.trim().to_string())
        .map_err(|_| anyhow::anyhow!("IG_PASSWORD environment variable not set"))?;

    let is_demo = config.ig.environment == "demo";
    let mut client = match IGRestClient::new(
        api_key,
        identifier,
        password,
        is_demo,
        config.ig.rate_limit_per_minute,
    )
    .await
    {
        Ok(c) => {
            info!("Successfully authenticated with IG API");
            c
        }
        Err(e) => {
            error!("Authentication failed: {}", e);
            {
                let mut s = state.write().await;
                s.status = EngineStatus::Error;
            }
            return Err(e);
        }
    };

    {
        let mut s = state.write().await;
        s.status = EngineStatus::Running;
        s.started_at = Some(Utc::now());
    }
    let _ = event_tx.send(EngineEvent::status_change(
        "starting".into(),
        "running".into(),
    ));
    info!("Engine status set to Running");

    // Send Telegram startup ping to verify bot connectivity
    let mode_str = format!("{:?}", config.general.mode);
    telegram
        .send_startup_ping(&mode_str, &config.markets.epics)
        .await;

    // Spawn Telegram command listener for /status and /positions
    let tg_listener = telegram.clone();
    let state_listener = state.clone();
    tokio::spawn(async move {
        tg_listener.start_listener(state_listener).await;
    });

    let ls_endpoint = client.lightstreamer_endpoint().unwrap_or("").to_string();
    let acct_id = client.account_id().unwrap_or("").to_string();

    {
        let mut s = state.write().await;
        s.session.ig_session_token = client.cst().map(|s| s.to_string());
        s.session.ig_security_token = client.security_token().map(|s| s.to_string());
    }

    if !ls_endpoint.is_empty() && !acct_id.is_empty() {
        let epics_clone = config.markets.epics.clone();
        let acct_id_clone = acct_id.clone();
        let state_clone = state.clone();
        let _spawn_event_rx = event_tx.subscribe();
        let streaming_shutdown = Arc::new(tokio::sync::Notify::new());
        let loop_shutdown = streaming_shutdown.clone();

        // Spawn the state-update worker ONCE — it outlives all reconnect attempts.
        // Passing clones of update_tx into each new IGStreamingClient means the
        // single worker task processes all incoming ticks regardless of reconnects.
        let update_tx =
            crate::api::streaming_client::spawn_state_worker(state.clone(), event_tx.clone());

        tokio::spawn(async move {
            loop {
                // 1. Get tokens
                let (cst_token, sec_token) = {
                    let s = state_clone.read().await;
                    (
                        s.session.ig_session_token.clone().unwrap_or_default(),
                        s.session.ig_security_token.clone().unwrap_or_default(),
                    )
                };

                if cst_token.is_empty() || sec_token.is_empty() {
                    warn!("Missing IG tokens, waiting before Lightstreamer reconnect...");
                    tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
                    continue;
                }

                // 2. Connect
                match IGStreamingClient::new(
                    &ls_endpoint,
                    &acct_id_clone,
                    &cst_token,
                    &sec_token,
                    update_tx.clone(),
                ) {
                    Ok(mut streaming) => {
                        streaming.subscribe_prices(&epics_clone);
                        streaming.subscribe_account(&acct_id_clone);
                        streaming.subscribe_trades(&acct_id_clone);

                        // Share the shutdown notify with the client
                        streaming.set_shutdown_notify(loop_shutdown.clone());

                        info!("Connecting to IG Lightstreamer...");
                        if let Err(e) = streaming.connect().await {
                            error!("Lightstreamer connection error or stream ended: {}. Reconnecting in 10s...", e);
                        } else {
                            warn!("Lightstreamer connection ended cleanly. Reconnecting in 10s...");
                        }
                    }
                    Err(e) => {
                        error!(
                            "Failed to create Lightstreamer client: {}. Retrying in 10s...",
                            e
                        );
                    }
                }

                // 3. Mandatory sleep between reconnect attempts to prevent tight loops
                tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
            }
        });
        info!(
            "Lightstreamer streaming task spawned for {} markets (with auto-reconnect)",
            config.markets.epics.len()
        );
    } else {
        warn!("Lightstreamer endpoint or account ID not available — falling back to REST polling only");
    }

    let mut strategies: Vec<Box<dyn Strategy + Send + Sync>> = Vec::new();

    if let Some(ma_cfg) = &config.strategies.ma_crossover {
        if ma_cfg.enabled {
            strategies.push(Box::new(MACrossoverStrategy::new(
                ma_cfg.short_period,
                ma_cfg.long_period,
                ma_cfg.require_adx_above,
                ma_cfg.weight,
                ma_cfg
                    .atr_sl_multiplier
                    .unwrap_or(config.strategies.default_atr_sl_multiplier),
                ma_cfg
                    .atr_tp_multiplier
                    .unwrap_or(config.strategies.default_atr_tp_multiplier),
                ma_cfg.trailing_stop_pips,
            )));
            info!("MA Crossover strategy enabled");
        }
    }

    if let Some(rsi_cfg) = &config.strategies.rsi_divergence {
        if rsi_cfg.enabled {
            strategies.push(Box::new(RSIReversalStrategy::new(
                rsi_cfg.period,
                rsi_cfg.overbought,
                rsi_cfg.oversold,
                rsi_cfg.weight,
                rsi_cfg.detect_divergence,
                rsi_cfg
                    .atr_sl_multiplier
                    .unwrap_or(config.strategies.default_atr_sl_multiplier),
                rsi_cfg
                    .atr_tp_multiplier
                    .unwrap_or(config.strategies.default_atr_tp_multiplier),
                rsi_cfg.trailing_stop_pips,
            )));
            info!("RSI Reversal strategy enabled");
        }
    }

    if let Some(macd_cfg) = &config.strategies.macd_momentum {
        if macd_cfg.enabled {
            strategies.push(Box::new(MACDMomentumStrategy::new(
                macd_cfg.weight,
                macd_cfg
                    .atr_sl_multiplier
                    .unwrap_or(config.strategies.default_atr_sl_multiplier),
                macd_cfg
                    .atr_tp_multiplier
                    .unwrap_or(config.strategies.default_atr_tp_multiplier),
                macd_cfg.trailing_stop_pips,
            )));
            info!("MACD Momentum strategy enabled");
        }
    }

    if let Some(bollinger_cfg) = &config.strategies.bollinger_reversion {
        if bollinger_cfg.enabled {
            strategies.push(Box::new(BollingerStrategy::new(
                bollinger_cfg.period,
                bollinger_cfg.std_dev,
                bollinger_cfg.weight,
                bollinger_cfg
                    .atr_sl_multiplier
                    .unwrap_or(config.strategies.default_atr_sl_multiplier),
                bollinger_cfg
                    .atr_tp_multiplier
                    .unwrap_or(config.strategies.default_atr_tp_multiplier),
                bollinger_cfg.trailing_stop_pips,
            )));
            info!("Bollinger Reversion strategy enabled");
        }
    }

    if let Some(mtf_cfg) = &config.strategies.multi_timeframe {
        if mtf_cfg.enabled {
            strategies.push(Box::new(
                crate::strategy::multi_timeframe::MultiTimeframeStrategy::new(
                    mtf_cfg.trend_tf.clone(),
                    mtf_cfg.signal_tf.clone(),
                    mtf_cfg.entry_tf.clone(),
                    mtf_cfg.weight,
                    config.strategies.default_atr_sl_multiplier,
                    config.strategies.default_atr_tp_multiplier,
                    mtf_cfg.trailing_stop_pips,
                ),
            ));
            info!("Multi-Timeframe strategy enabled");
        }
    }

    if let Some(stoch_cfg) = &config.strategies.stochastic_momentum {
        if stoch_cfg.enabled {
            strategies.push(Box::new(StochasticMomentumStrategy::new(
                stoch_cfg.weight,
                stoch_cfg.overbought,
                stoch_cfg.oversold,
                stoch_cfg
                    .atr_sl_multiplier
                    .unwrap_or(config.strategies.default_atr_sl_multiplier),
                stoch_cfg
                    .atr_tp_multiplier
                    .unwrap_or(config.strategies.default_atr_tp_multiplier),
                stoch_cfg.trailing_stop_pips,
            )));
            info!("Stochastic Momentum strategy enabled");
        }
    }

    let mut ensemble = EnsembleVoter::new(
        config.strategies.min_consensus,
        config.strategies.min_avg_strength,
    );

    // Strategy weight keys MUST match the string returned by each strategy's name() method.
    if let Some(ma_cfg) = &config.strategies.ma_crossover {
        if ma_cfg.enabled {
            ensemble.set_strategy_weight("MA_Crossover".to_string(), ma_cfg.weight);
        }
    }
    if let Some(rsi_cfg) = &config.strategies.rsi_divergence {
        if rsi_cfg.enabled {
            ensemble.set_strategy_weight("RSI_Reversal".to_string(), rsi_cfg.weight);
        }
    }
    if let Some(macd_cfg) = &config.strategies.macd_momentum {
        if macd_cfg.enabled {
            ensemble.set_strategy_weight("MACD_Momentum".to_string(), macd_cfg.weight);
        }
    }
    if let Some(bollinger_cfg) = &config.strategies.bollinger_reversion {
        if bollinger_cfg.enabled {
            ensemble.set_strategy_weight("Bollinger_Bands".to_string(), bollinger_cfg.weight);
        }
    }
    if let Some(mtf_cfg) = &config.strategies.multi_timeframe {
        if mtf_cfg.enabled {
            ensemble.set_strategy_weight("Multi_Timeframe".to_string(), mtf_cfg.weight);
        }
    }
    if let Some(stoch_cfg) = &config.strategies.stochastic_momentum {
        if stoch_cfg.enabled {
            ensemble.set_strategy_weight("Stochastic_Momentum".to_string(), stoch_cfg.weight);
        }
    }
    // Gold sentiment signal — starts at weight 1.0; adaptive manager may tune over time
    ensemble.set_strategy_weight("Gold_Sentiment".to_string(), 1.0);

    info!(
        "Ensemble voter configured with {} strategies",
        strategies.len()
    );

    // --- M15 strategies (Phase 14) ---
    let mut m15_strategies: Vec<Box<dyn M15Strategy + Send + Sync>> = Vec::new();

    if let Some(cfg) = &config.strategies.m15_momentum_burst {
        if cfg.enabled {
            m15_strategies.push(Box::new(M15MomentumBurstStrategy::new(
                cfg.weight,
                cfg.rsi_min,
                cfg.rsi_max,
                cfg.atr_sl_multiplier
                    .unwrap_or(config.strategies.default_atr_sl_multiplier * 0.75),
                cfg.atr_tp_multiplier
                    .unwrap_or(config.strategies.default_atr_tp_multiplier * 0.5),
            )));
            info!("M15 MomentumBurst strategy enabled");
        }
    }
    if let Some(cfg) = &config.strategies.m15_ema_microtrend {
        if cfg.enabled {
            m15_strategies.push(Box::new(M15EmaMicrotrendStrategy::new(
                cfg.weight,
                cfg.atr_sl_multiplier
                    .unwrap_or(config.strategies.default_atr_sl_multiplier * 0.75),
                cfg.atr_tp_multiplier
                    .unwrap_or(config.strategies.default_atr_tp_multiplier * 0.5),
            )));
            info!("M15 EmaMicrotrend strategy enabled");
        }
    }
    if let Some(cfg) = &config.strategies.m15_bollinger_reversion {
        if cfg.enabled {
            m15_strategies.push(Box::new(M15BollingerReversionStrategy::new(
                cfg.weight,
                cfg.percent_b_threshold,
                cfg.rsi_threshold,
                cfg.h1_rsi_confirm,
                cfg.atr_sl_multiplier
                    .unwrap_or(config.strategies.default_atr_sl_multiplier * 0.75),
            )));
            info!("M15 BollingerReversion strategy enabled");
        }
    }

    let m15_ensemble = EnsembleVoter::new(
        config.strategies.m15_min_consensus,
        config.strategies.m15_min_avg_strength,
    );

    if !m15_strategies.is_empty() {
        info!(
            "M15 ensemble voter configured with {} strategies",
            m15_strategies.len()
        );
    }

    let mut risk_config = config.risk.clone();
    risk_config.trading_hours_utc = {
        let start: u32 = config
            .trading_hours
            .start
            .split(':')
            .next()
            .unwrap_or("0")
            .parse()
            .unwrap_or(0);
        let end: u32 = config
            .trading_hours
            .end
            .split(':')
            .next()
            .unwrap_or("16")
            .parse()
            .unwrap_or(16);
        Some((start, end))
    };

    let mut risk_manager = RiskManager::new(risk_config);
    info!("Risk manager initialized");

    let order_manager = crate::engine::order_manager::OrderManager::new(
        crate::engine::order_manager::OrderManagerConfig {
            confirm_timeout_ms: config.ig.confirm_timeout_ms,
            confirm_max_retries: config.ig.confirm_max_retries,
            guaranteed_stop: config.risk.limited_risk_account,
        },
    );
    info!("Order manager initialized");

    let scorecard = crate::learning::scorecard::StrategyScorecard::new(50);
    let base_weights: HashMap<String, f64> = ensemble.strategy_weights.clone();
    let weight_manager = crate::learning::adaptive_weights::AdaptiveWeightManager::new(
        base_weights,
        crate::learning::adaptive_weights::AdaptiveConfig::default(),
    );
    {
        let mut s = state.write().await;
        s.learning.scorecard = Some(scorecard);
        s.learning.weight_manager = Some(weight_manager);
    }
    info!("🧠 Adaptive learning system initialized (window=50 trades, recalc every 10)");

    match client.get_accounts().await {
        Ok(accounts_resp) => {
            if let Some(acct) = accounts_resp.accounts.first() {
                let mut s = state.write().await;
                s.account = AccountState {
                    balance: acct.balance.balance,
                    available: acct.balance.available,
                    margin: acct.balance.deposit,
                    equity: acct.balance.balance + acct.balance.profit_loss,
                    pnl: acct.balance.profit_loss,
                    deposit: acct.balance.deposit,
                    currency: acct.currency.clone(),
                };
                info!(
                    "💰 Account loaded: balance={:.2} {}, available={:.2}, P&L={:.2}",
                    acct.balance.balance,
                    acct.currency,
                    acct.balance.available,
                    acct.balance.profit_loss
                );
            }
        }
        Err(e) => {
            warn!("Failed to fetch account info on startup: {}. Balance will be 0 until next refresh.", e);
        }
    }

    // --- Live readiness checks (only in live mode, runs after balance is known) ---
    {
        let balance = {
            let s = state.read().await;
            s.account.balance
        };
        if let Err(e) = validate_live_readiness(&config, balance) {
            error!("❌ Live readiness check failed: {}", e);
            return Err(e);
        }
    }

    // --- Sync existing positions from IG ---
    match client.get_positions().await {
        Ok(pos_resp) => {
            let mut s = state.write().await;
            for wrapper in pos_resp.positions {
                let ig_pos = wrapper.position;
                let ig_market = wrapper.market;

                let direction = match ig_pos.direction.to_uppercase().as_str() {
                    "BUY" => crate::engine::state::Direction::Buy,
                    _ => crate::engine::state::Direction::Sell,
                };

                let position = crate::engine::state::Position {
                    deal_id: ig_pos.deal_id,
                    deal_reference: "".to_string(), // Not available in list response
                    epic: ig_market.epic,
                    direction,
                    size: ig_pos.size,
                    open_price: ig_pos.level,
                    current_price: ig_pos.level,
                    stop_loss: ig_pos.stop_level,
                    take_profit: ig_pos.limit_level,
                    trailing_stop: None,
                    pnl: 0.0, // Updated by monitor loop later
                    currency: ig_pos.currency,
                    strategy: "Existing".to_string(),
                    opened_at: {
                        let date_str = ig_pos.created_date.replace("T", " ");
                        let clean_date = date_str.split('.').next().unwrap_or(&date_str);
                        chrono::NaiveDateTime::parse_from_str(clean_date, "%Y-%m-%d %H:%M:%S")
                            .map(|dt| dt.and_utc())
                            .unwrap_or_else(|_| chrono::Utc::now())
                    },
                    is_virtual: false,
                    opened_in_regime: None, // birth regime unknown for pre-existing positions
                };
                s.trades.active.push(position);
            }
            info!(
                "📥 Synced {} existing positions from IG",
                s.trades.active.len()
            );
        }
        Err(e) => {
            warn!("Failed to sync existing positions on startup: {}", e);
        }
    }

    // --- Crash-recovery reconciliation ---
    // Compare the positions persisted from the previous session against IG live
    // positions just synced above. Any deal that was open last session but is no
    // longer in IG live was closed while the engine was offline (SL/TP hit or
    // manual close). We can't reconstruct P&L here so we alert via Telegram.
    {
        use crate::engine::state::EngineState;
        use std::collections::HashSet;

        let persisted = EngineState::load_persisted_positions();
        if !persisted.is_empty() {
            let live_deal_ids: HashSet<String> = {
                let s = state.read().await;
                s.trades.active.iter().map(|p| p.deal_id.clone()).collect()
            };
            let offline_closed: Vec<_> = persisted
                .into_iter()
                .filter(|p| !live_deal_ids.contains(&p.deal_id))
                .collect();
            if !offline_closed.is_empty() {
                warn!(
                    "⚠️  Recovery: {} position(s) closed while engine was offline — P&L not tracked:",
                    offline_closed.len()
                );
                let mut tg_lines = Vec::new();
                for p in &offline_closed {
                    warn!(
                        "  → {} {:?} deal_id={} opened={} — check IG portal for P&L",
                        p.epic,
                        p.direction,
                        p.deal_id,
                        p.opened_at.format("%Y-%m-%d %H:%M UTC")
                    );
                    tg_lines.push(format!(
                        "• {} {:?} ({})",
                        crate::engine::state::get_instrument_name(&p.epic),
                        p.direction,
                        p.opened_at.format("%H:%M UTC")
                    ));
                }
                let tg = crate::notifications::telegram::TelegramNotifier::new(
                    &config.notifications.telegram,
                );
                let msg = format!(
                    "⚠️ <b>Engine restarted — {} position(s) closed while offline</b>\nP&amp;L not recorded — check IG portal:\n{}",
                    offline_closed.len(),
                    tg_lines.join("\n")
                );
                tokio::spawn(async move {
                    let _ = tg.send_message(&msg).await;
                });
            } else {
                info!("✅ Recovery check: all previous positions still open or already synced");
            }
        }
    }

    info!(
        "Warming up price history (HOUR resolution) for {} markets — disk-first strategy",
        config.markets.epics.len()
    );

    for (idx, epic) in config.markets.epics.iter().enumerate() {
        info!(
            "📊 Warming up [{}/{}] {} ...",
            idx + 1,
            config.markets.epics.len(),
            epic
        );

        // Step 1: Try loading from disk cache first
        let disk_candles = candle_store::load_from_disk(epic, "HOUR");
        let disk_count = disk_candles.len();

        // Step 2: If disk has enough candles (≥ 210 to satisfy MA 200), skip REST API
        let candles = if disk_count >= 210 {
            info!(
                "  ✓ Loaded {} candles from disk for {} [HOUR] — REST API fetch skipped",
                disk_count, epic
            );
            disk_candles
        } else {
            // Try REST API to get 250 candles
            match client.get_price_history(epic, "HOUR", 250).await {
                Ok(price_response) => {
                    // Log the raw snapshotTime from the first candle so we can
                    // confirm the exact format IG's API returns.
                    if let Some(first) = price_response.prices.first() {
                        info!(
                            "  ℹ snapshotTime sample for {}: {:?}",
                            epic, first.snapshot_time
                        );
                    }

                    let mut parse_failures = 0usize;
                    let api_candles: Vec<Candle> = price_response
                        .prices
                        .iter()
                        .map(|p| {
                            // IG REST API snapshotTime format variants observed:
                            //   "YYYY:MM:DD-HH:mm:ss"      (Confirmed March 2026)
                            //   "DD/MM/YYYY HH:mm:ss:SSS"  (British date, colon-ms suffix)
                            //   "YYYY/MM/DD HH:mm:ss:SSS"  (ISO date, colon-ms suffix)
                            //   "YYYY-MM-DDTHH:mm:ss+00:00" (RFC3339, mock client only)
                            let st = &p.snapshot_time;
                            // Strip ":SSS" or ":mmm" millisecond suffix if present (length > 19)
                            let trimmed = if st.len() > 19 {
                                &st[..19]
                            } else {
                                st.as_str()
                            };

                            let timestamp = chrono::DateTime::parse_from_rfc3339(st)
                                .map(|dt| dt.timestamp())
                                // IG specific: "YYYY:MM:DD-HH:mm:ss"
                                .or_else(|_| {
                                    chrono::NaiveDateTime::parse_from_str(
                                        trimmed,
                                        "%Y:%m:%d-%H:%M:%S",
                                    )
                                    .map(|dt| dt.and_utc().timestamp())
                                })
                                // British date: "DD/MM/YYYY HH:mm:ss"
                                .or_else(|_| {
                                    chrono::NaiveDateTime::parse_from_str(
                                        trimmed,
                                        "%d/%m/%Y %H:%M:%S",
                                    )
                                    .map(|dt| dt.and_utc().timestamp())
                                })
                                // ISO date: "YYYY/MM/DD HH:mm:ss"
                                .or_else(|_| {
                                    chrono::NaiveDateTime::parse_from_str(
                                        trimmed,
                                        "%Y/%m/%d %H:%M:%S",
                                    )
                                    .map(|dt| dt.and_utc().timestamp())
                                })
                                .unwrap_or_else(|_| {
                                    parse_failures += 1;
                                    Utc::now().timestamp()
                                });

                            Candle {
                                timestamp,
                                open: p.open_price.bid,
                                high: p.high_price.bid,
                                low: p.low_price.bid,
                                close: p.close_price.bid,
                                volume: p.last_traded_volume.unwrap_or(0.0) as u64,
                            }
                        })
                        .collect();
                    if parse_failures > 0 {
                        warn!(
                            "  ⚠ {}/{} candle timestamps still failed to parse for {} (see snapshotTime sample above)",
                            parse_failures, api_candles.len(), epic
                        );
                    } else {
                        info!(
                            "  ✓ All {} snapshotTime values parsed successfully for {}",
                            api_candles.len(),
                            epic
                        );
                    }

                    // Merge disk + API candles (dedup, sort, trim to 1000)
                    let merged = if disk_candles.is_empty() {
                        api_candles.clone()
                    } else {
                        candle_store::merge_candles(disk_candles, api_candles.clone())
                    };
                    info!(
                        "  ✓ {} warmed up — {} candles from API, {} total after merge with disk",
                        epic,
                        api_candles.len(),
                        merged.len()
                    );
                    merged
                }
                Err(e) => {
                    if disk_candles.is_empty() {
                        warn!(
                            "  ✗ No candles available for {} — REST API failed: {}",
                            epic, e
                        );
                        continue;
                    }
                    warn!(
                        "  ⚠ REST API failed for {}: {} — using {} candles from disk",
                        epic, e, disk_count
                    );
                    disk_candles
                }
            }
        };

        // Step 3: Load candles into store and feed indicators
        {
            let mut s = state.write().await;
            s.markets.history.warm_up(epic, "HOUR", candles.clone());
            // Persist merged result back to disk so next restart has the best data
            s.markets.history.persist_series(epic, "HOUR");

            if let Some(indicator_set_map) = s.markets.indicators.get_mut(epic) {
                if let Some(indicator_set) = indicator_set_map.get_mut("HOUR") {
                    for candle in &candles {
                        indicator_set.update(candle);
                    }
                }
            }
        }
    }
    info!("🚀 All markets warmed up — engine ready");

    // --- M15 warmup (Phase 14) ---
    // Fetch 250 MINUTE_15 bars per epic to warm up M15 indicator sets.
    // Only runs when at least one M15 strategy is enabled in config.
    let m15_enabled = config
        .strategies
        .m15_momentum_burst
        .as_ref()
        .is_some_and(|c| c.enabled)
        || config
            .strategies
            .m15_ema_microtrend
            .as_ref()
            .is_some_and(|c| c.enabled)
        || config
            .strategies
            .m15_bollinger_reversion
            .as_ref()
            .is_some_and(|c| c.enabled);

    if m15_enabled {
        info!(
            "Warming up M15 indicators (MINUTE_15 resolution) for {} markets...",
            config.markets.epics.len()
        );
        for (idx, epic) in config.markets.epics.iter().enumerate() {
            info!(
                "  M15 [{}/{}] {} ...",
                idx + 1,
                config.markets.epics.len(),
                epic
            );

            // Try disk cache first
            let disk_candles = candle_store::load_from_disk(epic, "MINUTE_15");
            let candles = if disk_candles.len() >= 210 {
                info!(
                    "  {} M15 candles from disk for {}",
                    disk_candles.len(),
                    epic
                );
                disk_candles
            } else {
                match client.get_price_history(epic, "MINUTE_15", 250).await {
                    Ok(price_response) => {
                        let api_candles: Vec<crate::indicators::Candle> = price_response
                            .prices
                            .iter()
                            .map(|p| {
                                let st = &p.snapshot_time;
                                let trimmed = if st.len() > 19 {
                                    &st[..19]
                                } else {
                                    st.as_str()
                                };
                                let timestamp = chrono::DateTime::parse_from_rfc3339(st)
                                    .map(|dt| dt.timestamp())
                                    .or_else(|_| {
                                        chrono::NaiveDateTime::parse_from_str(
                                            trimmed,
                                            "%Y:%m:%d-%H:%M:%S",
                                        )
                                        .map(|dt| dt.and_utc().timestamp())
                                    })
                                    .or_else(|_| {
                                        chrono::NaiveDateTime::parse_from_str(
                                            trimmed,
                                            "%d/%m/%Y %H:%M:%S",
                                        )
                                        .map(|dt| dt.and_utc().timestamp())
                                    })
                                    .or_else(|_| {
                                        chrono::NaiveDateTime::parse_from_str(
                                            trimmed,
                                            "%Y/%m/%d %H:%M:%S",
                                        )
                                        .map(|dt| dt.and_utc().timestamp())
                                    })
                                    .unwrap_or_else(|_| Utc::now().timestamp());
                                crate::indicators::Candle {
                                    timestamp,
                                    open: p.open_price.bid,
                                    high: p.high_price.bid,
                                    low: p.low_price.bid,
                                    close: p.close_price.bid,
                                    volume: p.last_traded_volume.unwrap_or(0.0) as u64,
                                }
                            })
                            .collect();
                        let merged = if disk_candles.is_empty() {
                            api_candles
                        } else {
                            candle_store::merge_candles(disk_candles, api_candles)
                        };
                        info!("  {} M15 candles for {}", merged.len(), epic);
                        merged
                    }
                    Err(e) => {
                        if disk_candles.is_empty() {
                            warn!("  M15 warmup failed for {}: {}", epic, e);
                            vec![]
                        } else {
                            warn!(
                                "  M15 API failed for {}: {} — using {} disk candles",
                                epic,
                                e,
                                disk_candles.len()
                            );
                            disk_candles
                        }
                    }
                }
            };

            if !candles.is_empty() {
                let mut s = state.write().await;
                s.markets
                    .history
                    .warm_up(epic, "MINUTE_15", candles.clone());
                // Persist to disk so future restarts use disk-first and skip the API call
                s.markets.history.persist_series(epic, "MINUTE_15");
                // Insert MINUTE_15 indicator set if not already present
                let tf_map = s.markets.indicators.entry(epic.clone()).or_default();
                let m15_indicators = tf_map
                    .entry("MINUTE_15".to_string())
                    .or_insert_with(crate::indicators::IndicatorSet::default_config);
                for candle in &candles {
                    m15_indicators.update(candle);
                }
                let warmed = m15_indicators.is_warmed_up();
                info!("  M15 indicators for {} — warmed_up={}", epic, warmed);
            }
        }
        info!("M15 warmup complete");
    }

    let mut position_monitor_interval = interval(Duration::from_secs(5));
    let mut session_refresh_interval =
        interval(Duration::from_secs(config.ig.session_refresh_mins * 60));
    let mut daily_reset_interval = interval(Duration::from_secs(60));
    let mut heartbeat_interval =
        interval(Duration::from_secs(config.general.heartbeat_interval_secs));
    let mut daily_summary_interval = interval(Duration::from_secs(60));
    // IG client sentiment — refreshed every 15 minutes for all configured context_market_ids
    let mut sentiment_poll_interval = interval(Duration::from_secs(15 * 60));
    // BOT_ACTIVE watchlist sync — dynamically add epics from IG every hour (10.4)
    let mut watchlist_sync_interval = interval(Duration::from_secs(60 * 60));
    // Overnight financing — fetch from IG /history/transactions every hour.
    // IG applies financing once per day; polling hourly ensures we capture it promptly.
    let mut financing_poll_interval = interval(Duration::from_secs(60 * 60));
    let mut last_summary_date = String::new();
    // Track the most recent bar-start timestamp per epic that we ran analysis on.
    // Strategy evaluation only runs when indicators have actually advanced (new bar closed),
    // avoiding hundreds of redundant evaluations on intra-bar ticks.
    let mut last_analyzed_bar_ts: HashMap<String, i64> = HashMap::new();

    // M15 refresh — fetches new MINUTE_15 candles every 60s and updates M15 indicators
    // Only active when M15 strategies are configured.
    let mut m15_refresh_interval = if m15_enabled {
        Some(interval(Duration::from_secs(60)))
    } else {
        None
    };
    // Track last M15 candle timestamp per epic (to avoid reprocessing the same bar)
    let mut last_m15_candle_ts: HashMap<String, i64> = HashMap::new();
    // Self-heal backoff: after a rate-limit failure, wait 30 min before retrying API self-heal.
    // During backoff, tick accumulator continues building bars locally — no API needed.
    let mut m15_self_heal_backoff: HashMap<String, std::time::Instant> = HashMap::new();

    info!("Engine event loop started");

    let mut event_rx = event_tx.subscribe();

    loop {
        {
            let s = state.read().await;
            if s.status == EngineStatus::Stopped {
                info!("Engine received stop signal, gracefully shutting down");
                break;
            }
        }

        tokio::select! {
            Ok(event) = event_rx.recv() => {
                match event.event {
                    crate::ipc::events::EventVariant::MarketUpdate { state: market_state } => {
                        let epic = market_state.epic.clone();

                        // Only run full strategy analysis when a new bar has closed for this epic.
                        // The bar_ts advances once per hour; intra-bar ticks carry no new indicator data.
                        let current_bar_ts = {
                            let s = state.read().await;
                            s.markets.bar_accumulator.current_bar_ts(&epic)
                        };
                        let should_analyze = match current_bar_ts {
                            Some(ts) => last_analyzed_bar_ts.get(&epic).copied() != Some(ts),
                            None => false, // No bar yet — wait for first bar close
                        };
                        if !should_analyze {
                            continue;
                        }
                        if let Some(ts) = current_bar_ts {
                            last_analyzed_bar_ts.insert(epic.clone(), ts);
                        }

                        if let Err(e) = analyze_market(
                            &state,
                            &mut client,
                            &strategies,
                            &ensemble,
                            &mut risk_manager,
                            &order_manager,
                            &event_tx,
                            &config,
                            &telegram,
                            Some(epic),
                        ).await {
                            warn!("Error in real-time analysis for {}: {}", market_state.epic, e);
                        }
                    }
                    crate::ipc::events::EventVariant::Shutdown { reason } => {
                        info!("Shutdown signal received: {}. Terminating event loop...", reason);
                        break;
                    }
                    crate::ipc::events::EventVariant::TriggerTrade { epic, direction } => {
                        if let Err(e) = crate::engine::event_loop::analysis::execute_manual_trigger(
                            &state,
                            &mut client,
                            &mut risk_manager,
                            &order_manager,
                            &event_tx,
                            &config,
                            &telegram,
                            epic,
                            direction,
                        ).await {
                            warn!("Manual trigger failed: {}", e);
                        }
                    }
                    _ => {}
                }
            }

            _ = position_monitor_interval.tick() => {
                if let Err(e) = handle_position_monitoring(
                    &state,
                    &mut client,
                    &order_manager,
                    &event_tx,
                    &telegram,
                    &mut ensemble,
                ).await {
                    warn!("Error in position monitoring: {}", e);
                }
            }

            _ = session_refresh_interval.tick() => {
                if let Err(e) = client.refresh_session().await {
                    error!("Proactive session refresh failed: {}", e);
                    let _ = event_tx.send(EngineEvent::status_change(
                        "running".into(),
                        "warning: session lost".into()
                    ));
                } else {
                    info!("IG session refreshed successfully");
                    let mut s = state.write().await;
                    s.session.ig_session_token = client.cst().map(|s| s.to_string());
                    s.session.ig_security_token = client.security_token().map(|s| s.to_string());
                }
            }

            _ = daily_reset_interval.tick() => {
                let did_reset = {
                    let mut s = state.write().await;
                    let prev_date = s.metrics.daily.date.clone();
                    s.check_daily_reset();
                    prev_date != s.metrics.daily.date   // true only when date rolled over
                };
                if did_reset {
                    risk_manager.reset_daily();
                    info!("🔄 Daily counters reset — new trading day (risk_manager + state)");
                }
                debug!("Daily reset check performed");
            }

            _ = daily_summary_interval.tick() => {
                let now_sgt = {
                    let utc = Utc::now();
                    // SAFETY: 8 * 3600 = 28800, always within the valid range (-86399..=86399)
                let sgt_offset = chrono::FixedOffset::east_opt(8 * 3600)
                    .expect("SGT offset 28800 is always valid");
                    utc.with_timezone(&sgt_offset)
                };
                let today = now_sgt.format("%Y-%m-%d").to_string();
                let hour = now_sgt.hour();
                if hour == 21 && today != last_summary_date {
                    let s = state.read().await;
                    let tg = telegram.clone();
                    let trades = s.metrics.daily.trades;
                    let wins = s.metrics.daily.wins;
                    let pnl = s.metrics.daily.pnl;
                    let balance = s.account.balance;
                    let financing_pnl = s.metrics.daily.financing_pnl;
                    drop(s);
                    last_summary_date = today;
                    tokio::spawn(async move {
                        let _ = tg.send_daily_summary(trades, wins, pnl, balance, financing_pnl).await;
                    });
                }
            }

            _ = heartbeat_interval.tick() => {
                let s = state.read().await;
                let positions_count = s.trades.active.len();
                let uptime_secs = s.started_at
                    .map(|s| (Utc::now() - s).num_seconds() as u64)
                    .unwrap_or(0);
                drop(s);

                let _ = event_tx.send(EngineEvent::heartbeat(uptime_secs, positions_count));
            }

            _ = sentiment_poll_interval.tick() => {
                let market_ids: Vec<String> = {
                    let s = state.read().await;
                    s.config.markets.context_market_ids.clone()
                };

                if market_ids.is_empty() {
                    debug!("No context_market_ids configured — skipping sentiment poll");
                } else {
                    let mut updated = 0usize;
                    for market_id in &market_ids {
                        match client.get_client_sentiment(market_id).await {
                            Ok(resp) => {
                                let mut s = state.write().await;
                                s.sentiment.update(
                                    resp.market_id.clone(),
                                    resp.long_position_percentage,
                                    resp.short_position_percentage,
                                );
                                updated += 1;
                            }
                            Err(e) => {
                                warn!("Sentiment poll failed for {}: {}", market_id, e);
                            }
                        }
                    }
                    if updated > 0 {
                        info!("📊 Sentiment updated for {}/{} markets", updated, market_ids.len());
                    }

                    // ── 12.2 Sentiment Velocity Guard ────────────────────────────
                    // After updating all sentiments, check for velocity spikes.
                    // A delta > 0.5 in a single 15-min window indicates a "Breaking News"
                    // event — trigger a 2-hour macro pause on new trade entries.
                    const VELOCITY_THRESHOLD: f64 = 0.5;
                    let velocity_spike = {
                        let s = state.read().await;
                        market_ids.iter().any(|mid| s.sentiment.get_velocity(mid) > VELOCITY_THRESHOLD)
                    };
                    if velocity_spike {
                        // 30-minute pause: covers post-spike shock without consuming the
                        // rest of a day trading session (the original 2-hour window was
                        // excessive; scheduled events are already handled by macro_events).
                        let pause_until = chrono::Utc::now() + chrono::Duration::minutes(30);
                        let mut s = state.write().await;
                        s.metrics.macro_pause_until = Some(pause_until);
                        warn!(
                            "⚡ Sentiment velocity spike detected — macro pause active until {} UTC (30 min)",
                            pause_until.format("%H:%M")
                        );
                        let _ = event_tx.send(crate::ipc::events::EngineEvent::status_change(
                            "running".into(),
                            format!("macro_pause_until:{}", pause_until.timestamp()),
                        ));
                    } else {
                        // Clear expired macro pause automatically
                        let expired = {
                            let s = state.read().await;
                            s.metrics.macro_pause_until
                                .map(|until| chrono::Utc::now() >= until)
                                .unwrap_or(false)
                        };
                        if expired {
                            let mut s = state.write().await;
                            s.metrics.macro_pause_until = None;
                            info!("✅ Macro pause expired — trade entries re-enabled");
                        }
                    }
                }
            }

            _ = watchlist_sync_interval.tick() => {
                // ── 10.4 Watchlist Syncing (BOT_ACTIVE) ──────────────────────────
                // Fetch the "BOT_ACTIVE" watchlist from IG and dynamically add any
                // new epics to the engine's active monitoring list without a restart.
                match client.get_watchlist_by_name("BOT_ACTIVE").await {
                    Ok(markets) => {
                        let new_epics: Vec<String> = markets.markets
                            .into_iter()
                            .map(|m| m.epic)
                            .collect();

                        let mut s = state.write().await;
                        let mut added = 0usize;
                        for epic in &new_epics {
                            if !s.config.markets.epics.contains(epic) {
                                s.config.markets.epics.push(epic.clone());

                                // Initialise indicator map for the new epic
                                let mut tf_map = std::collections::HashMap::new();
                                tf_map.insert("HOUR".to_string(), crate::indicators::IndicatorSet::default_config());
                                s.markets.indicators.insert(epic.clone(), tf_map);

                                info!("📋 BOT_ACTIVE watchlist: added new epic {}", epic);
                                added += 1;
                            }
                        }
                        if added > 0 {
                            info!("📋 Watchlist sync: {} new epic(s) added from BOT_ACTIVE", added);
                        } else {
                            debug!("Watchlist sync: BOT_ACTIVE unchanged ({} epics)", new_epics.len());
                        }
                    }
                    Err(e) => {
                        debug!("Watchlist sync skipped (BOT_ACTIVE not found or error): {}", e);
                    }
                }
            }

            _ = financing_poll_interval.tick() => {
                // ── Overnight Financing Poll ──────────────────────────────────────
                // Fetch today's INTEREST transactions from IG and update DailyStats.
                // IG applies overnight funding once per day (around 22:00–00:00 UTC).
                match client.get_today_financing().await {
                    Ok(net) => {
                        let mut s = state.write().await;
                        s.metrics.daily.financing_pnl = net;
                        if net.abs() > 0.01 {
                            info!("💳 Financing updated: {}{:.2} SGD today",
                                if net >= 0.0 { "+" } else { "" }, net);
                        }
                    }
                    Err(e) => {
                        debug!("Financing poll skipped: {}", e);
                    }
                }
            }

            _ = async {
                if let Some(ref mut interval) = m15_refresh_interval {
                    interval.tick().await
                } else {
                    std::future::pending::<tokio::time::Instant>().await
                }
            } => {
                // Fetch new M15 candles for each epic and update M15 indicators.
                // Self-heal: if indicators not warmed up (startup warmup failed due to rate limit
                // or missing disk cache), fetch full 250-bar history on this tick instead of 5.
                for epic in &config.markets.epics.clone() {
                    let needs_warmup = {
                        let s = state.read().await;
                        s.markets.indicators.get(epic.as_str())
                            .and_then(|tf| tf.get("MINUTE_15"))
                            .map(|ind| !ind.is_warmed_up())
                            .unwrap_or(true)
                    };
                    // Self-heal backoff: if a recent self-heal attempt was rate-limited,
                    // skip the API call and fall through to tick-warmed fallback analysis.
                    // Tick accumulator is building bars locally regardless — no API needed.
                    if needs_warmup {
                        let in_backoff = m15_self_heal_backoff.get(epic.as_str())
                            .map(|t| t.elapsed() < std::time::Duration::from_secs(1800))
                            .unwrap_or(false);
                        if in_backoff {
                            debug!("[M15] {} — self-heal in 30-min backoff (rate limited), tick accumulator is building bars", epic);
                            let is_warmed = {
                                let s = state.read().await;
                                s.markets.indicators.get(epic.as_str())
                                    .and_then(|tf| tf.get("MINUTE_15"))
                                    .map(|ind| ind.is_warmed_up())
                                    .unwrap_or(false)
                            };
                            if is_warmed {
                                if let Err(ae) = analyze_market_m15(
                                    &state,
                                    &mut client,
                                    &m15_strategies,
                                    &m15_ensemble,
                                    &mut risk_manager,
                                    &order_manager,
                                    &event_tx,
                                    &config,
                                    &telegram,
                                ).await {
                                    warn!("[M15] Error in M15 analysis (tick-warmed, backoff) for {}: {}", epic, ae);
                                }
                            }
                            continue;
                        }
                    }

                    let fetch_count = if needs_warmup { 250 } else { 5 };
                    if needs_warmup {
                        info!("[M15] {} — indicators not warmed up, fetching {} bars for self-heal", epic, fetch_count);
                    }
                    match client.get_price_history(epic, "MINUTE_15", fetch_count).await {
                        Ok(price_response) => {
                            let new_candles: Vec<Candle> = price_response.prices.iter().map(|p| {
                                let st = &p.snapshot_time;
                                let trimmed = if st.len() > 19 { &st[..19] } else { st.as_str() };
                                let timestamp = chrono::DateTime::parse_from_rfc3339(st)
                                    .map(|dt| dt.timestamp())
                                    .or_else(|_| chrono::NaiveDateTime::parse_from_str(trimmed, "%Y:%m:%d-%H:%M:%S").map(|dt| dt.and_utc().timestamp()))
                                    .or_else(|_| chrono::NaiveDateTime::parse_from_str(trimmed, "%d/%m/%Y %H:%M:%S").map(|dt| dt.and_utc().timestamp()))
                                    .or_else(|_| chrono::NaiveDateTime::parse_from_str(trimmed, "%Y/%m/%d %H:%M:%S").map(|dt| dt.and_utc().timestamp()))
                                    .unwrap_or_else(|_| Utc::now().timestamp());
                                Candle {
                                    timestamp,
                                    open: p.open_price.bid,
                                    high: p.high_price.bid,
                                    low: p.low_price.bid,
                                    close: p.close_price.bid,
                                    volume: p.last_traded_volume.unwrap_or(0.0) as u64,
                                }
                            }).collect();

                            let last_ts = last_m15_candle_ts.get(epic.as_str()).copied().unwrap_or(0);
                            let truly_new: Vec<&Candle> = new_candles.iter()
                                .filter(|c| c.timestamp > last_ts)
                                .collect();

                            if !truly_new.is_empty() {
                                let newest_ts = truly_new.iter().map(|c| c.timestamp).max().unwrap_or(0);
                                {
                                    let mut s = state.write().await;
                                    // Push new candles into CandleStore history AND indicators
                                    for candle in &truly_new {
                                        s.markets.history.push(epic, "MINUTE_15", (*candle).clone());
                                    }
                                    // Persist updated MINUTE_15 history to disk (disk-first warmup on next restart)
                                    s.markets.history.persist_series(epic, "MINUTE_15");
                                    let tf_map = s.markets.indicators.entry(epic.clone()).or_default();
                                    let m15_set = tf_map.entry("MINUTE_15".to_string()).or_insert_with(crate::indicators::IndicatorSet::default_config);
                                    for candle in &truly_new {
                                        m15_set.update(candle);
                                    }
                                }
                                last_m15_candle_ts.insert(epic.clone(), newest_ts);
                                debug!("[M15] {} — {} new bars, last_ts={}", epic, truly_new.len(), newest_ts);

                                // Run M15 analysis for this epic now that indicators updated
                                if let Err(e) = analyze_market_m15(
                                    &state,
                                    &mut client,
                                    &m15_strategies,
                                    &m15_ensemble,
                                    &mut risk_manager,
                                    &order_manager,
                                    &event_tx,
                                    &config,
                                    &telegram,
                                ).await {
                                    warn!("[M15] Error in M15 analysis for {}: {}", epic, e);
                                }
                            }
                        }
                        Err(e) => {
                            debug!("[M15] Failed to fetch M15 candles for {}: {}", epic, e);
                            // If this was a self-heal attempt, record backoff to stop hammering the API.
                            // Tick accumulator will build the required bars in ~52 hours of operation.
                            if needs_warmup {
                                m15_self_heal_backoff.insert(epic.clone(), std::time::Instant::now());
                                info!(
                                    "[M15] {} — self-heal API failed (rate limited), backing off 30 min. \
                                     Tick accumulator building bars locally — no action needed.",
                                    epic
                                );
                            }
                            // API failed — but if M15 indicators are already warmed up from
                            // tick-built candles, run analysis anyway on current indicator state.
                            let is_warmed = {
                                let s = state.read().await;
                                s.markets.indicators.get(epic.as_str())
                                    .and_then(|tf| tf.get("MINUTE_15"))
                                    .map(|ind| ind.is_warmed_up())
                                    .unwrap_or(false)
                            };
                            if is_warmed {
                                if let Err(ae) = analyze_market_m15(
                                    &state,
                                    &mut client,
                                    &m15_strategies,
                                    &m15_ensemble,
                                    &mut risk_manager,
                                    &order_manager,
                                    &event_tx,
                                    &config,
                                    &telegram,
                                ).await {
                                    warn!("[M15] Error in M15 analysis (tick-warmed) for {}: {}", epic, ae);
                                }
                            }
                        }
                    }
                }
            }

        }
    }

    info!("Engine event loop terminated");

    // Persist candle data to disk before shutdown
    {
        let s = state.read().await;
        s.markets.history.persist_all();
        info!("Candle data persisted to disk");
    }

    // Perform cleanup
    info!("Logging out and closing IG session...");
    if let Err(e) = client.logout().await {
        error!("Logout failed: {}", e);
    } else {
        info!("Cleanly logged out from IG API");
    }

    Ok(())
}
