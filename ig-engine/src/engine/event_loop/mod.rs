pub mod analysis;
pub mod handlers;
pub mod learning;
pub mod validation;

use std::sync::Arc;
use tokio::sync::{RwLock, broadcast};
use tokio::time::{interval, Duration};
use anyhow::Result;
use tracing::{info, warn, error, debug};
use chrono::{Utc, Timelike};
use std::collections::HashMap;

use crate::engine::state::{EngineState, EngineStatus, AccountState};
use crate::api::rest_client::IGRestClient;
use crate::api::streaming_client::IGStreamingClient;
use crate::api::traits::TraderAPI;
use crate::risk::RiskManager;
use crate::strategy::traits::Strategy;
use crate::strategy::{
    ma_crossover::MACrossoverStrategy,
    rsi_reversal::RSIReversalStrategy,
    macd_momentum::MACDMomentumStrategy,
    bollinger::BollingerStrategy,
    ensemble::EnsembleVoter,
};
use crate::indicators::Candle;
use crate::ipc::events::EngineEvent;
use crate::notifications::telegram::TelegramNotifier;

pub use analysis::analyze_market;
pub use handlers::handle_position_monitoring;
pub use learning::build_learning_snapshot;
pub use validation::validate_config;

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

    let telegram = TelegramNotifier::new();

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
    let mut client = match IGRestClient::new(api_key, identifier, password, is_demo).await {
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
    let _ = event_tx.send(EngineEvent::status_change("starting".into(), "running".into()));
    info!("Engine status set to Running");

    // Send Telegram startup ping to verify bot connectivity
    let mode_str = format!("{:?}", config.general.mode);
    telegram.send_startup_ping(&mode_str, &config.markets.epics).await;

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
        let mut spawn_event_rx = event_tx.subscribe();
        let streaming_shutdown = Arc::new(tokio::sync::Notify::new());
        let loop_shutdown = streaming_shutdown.clone();

        // Spawn the state-update worker ONCE — it outlives all reconnect attempts.
        // Passing clones of update_tx into each new IGStreamingClient means the
        // single worker task processes all incoming ticks regardless of reconnects.
        let update_tx = crate::api::streaming_client::spawn_state_worker(
            state.clone(),
            event_tx.clone(),
        );

        tokio::spawn(async move {
            loop {
                // Check for shutdown signal before attempting reconnect
                tokio::select! {
                    Ok(event) = spawn_event_rx.recv() => {
                        if let crate::ipc::events::EventVariant::Shutdown { .. } = event.event {
                            info!("Lightstreamer reconnection loop received shutdown. Terminating.");
                            loop_shutdown.notify_one();
                            break;
                        }
                    }
                    _ = async {
                        let (cst_token, sec_token) = {
                            let s = state_clone.read().await;
                            (
                                s.session.ig_session_token.clone().unwrap_or_default(),
                                s.session.ig_security_token.clone().unwrap_or_default()
                            )
                        };

                        if cst_token.is_empty() || sec_token.is_empty() {
                            warn!("Missing IG tokens, waiting before Lightstreamer reconnect...");
                            tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
                            return;
                        }

                        match IGStreamingClient::new(&ls_endpoint, &acct_id_clone, &cst_token, &sec_token, update_tx.clone()) {
                            Ok(mut streaming) => {
                                streaming.subscribe_prices(&epics_clone);
                                streaming.subscribe_account(&acct_id_clone);
                                streaming.subscribe_trades(&acct_id_clone);

                                // Share the shutdown notify with the client
                                streaming.set_shutdown_notify(loop_shutdown.clone());

                                if let Err(e) = streaming.connect().await {
                                    error!("Lightstreamer connection error or stream ended: {}. Reconnecting in 5s...", e);
                                } else {
                                    warn!("Lightstreamer connection ended cleanly.");
                                }
                            }
                            Err(e) => {
                                error!("Failed to create Lightstreamer client: {}. Retrying in 5s...", e);
                            }
                        }

                        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                    } => {}
                }
            }
        });
        info!("Lightstreamer streaming task spawned for {} markets (with auto-reconnect)", config.markets.epics.len());
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
                ma_cfg.atr_sl_multiplier.unwrap_or(config.strategies.default_atr_sl_multiplier),
                ma_cfg.atr_tp_multiplier.unwrap_or(config.strategies.default_atr_tp_multiplier),
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
                rsi_cfg.atr_sl_multiplier.unwrap_or(config.strategies.default_atr_sl_multiplier),
                rsi_cfg.atr_tp_multiplier.unwrap_or(config.strategies.default_atr_tp_multiplier),
                rsi_cfg.trailing_stop_pips,
            )));
            info!("RSI Reversal strategy enabled");
        }
    }

    if let Some(macd_cfg) = &config.strategies.macd_momentum {
        if macd_cfg.enabled {
            strategies.push(Box::new(MACDMomentumStrategy::new(
                macd_cfg.weight,
                macd_cfg.atr_sl_multiplier.unwrap_or(config.strategies.default_atr_sl_multiplier),
                macd_cfg.atr_tp_multiplier.unwrap_or(config.strategies.default_atr_tp_multiplier),
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
                bollinger_cfg.atr_sl_multiplier.unwrap_or(config.strategies.default_atr_sl_multiplier),
                bollinger_cfg.atr_tp_multiplier.unwrap_or(config.strategies.default_atr_tp_multiplier),
                bollinger_cfg.trailing_stop_pips,
            )));
            info!("Bollinger Reversion strategy enabled");
        }
    }

    if let Some(mtf_cfg) = &config.strategies.multi_timeframe {
        if mtf_cfg.enabled {
            strategies.push(Box::new(crate::strategy::multi_timeframe::MultiTimeframeStrategy::new(
                mtf_cfg.trend_tf.clone(),
                mtf_cfg.signal_tf.clone(),
                mtf_cfg.entry_tf.clone(),
                mtf_cfg.weight,
                config.strategies.default_atr_sl_multiplier,
                config.strategies.default_atr_tp_multiplier,
                mtf_cfg.trailing_stop_pips,
            )));
            info!("Multi-Timeframe strategy enabled");
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
    // Gold sentiment signal — starts at weight 1.0; adaptive manager may tune over time
    ensemble.set_strategy_weight("Gold_Sentiment".to_string(), 1.0);

    info!("Ensemble voter configured with {} strategies", strategies.len());
 
    let mut risk_config = config.risk.clone();
    risk_config.trading_hours_utc = {
        let start: u32 = config.trading_hours.start.split(':').next().unwrap_or("0").parse().unwrap_or(0);
        let end: u32 = config.trading_hours.end.split(':').next().unwrap_or("16").parse().unwrap_or(16);
        Some((start, end))
    };

    let mut risk_manager = RiskManager::new(risk_config);
    info!("Risk manager initialized");

    let order_manager = crate::engine::order_manager::OrderManager::new(crate::engine::order_manager::OrderManagerConfig {
        confirm_timeout_ms: config.ig.confirm_timeout_ms,
        confirm_max_retries: config.ig.confirm_max_retries,
        guaranteed_stop: true,
    });
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
                    acct.balance.balance, acct.currency, acct.balance.available, acct.balance.profit_loss
                );
            }
        }
        Err(e) => {
            warn!("Failed to fetch account info on startup: {}. Balance will be 0 until next refresh.", e);
        }
    }

    info!(
        "Fetching initial price history (250 candles, HOUR resolution) for {} markets",
        config.markets.epics.len()
    );

    for (idx, epic) in config.markets.epics.iter().enumerate() {
        info!("📊 Warming up [{}/{}] {} ...", idx + 1, config.markets.epics.len(), epic);
        match client.get_price_history(epic, "HOUR", 250).await {
            Ok(price_response) => {
                let candles: Vec<Candle> = price_response
                    .prices
                    .iter()
                    .map(|p| {
                        let timestamp = chrono::NaiveDateTime::parse_from_str(
                            &p.snapshot_time, 
                            "%Y/%m/%d %H:%M:%S"
                        ).map(|dt| dt.and_utc().timestamp())
                        .unwrap_or_else(|_| Utc::now().timestamp());

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

                let mut s = state.write().await;
                s.markets.history.warm_up(epic, "HOUR", candles.clone());
                
                if let Some(indicator_set_map) = s.markets.indicators.get_mut(epic) {
                    if let Some(indicator_set) = indicator_set_map.get_mut("HOUR") {
                        for candle in &candles {
                            indicator_set.update(candle);
                        }
                    }
                    info!("  ✓ {} warmed up — {} candles loaded", epic, candles.len());
                }
            }
            Err(e) => {
                warn!("  ✗ Failed to fetch price history for {}: {}", epic, e);
            }
        }
    }
    info!("🚀 All markets warmed up — engine ready");

    let mut position_monitor_interval = interval(Duration::from_secs(5));
    let mut session_refresh_interval = interval(Duration::from_secs(config.ig.session_refresh_mins * 60));
    let mut daily_reset_interval = interval(Duration::from_secs(60));
    let mut heartbeat_interval = interval(Duration::from_secs(config.general.heartbeat_interval_secs));
    let mut daily_summary_interval = interval(Duration::from_secs(60));
    let mut last_summary_date = String::new();
    // Track the most recent bar-start timestamp per epic that we ran analysis on.
    // Strategy evaluation only runs when indicators have actually advanced (new bar closed),
    // avoiding hundreds of redundant evaluations on intra-bar ticks.
    let mut last_analyzed_bar_ts: HashMap<String, i64> = HashMap::new();

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
                let mut s = state.write().await;
                s.check_daily_reset();
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
                    drop(s);
                    last_summary_date = today;
                    tokio::spawn(async move {
                        let _ = tg.send_daily_summary(trades, wins, pnl, balance).await;
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

        }
    }

    info!("Engine event loop terminated");

    // Perform cleanup
    info!("Logging out and closing IG session...");
    if let Err(e) = client.logout().await {
        error!("Logout failed: {}", e);
    } else {
        info!("Cleanly logged out from IG API");
    }

    Ok(())
}
