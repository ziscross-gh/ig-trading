use std::sync::Arc;
use tokio::sync::{RwLock, broadcast};
use anyhow::Result;
use tracing::{info, warn, error, debug};
use chrono::Utc;

use crate::engine::config::{EngineConfig, EngineMode};
use crate::engine::state::{EngineState, Direction, Position, Signal, get_instrument_name};
use crate::api::rest_client::IGRestClient;
use crate::risk::RiskManager;
use crate::strategy::traits::Strategy;
use crate::strategy::ensemble::EnsembleVoter;
use crate::ipc::events::EngineEvent;
use crate::notifications::telegram::TelegramNotifier;

/// Analyze one or more markets and potentially execute trades
#[allow(clippy::too_many_arguments)]
// TODO: bundle args into an AnalysisContext struct to reduce parameter count
pub async fn analyze_market(
    state: &Arc<RwLock<EngineState>>,
    client: &mut IGRestClient,
    strategies: &[Box<dyn Strategy + Send + Sync>],
    ensemble: &EnsembleVoter,
    risk_manager: &mut RiskManager,
    order_manager: &crate::engine::order_manager::OrderManager,
    event_tx: &broadcast::Sender<EngineEvent>,
    config: &EngineConfig,
    telegram: &TelegramNotifier,
    target_epic: Option<String>,
) -> Result<()> {
    // If target_epic is provided, analyze just that one. Otherwise analyze all.
    let epics = match target_epic {
        Some(e) => vec![e],
        None => config.markets.epics.clone(),
    };

    for epic in &epics {
        let (bid, offer, mid_price, mkt_state) = {
            let s = state.read().await;
            if let Some(ms) = s.markets.live.get(epic) {
                (ms.bid, ms.ask, (ms.bid + ms.ask) / 2.0, ms.market_state.clone())
            } else {
                debug!("No market data yet for {} (waiting for Lightstreamer tick)", epic);
                continue;
            }
        };

        if bid <= 0.0 || offer <= 0.0 {
            continue;
        }

        // Skip analysis when the market is not in a tradeable state (e.g., weekend "edit",
        // auction, or offline). MARKET_STATE is None until IG sends the initial snapshot.
        if let Some(ref state_str) = mkt_state {
            if state_str != "TRADEABLE" {
                debug!("Market {} not tradeable (MARKET_STATE={}), skipping analysis", epic, state_str);
                continue;
            }
        }

        let indicator_set = {
            let s = state.read().await;
            s.markets.indicators.get(epic).cloned()
        };

        if let Some(indicators_map) = indicator_set {
            let mut snapshot_map = std::collections::HashMap::new();

            // Indicators are updated on bar close via the BarAccumulator in the streaming
            // client; here we only read the current snapshot.
            for (tf, indicators) in &indicators_map {
                if let Some(snap) = indicators.snapshot() {
                    snapshot_map.insert(tf.clone(), snap);
                }
            }

            // Emitting events - just using HOUR as default stream visualization for now
            if let Some(snap_hour) = snapshot_map.get("HOUR") {
                let _ = event_tx.send(EngineEvent::indicator_update(epic.clone(), snap_hour.clone()));
            }

            if snapshot_map.is_empty() {
                continue; // no warmed up timeframes
            }

            // Read per-instrument override (ADX range filter)
            let override_cfg = config.strategies.instrument_overrides.get(epic).cloned();
            let adx_range_filter = override_cfg.as_ref().map(|o| o.adx_range_filter).unwrap_or(false);
            let adx_range_max   = override_cfg.as_ref().and_then(|o| o.adx_range_max).unwrap_or(25.0);

            // Read current ADX from HOUR indicators (used by range filter below)
            let current_adx: Option<f64> = snapshot_map
                .get("HOUR")
                .and_then(|s| s.adx);

            // Mean-reversion strategy names — suppressed when market is trending
            const REVERSION_STRATEGIES: &[&str] = &["RSI_Reversal", "Bollinger_Bands"];

            let mut signals = Vec::new();
            for strategy in strategies {
                // ADX range filter: skip mean-reversion strategies in trending markets
                if adx_range_filter && REVERSION_STRATEGIES.contains(&strategy.name()) {
                    if let Some(adx) = current_adx {
                        if adx > adx_range_max {
                            debug!(
                                "ADX range filter: skipping {} for {} (ADX={:.1} > {:.1})",
                                strategy.name(), epic, adx, adx_range_max
                            );
                            continue;
                        }
                    }
                }

                if let Some(signal) = strategy.evaluate(epic, mid_price, &snapshot_map) {
                    let _ = event_tx.send(EngineEvent::signal(
                        signal.epic.clone(),
                        signal.direction.to_string(),
                        signal.strategy.clone(),
                        signal.strength,
                        false,
                    ));
                    signals.push(signal.clone());
                }
            }

            // ── Gold sentiment signal ──────────────────────────────────────────────
            // If this is the Gold epic and `scripts/sentiment_agent.py` has written a
            // fresh JSON file, inject a sentiment-derived signal into the ensemble.
            const GOLD_EPIC: &str = "CS.D.CFIGOLD.CFI.IP";
            if epic.as_str() == GOLD_EPIC {
                let atr = snapshot_map.get("HOUR").and_then(|s| s.atr);
                if let Some(sent) = read_gold_sentiment(
                    "data/gold_sentiment_latest.json",
                    atr,
                    mid_price,
                    config,
                ) {
                    info!(
                        "Gold sentiment signal injected: {} strength={:.1} — {}",
                        sent.direction, sent.strength, sent.reason
                    );
                    let _ = event_tx.send(EngineEvent::signal(
                        sent.epic.clone(),
                        sent.direction.to_string(),
                        sent.strategy.clone(),
                        sent.strength,
                        false,
                    ));
                    signals.push(sent);
                }
            }

            // ── ML Regime signal multipliers (Phase 8.4) ──────────────────────────
            // Read the latest regime from data/regime_latest.json (written hourly by
            // scripts/run_regime_classifier.py). If fresh, scale signal strengths so
            // the dominant strategy family gets a consensus boost and the other is muted.
            // Returns None silently when the file is missing or stale — no-op in that case.
            if let Some(regime) = crate::regime::read_regime(epic.as_str()) {
                crate::regime::apply_regime_multipliers(&mut signals, &regime);
            } else {
                debug!("No fresh regime data for {} — using unweighted signals", epic);
            }

            if let Some(ensemble_signal) = ensemble.vote(&signals) {
                info!(
                    "Ensemble consensus signal: {} {} strength={}",
                    ensemble_signal.direction, epic, ensemble_signal.strength
                );

                let can_trade = {
                    let s = state.read().await;
                    s.can_trade()
                };

                if can_trade {
                    let account_info = {
                        let s = state.read().await;
                        crate::risk::AccountInfo {
                            balance: s.account.balance,
                            equity: s.account.equity,
                            available_margin: s.account.available,
                        }
                    };

                    let open_positions = {
                        let s = state.read().await;
                        s.trades.active
                            .iter()
                            .map(|p| crate::risk::OpenPosition {
                                epic: p.epic.clone(),
                                direction: p.direction.to_string(),
                                size: p.size,
                                entry_price: p.open_price,
                                stop_loss: p.stop_loss.unwrap_or(0.0),
                                take_profit: p.take_profit.unwrap_or(0.0),
                            })
                            .collect::<Vec<_>>()
                    };

                    let direction_str = ensemble_signal.direction.to_string();
                    let verdict = risk_manager.check_trade(
                        &ensemble_signal.epic,
                        &direction_str,
                        ensemble_signal.price,
                        ensemble_signal.stop_loss,
                        ensemble_signal.take_profit,
                        ensemble_signal.trailing_stop_distance,
                        &account_info,
                        &open_positions,
                        &ensemble_signal.strategy,
                    );

                    match verdict {
                        crate::risk::RiskVerdict::Approved(adjusted_trade) => {
                            if config.general.mode != EngineMode::Paper {
                                match order_manager.execute_trade(client, &adjusted_trade).await {
                                    Ok(execution) => {
                                        let position = Position {
                                            deal_id: execution.deal_id.clone(),
                                            deal_reference: execution.deal_reference.clone(),
                                            epic: execution.epic.clone(),
                                            direction: if execution.direction == "BUY" {
                                                Direction::Buy
                                            } else {
                                                Direction::Sell
                                            },
                                            size: execution.size,
                                            open_price: execution.fill_price,
                                            stop_loss: Some(adjusted_trade.stop_loss),
                                            take_profit: Some(adjusted_trade.take_profit),
                                            trailing_stop: adjusted_trade.trailing_stop_distance,
                                            current_price: mid_price,
                                            pnl: 0.0,
                                            strategy: adjusted_trade.strategy.clone(),
                                            opened_at: Utc::now(),
                                            is_virtual: false,
                                        };

                                        {
                                            let mut s = state.write().await;
                                            s.trades.active.push(position.clone());
                                            s.add_signal_record(ensemble_signal.clone(), true, None);
                                        }

                                        let _ = event_tx.send(EngineEvent::trade_executed(
                                            execution.deal_id.clone(),
                                            execution.epic.clone(),
                                            execution.direction.clone(),
                                            execution.size,
                                            execution.fill_price,
                                        ));

                                        let tg = telegram.clone();
                                        let t_epic = execution.epic.clone();
                                        let t_dir = execution.direction.clone();
                                        let t_size = execution.size;
                                        let t_price = execution.fill_price;
                                        let t_sl = adjusted_trade.stop_loss;
                                        let t_tp = Some(adjusted_trade.take_profit);
                                        tokio::spawn(async move {
                                            let _ = tg.send_trade_alert(
                                                &t_epic,
                                                &t_dir,
                                                t_size,
                                                t_price,
                                                t_sl,
                                                t_tp,
                                            ).await;
                                        });
                                    }
                                    Err(e) => {
                                        error!("Failed to execute trade: {}", e);
                                        let mut s = state.write().await;
                                        s.add_signal_record(
                                            ensemble_signal.clone(),
                                            false,
                                            Some(format!("Execution failed: {}", e)),
                                        );
                                    }
                                }
                            } else {
                                info!("Shadow Mode: Signal approved, creating virtual position for tracking");
                                let position = Position {
                                    deal_id: format!("shadow_{}", ensemble_signal.id),
                                    deal_reference: format!("shadow_{}", ensemble_signal.id),
                                    epic: ensemble_signal.epic.clone(),
                                    direction: ensemble_signal.direction.clone(),
                                    size: adjusted_trade.size,
                                    open_price: mid_price,
                                    stop_loss: Some(adjusted_trade.stop_loss),
                                    take_profit: Some(adjusted_trade.take_profit),
                                    trailing_stop: adjusted_trade.trailing_stop_distance,
                                    current_price: mid_price,
                                    pnl: 0.0,
                                    strategy: adjusted_trade.strategy.clone(),
                                    opened_at: Utc::now(),
                                    is_virtual: true,
                                };

                                {
                                    let mut s = state.write().await;
                                    s.trades.active.push(position.clone());
                                    s.add_signal_record(
                                        ensemble_signal.clone(),
                                        true,
                                        Some("Shadow Mode execution".to_string()),
                                    );
                                }

                                let _ = event_tx.send(EngineEvent::trade_executed(
                                    position.deal_id.clone(),
                                    position.epic.clone(),
                                    position.direction.to_string(),
                                    position.size,
                                    position.open_price,
                                ));

                                let tg = telegram.clone();
                                let t_epic = position.epic.clone();
                                let t_dir = position.direction.to_string();
                                let t_size = position.size;
                                let t_price = position.open_price;
                                let t_sl = position.stop_loss;
                                let t_tp = position.take_profit;
                                tokio::spawn(async move {
                                    let mut msg = format!(
                                        "<b>VIRTUAL TRADE OPENED</b>\n\n<b>Instrument:</b> {}\n<b>Direction:</b> {}\n<b>Size:</b> {}\n<b>Entry Price:</b> {}\n<b>Stop Loss:</b> {}\n<b>Time:</b> {}",
                                        get_instrument_name(&t_epic), t_dir, t_size, t_price, t_sl.unwrap_or(0.0),
                                        (chrono::Utc::now() + chrono::Duration::hours(8)).format("%H:%M:%S SGT")
                                    );
                                    if let Some(tp) = t_tp {
                                        msg.push_str(&format!("\n<b>Take Profit:</b> {}", tp));
                                    }
                                    let _ = tg.send_message(&msg).await;
                                });
                            }
                        }
                        crate::risk::RiskVerdict::Rejected(reason) => {
                            warn!("Trade rejected by risk manager: {}", reason);
                            let mut s = state.write().await;
                            s.add_signal_record(
                                ensemble_signal.clone(),
                                false,
                                Some(format!("Risk rejected: {}", reason)),
                            );
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

/// Manually trigger a trade for a specific epic and direction
#[allow(clippy::too_many_arguments)]
// TODO: bundle args into an AnalysisContext struct to reduce parameter count
pub async fn execute_manual_trigger(
    state: &Arc<RwLock<EngineState>>,
    client: &mut IGRestClient,
    risk_manager: &mut RiskManager,
    order_manager: &crate::engine::order_manager::OrderManager,
    event_tx: &broadcast::Sender<EngineEvent>,
    config: &EngineConfig,
    telegram: &TelegramNotifier,
    epic: String,
    direction: String,
) -> Result<()> {
    info!("Executing manual trigger for {} {}", epic, direction);

    let (_bid, _ask, price) = {
        let s = state.read().await;
        if let Some(ms) = s.markets.live.get(&epic) {
            (ms.bid, ms.ask, (ms.bid + ms.ask) / 2.0)
        } else {
            return Err(anyhow::anyhow!("No market data available for {} to execute manual trigger", epic));
        }
    };

    let dir = match direction.to_lowercase().as_str() {
        "buy" | "long" => crate::engine::state::Direction::Buy,
        "sell" | "short" => crate::engine::state::Direction::Sell,
        _ => return Err(anyhow::anyhow!("Invalid direction: {}", direction)),
    };

    // Calculate default SL/TP based on ATR if available, else use a fixed distance
    let indicators = {
        let s = state.read().await;
        s.markets.indicators.get(&epic).and_then(|m| m.get("HOUR")).and_then(|i| i.snapshot())
    };

    let (stop_loss, take_profit) = if let Some(snap) = indicators {
        if let Some(atr) = snap.atr {
            let sl_dist = atr * config.strategies.default_atr_sl_multiplier;
            let tp_dist = atr * config.strategies.default_atr_tp_multiplier;
            match dir {
                crate::engine::state::Direction::Buy => (price - sl_dist, price + tp_dist),
                crate::engine::state::Direction::Sell => (price + sl_dist, price - tp_dist),
            }
        } else {
            // Fallback: 50 pips (rough estimation)
            let dist = price * 0.005; 
            match dir {
                crate::engine::state::Direction::Buy => (price - dist, price + dist * 2.0),
                crate::engine::state::Direction::Sell => (price + dist, price - dist * 2.0),
            }
        }
    } else {
        // Fallback: 50 pips (rough estimation)
        let dist = price * 0.005; 
        match dir {
            crate::engine::state::Direction::Buy => (price - dist, price + dist * 2.0),
            crate::engine::state::Direction::Sell => (price + dist, price - dist * 2.0),
        }
    };

    let account_info = {
        let s = state.read().await;
        crate::risk::AccountInfo {
            balance: s.account.balance,
            equity: s.account.equity,
            available_margin: s.account.available,
        }
    };

    let open_positions = {
        let s = state.read().await;
        s.trades.active.iter()
            .map(|p| crate::risk::OpenPosition {
                epic: p.epic.clone(),
                direction: p.direction.to_string(),
                size: p.size,
                entry_price: p.open_price,
                stop_loss: p.stop_loss.unwrap_or(0.0),
                take_profit: p.take_profit.unwrap_or(0.0),
            })
            .collect::<Vec<_>>()
    };

    let verdict = risk_manager.check_trade(
        &epic,
        &dir.to_string(),
        price,
        stop_loss,
        take_profit,
        None,
        &account_info,
        &open_positions,
        "ManualTrigger",
    );

    match verdict {
        crate::risk::RiskVerdict::Approved(adjusted_trade) => {
            info!("Manual trigger APPROVED: {} {} @ {}", epic, dir, price);
            if config.general.mode != EngineMode::Paper {
                match order_manager.execute_trade(client, &adjusted_trade).await {
                    Ok(execution) => {
                        let mut s = state.write().await;
                        let pos = Position {
                            deal_id: execution.deal_id.clone(),
                            deal_reference: execution.deal_reference.clone(),
                            epic: epic.clone(),
                            direction: dir.clone(),
                            size: adjusted_trade.size,
                            open_price: execution.fill_price,
                            stop_loss: Some(adjusted_trade.stop_loss),
                            take_profit: Some(adjusted_trade.take_profit),
                            trailing_stop: adjusted_trade.trailing_stop_distance,
                            current_price: execution.fill_price,
                            opened_at: Utc::now(),
                            pnl: 0.0,
                            strategy: "ManualTrigger".into(),
                            is_virtual: false,
                        };
                        s.trades.active.push(pos);
                        
                        let _ = event_tx.send(EngineEvent::trade_executed(
                            execution.deal_id,
                            epic.clone(),
                            dir.to_string(),
                            adjusted_trade.size,
                            execution.fill_price,
                        ));
                        
                        let _ = telegram.send_trade_alert(
                            &epic, 
                            &dir.to_string(), 
                            adjusted_trade.size, 
                            execution.fill_price,
                            adjusted_trade.stop_loss,
                            Some(adjusted_trade.take_profit)
                        ).await;
                    }
                    Err(e) => error!("Failed to execute manual trade: {}", e),
                }
            } else {
                // Paper mode: Create virtual position
                let mut s = state.write().await;
                let pos = Position {
                    deal_id: format!("v-{}", Utc::now().timestamp_millis()),
                    deal_reference: "manual-paper".into(),
                    epic: epic.clone(),
                    direction: dir.clone(),
                    size: adjusted_trade.size,
                    open_price: price,
                    stop_loss: Some(adjusted_trade.stop_loss),
                    take_profit: Some(adjusted_trade.take_profit),
                    trailing_stop: adjusted_trade.trailing_stop_distance,
                    current_price: price,
                    opened_at: Utc::now(),
                    pnl: 0.0,
                    strategy: "ManualTrigger".into(),
                    is_virtual: true,
                };
                s.trades.active.push(pos);
                info!("Paper Trade (Manual): Created virtual position for {}", epic);
            }
        }
        crate::risk::RiskVerdict::Rejected(reason) => {
            warn!("Manual trigger REJECTED by risk manager: {}", reason);
            let alert_msg = format!(
                "Manual trigger for {} rejected: {}\nTime: {}", 
                get_instrument_name(&epic), 
                reason,
                (chrono::Utc::now() + chrono::Duration::hours(8)).format("%H:%M:%S SGT")
            );
            
            let _ = event_tx.send(EngineEvent::risk_alert(
                alert_msg.clone(),
                "high".into(),
            ));

            let _ = telegram.send_instrument_risk_alert(&epic, &reason).await;
        }
    }

    Ok(())
}

// ── Gold sentiment reader ──────────────────────────────────────────────────────

/// Read the Gold news sentiment JSON written by `scripts/sentiment_agent.py`.
///
/// Returns a `Signal` when all conditions are met:
///   - File exists and is valid JSON
///   - `timestamp` is within the last 30 minutes (not stale)
///   - `|score|` ≥ 0.55 (strong enough to influence the ensemble)
///
/// Returns `None` on any I/O/parse error, stale data, or neutral/weak signal.
fn read_gold_sentiment(
    file_path: &str,
    atr: Option<f64>,
    mid_price: f64,
    config: &EngineConfig,
) -> Option<Signal> {
    // ── Read & parse ──────────────────────────────────────────────────────────
    let raw  = std::fs::read_to_string(file_path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&raw).ok()?;

    let ts         = json["timestamp"].as_i64()?;
    let score      = json["score"].as_f64()?;
    let confidence = json["confidence"].as_f64().unwrap_or(0.5);
    let mode       = json["mode"].as_str().unwrap_or("unknown").to_string();
    let drivers: Vec<String> = json["key_drivers"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .take(4)
                .collect()
        })
        .unwrap_or_default();

    // ── Stale check: reject if older than 30 minutes ──────────────────────────
    let age_secs = Utc::now().timestamp() - ts;
    if age_secs > 1800 {
        debug!("Gold sentiment file is stale ({} s old) — skipping", age_secs);
        return None;
    }

    // ── Score threshold gate ───────────────────────────────────────────────────
    const THRESHOLD: f64 = 0.55;
    let direction = if score >= THRESHOLD {
        Direction::Buy
    } else if score <= -THRESHOLD {
        Direction::Sell
    } else {
        debug!("Gold sentiment score {:.3} below threshold ±{} — skipping", score, THRESHOLD);
        return None;
    };

    // ── Signal strength: 6.0 (min consensus) + confidence bonus up to +3.5 ───
    let strength = (6.0_f64 + confidence * 3.5).min(9.5);

    // ── SL / TP from ATR, falling back to 0.5 % distance ─────────────────────
    let sl_mult = config.strategies.default_atr_sl_multiplier;
    let tp_mult = config.strategies.default_atr_tp_multiplier;

    let (stop_loss, take_profit) = match (atr, &direction) {
        (Some(a), Direction::Buy)  => (mid_price - a * sl_mult, mid_price + a * tp_mult),
        (Some(a), Direction::Sell) => (mid_price + a * sl_mult, mid_price - a * tp_mult),
        (None, Direction::Buy)  => {
            let d = mid_price * 0.005;
            (mid_price - d, mid_price + d * 2.0)
        }
        (None, Direction::Sell) => {
            let d = mid_price * 0.005;
            (mid_price + d, mid_price - d * 2.0)
        }
    };

    let reason = format!(
        "score={:.3} conf={:.2} mode={} age={}s drivers=[{}]",
        score,
        confidence,
        mode,
        age_secs,
        drivers.join(", "),
    );

    Some(Signal {
        id:         uuid::Uuid::new_v4().to_string(),
        epic:       "CS.D.CFIGOLD.CFI.IP".to_string(),
        direction,
        strength,
        strategy:   "Gold_Sentiment".to_string(),
        reason,
        price:      mid_price,
        stop_loss,
        take_profit,
        trailing_stop_distance: None,
        timestamp:  Utc::now(),
    })
}
