use anyhow::Result;
use chrono::Utc;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tracing::{debug, error, info, warn};

use crate::api::rest_client::IGRestClient;
use crate::engine::config::{EngineConfig, EngineMode};
use crate::engine::state::{get_instrument_name, Direction, EngineState, Position, Signal};
use crate::ipc::events::EngineEvent;
use crate::notifications::telegram::TelegramNotifier;
use crate::risk::RiskManager;
use crate::strategy::ensemble::EnsembleVoter;
use crate::strategy::traits::{M15Strategy, Strategy};

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
                (
                    ms.bid,
                    ms.ask,
                    (ms.bid + ms.ask) / 2.0,
                    ms.market_state.clone(),
                )
            } else {
                debug!(
                    "No market data yet for {} (waiting for Lightstreamer tick)",
                    epic
                );
                continue;
            }
        };

        if bid <= 0.0 || offer <= 0.0 {
            debug!(
                "[{}] Skipping analysis — bid={:.5} offer={:.5} (waiting for valid prices)",
                epic, bid, offer
            );
            continue;
        }

        // Skip analysis when the market is not in a tradeable state (e.g., weekend "edit",
        // auction, or offline). MARKET_STATE is None until IG sends the initial snapshot.
        // Case-insensitive: IG sends lowercase "tradeable" or "tradeable_no_stops".
        if let Some(ref state_str) = mkt_state {
            let upper = state_str.to_ascii_uppercase();
            if !upper.starts_with("TRADEABLE") {
                info!(
                    "Market {} not tradeable (MARKET_STATE={}), skipping analysis",
                    epic, state_str
                );
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
                let _ = event_tx.send(EngineEvent::indicator_update(
                    epic.clone(),
                    snap_hour.clone(),
                ));
            }

            if snapshot_map.is_empty() {
                tracing::info!(
                    "[{}] Indicators not warmed up yet — skipping bar analysis",
                    epic
                );
                continue;
            }

            // Read per-instrument override (ADX range filter)
            let override_cfg = config.strategies.instrument_overrides.get(epic).cloned();
            let adx_range_filter = override_cfg
                .as_ref()
                .map(|o| o.adx_range_filter)
                .unwrap_or(false);
            let adx_range_max = override_cfg
                .as_ref()
                .and_then(|o| o.adx_range_max)
                .unwrap_or(25.0);

            // Read current ADX from HOUR indicators (used by range filter below)
            let current_adx: Option<f64> = snapshot_map.get("HOUR").and_then(|s| s.adx);

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
                                strategy.name(),
                                epic,
                                adx,
                                adx_range_max
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
                if let Some(sent) =
                    read_gold_sentiment("data/gold_sentiment_latest.json", atr, mid_price, config)
                {
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

            // ── Phase B: Signal Boosters (structural context) ─────────────────────
            // Applied BEFORE regime multipliers so they amplify the raw signal quality,
            // and regime scaling then modulates the boosted strengths.
            if let Some(snap) = snapshot_map.get("HOUR") {
                apply_signal_boosters(&mut signals, mid_price, snap);
            }

            // ── ML Regime signal multipliers (Phase 8.4 / 12.1) ──────────────────
            // Read the latest regime from data/regime_latest.json (written hourly by
            // scripts/run_regime_classifier.py). If fresh, scale signal strengths so
            // the dominant strategy family gets a consensus boost and the other is muted.
            // Returns None silently when the file is missing or stale — no-op in that case.
            // Also capture the regime string for birth tracking (13.1) and VOLATILE gate (12.4).
            let current_regime_str: Option<String> =
                crate::regime::read_regime(epic.as_str()).map(|regime| {
                    crate::regime::apply_regime_multipliers(&mut signals, &regime);
                    regime.kind.to_string()
                });

            // ── Regime persistence tracking ─────────────────────────────────────
            // Track how many days the current regime has been unchanged for this
            // epic. Used by the regime cooldown subsystem to relax VOLATILE
            // restrictions when they persist beyond the configured threshold.
            let regime_persistence_days: u64 = current_regime_str
                .as_deref()
                .map(|r| crate::regime::update_persistence_and_get_days(epic.as_str(), r))
                .unwrap_or(0);

            // Check if regime cooldown applies (VOLATILE for longer than threshold)
            let regime_cooldown_active = current_regime_str.as_deref() == Some("VOLATILE")
                && config
                    .strategies
                    .regime_cooldown_days
                    .is_some_and(|threshold| regime_persistence_days >= threshold);

            if regime_cooldown_active {
                info!(
                    "[{}] Regime cooldown active: VOLATILE for {} days (threshold: {} days) — relaxing restrictions",
                    epic,
                    regime_persistence_days,
                    config.strategies.regime_cooldown_days.unwrap_or(0)
                );
            }

            if current_regime_str.is_none() {
                debug!(
                    "No fresh regime data for {} — using unweighted signals",
                    epic
                );
            }
            let is_volatile_regime = current_regime_str.as_deref() == Some("VOLATILE");

            tracing::info!(
                "[{}] Bar analysis: {}/{} strategies fired signals",
                epic,
                signals.len(),
                strategies.len()
            );

            // ── H1 Direction Bias (Phase 14.E) ────────────────────────────────
            // Record the net directional lean of H1 strategies for this epic.
            // Written after regime multipliers so muted strategies naturally
            // contribute less — e.g. TRENDING mutes RSI/Bollinger to 0.3×,
            // reducing their vote weight relative to trend strategies.
            // M15 reads this to gate contra-direction entries.
            {
                use crate::engine::state::H1DirectionBias;
                let mut buy_count = 0usize;
                let mut sell_count = 0usize;
                for sig in &signals {
                    match sig.direction {
                        Direction::Buy => buy_count += 1,
                        Direction::Sell => sell_count += 1,
                    }
                }
                let bias_direction = match buy_count.cmp(&sell_count) {
                    std::cmp::Ordering::Greater => Some(Direction::Buy),
                    std::cmp::Ordering::Less => Some(Direction::Sell),
                    std::cmp::Ordering::Equal => None,
                };
                debug!(
                    "[{}] H1 bias: {:?} ({} buy, {} sell)",
                    epic, bias_direction, buy_count, sell_count
                );
                let mut s = state.write().await;
                s.markets.h1_bias.insert(
                    epic.clone(),
                    H1DirectionBias {
                        direction: bias_direction,
                        buy_count,
                        sell_count,
                        updated_at: Utc::now(),
                    },
                );
            }

            // ── VOLATILE scalp tier ───────────────────────────────────────────
            // Standard vote: uses min_consensus + min_avg_strength from config (default 3 / 7.5).
            // Scalp fallback (VOLATILE only): reads from strategies.consensus_matrix["volatile"].
            // Phase 15.D recalibration: with differentiated VOLATILE multipliers
            // (Stochastic 1.2×, RSI/Bollinger 1.0×, MACD 0.8×), signals are stronger than
            // the old flat 0.5× mute. Scalp fallback threshold updated to 7.5 in config
            // (was hardcoded 5.0 when all strategies were at 0.5×).
            let (v_barrier, v_min_strength) = config
                .strategies
                .consensus_matrix
                .get("volatile")
                .map(|e| (e.barrier, e.min_strength))
                .unwrap_or((2, 7.5)); // safe fallback if config key missing

            let (maybe_signal, volatile_scalp) = if is_volatile_regime {
                match ensemble.vote(&signals) {
                    Some(sig) => (Some(sig), false),
                    None => (
                        ensemble.vote_with_overrides(&signals, v_barrier, v_min_strength),
                        true,
                    ),
                }
            } else {
                (ensemble.vote(&signals), false)
            };

            if let Some(mut ensemble_signal) = maybe_signal {
                if volatile_scalp {
                    info!(
                        "VOLATILE scalp signal: {} {} strength={:.2} (half-size trade)",
                        ensemble_signal.direction, epic, ensemble_signal.strength
                    );
                } else {
                    info!(
                        "Ensemble consensus signal: {} {} strength={}",
                        ensemble_signal.direction, epic, ensemble_signal.strength
                    );
                }

                // ── 15.D VOLATILE SL/TP Override ─────────────────────────────────
                // Default strategy SL/TP use 1.5×ATR / 4.0×ATR. In VOLATILE regime
                // the ATR is inflated, making TP unreachably far (4×ATR ≈ 632 pips
                // for EURUSD). A volatile wave only moves 1.5–2× ATR before reversing.
                // Override to 0.75×SL / 2.0×TP: R:R = 2.67, TP reachable in a wave.
                // The early BE snap (Phase 15.C) then locks profit at 30% of the
                // tighter SL distance — catching profit on the wave, not waiting for 4×ATR.
                //
                // Regime cooldown: if VOLATILE has persisted beyond regime_cooldown_days,
                // use relaxed intermediate multipliers instead of the tight VOLATILE ones.
                // This prevents the restrictive VOLATILE parameters from becoming the
                // permanent (losing) default when the classifier stays VOLATILE for weeks.
                if is_volatile_regime {
                    if let Some(h1_snap) = snapshot_map.get("HOUR") {
                        if let Some(atr) = h1_snap.atr {
                            let (sl_mult, tp_mult) = if regime_cooldown_active {
                                let cd_sl = config
                                    .strategies
                                    .regime_cooldown_sl_multiplier
                                    .unwrap_or(1.25);
                                let cd_tp = config
                                    .strategies
                                    .regime_cooldown_tp_multiplier
                                    .unwrap_or(3.0);
                                info!(
                                    "[{}] Regime cooldown SL/TP: {:.2}x / {:.2}x (relaxed from {:.2}x / {:.2}x)",
                                    epic, cd_sl, cd_tp,
                                    config.strategies.volatile_atr_sl_multiplier,
                                    config.strategies.volatile_atr_tp_multiplier
                                );
                                (cd_sl, cd_tp)
                            } else {
                                (
                                    config.strategies.volatile_atr_sl_multiplier,
                                    config.strategies.volatile_atr_tp_multiplier,
                                )
                            };
                            let sl_dist = atr * sl_mult;
                            let tp_dist = atr * tp_mult;
                            use crate::engine::state::Direction;
                            let (new_sl, new_tp) = match ensemble_signal.direction {
                                Direction::Buy => (
                                    ensemble_signal.price - sl_dist,
                                    ensemble_signal.price + tp_dist,
                                ),
                                Direction::Sell => (
                                    ensemble_signal.price + sl_dist,
                                    ensemble_signal.price - tp_dist,
                                ),
                            };
                            info!(
                                "[{}] VOLATILE SL/TP override: SL={:.5} ({:.2}×ATR) TP={:.5} ({:.2}×ATR)",
                                epic, new_sl, sl_mult, new_tp, tp_mult
                            );
                            ensemble_signal.stop_loss = new_sl;
                            ensemble_signal.take_profit = new_tp;
                        }
                    }
                }

                // ── 15.B H1 Macro Trend Gate ──────────────────────────────────────
                // Block H1 entries that trade against the recent H1 price trend.
                // Lesson from losses: strategies fire SELL on individual bearish candles
                // even when the overall H1 close-to-close trend is clearly rising.
                // Fix: compute linear regression slope over last N H1 closes.
                //   slope > 0 → uptrend  → block SELL
                //   slope < 0 → downtrend → block BUY
                // Requires at least 3 bars to compute a meaningful slope.
                // In VOLATILE regime, skip the trend gate entirely — we want to catch
                // waves in both directions. Phase 15.C (early breakeven snap) handles
                // profit protection instead. Gate is only useful for TRENDING/RANGING
                // where fighting the trend is reliably bad.
                if config.strategies.h1_macro_trend_gate_enabled && !is_volatile_regime {
                    let lookback = config.strategies.h1_macro_trend_lookback.max(3);
                    let slope: Option<f64> = {
                        let s = state.read().await;
                        s.markets
                            .history
                            .get_candles(epic.as_str(), "HOUR")
                            .and_then(|bars| {
                                let n = bars.len();
                                if n < 3 {
                                    return None;
                                }
                                let take = lookback.min(n);
                                let closes: Vec<f64> =
                                    bars[n - take..].iter().map(|b| b.close).collect();
                                // Simple linear regression slope
                                let len = closes.len() as f64;
                                let x_mean = (len - 1.0) / 2.0;
                                let y_mean = closes.iter().sum::<f64>() / len;
                                let num: f64 = closes
                                    .iter()
                                    .enumerate()
                                    .map(|(i, &y)| (i as f64 - x_mean) * (y - y_mean))
                                    .sum();
                                let den: f64 = closes
                                    .iter()
                                    .enumerate()
                                    .map(|(i, _)| (i as f64 - x_mean).powi(2))
                                    .sum();
                                if den == 0.0 {
                                    None
                                } else {
                                    Some(num / den)
                                }
                            })
                    };
                    if let Some(slope_val) = slope {
                        use crate::engine::state::Direction;
                        let blocked = match ensemble_signal.direction {
                            Direction::Sell if slope_val > 0.0 => {
                                warn!(
                                    "[{}] H1 macro trend gate: slope={:.6} (uptrend) — blocking SELL signal",
                                    epic, slope_val
                                );
                                true
                            }
                            Direction::Buy if slope_val < 0.0 => {
                                warn!(
                                    "[{}] H1 macro trend gate: slope={:.6} (downtrend) — blocking BUY signal",
                                    epic, slope_val
                                );
                                true
                            }
                            _ => {
                                info!(
                                    "[{}] H1 macro trend gate: slope={:.6} — signal {:?} aligned, passing",
                                    epic, slope_val, ensemble_signal.direction
                                );
                                false
                            }
                        };
                        if blocked {
                            let mut s = state.write().await;
                            s.add_signal_record(
                                ensemble_signal.clone(),
                                false,
                                Some(format!(
                                    "H1 macro trend gate: slope={:.6} blocks {:?}",
                                    slope_val, ensemble_signal.direction
                                )),
                            );
                            continue;
                        }
                    }
                }

                // ── 12.2 Macro Pause — Sentiment Velocity Guard ──────────────────
                let macro_paused = {
                    let s = state.read().await;
                    s.metrics
                        .macro_pause_until
                        .map(|until| chrono::Utc::now() < until)
                        .unwrap_or(false)
                };
                if macro_paused {
                    warn!(
                        "[{}] Macro pause active (sentiment velocity spike) — skipping trade entry",
                        epic
                    );
                    let mut s = state.write().await;
                    s.add_signal_record(
                        ensemble_signal.clone(),
                        false,
                        Some("Macro pause: sentiment velocity spike".to_string()),
                    );
                    continue;
                }

                // ── 12.3 Dynamic Spread Gate ─────────────────────────────────────
                let (current_spread, avg_spread) = {
                    let s = state.read().await;
                    s.markets
                        .live
                        .get(epic.as_str())
                        .map(|ms| (ms.spread, ms.avg_spread))
                        .unwrap_or((0.0, 0.0))
                };
                let spread_threshold = avg_spread * 1.5;
                if avg_spread > 0.0 && current_spread > spread_threshold {
                    warn!(
                        "[{}] Spread gate: spread={:.5} > 1.5×avg={:.5} — trade rejected",
                        epic, current_spread, avg_spread
                    );
                    let mut s = state.write().await;
                    s.add_signal_record(
                        ensemble_signal.clone(),
                        false,
                        Some(format!(
                            "Spread gate: {:.5} > {:.5}",
                            current_spread, spread_threshold
                        )),
                    );
                    continue;
                }

                // ── Post-trade cooldown gate ─────────────────────────────────────
                let in_cooldown = {
                    let s = state.read().await;
                    s.is_in_cooldown(epic.as_str())
                };
                if in_cooldown {
                    warn!("[{}] Re-entry blocked — post-trade cooldown active", epic);
                    let mut s = state.write().await;
                    s.add_signal_record(
                        ensemble_signal.clone(),
                        false,
                        Some("Post-trade cooldown active".to_string()),
                    );
                    continue;
                }

                let can_trade = {
                    let s = state.read().await;
                    s.can_trade()
                };

                if can_trade {
                    let (account_info, account_currency) = {
                        let s = state.read().await;
                        (
                            crate::risk::AccountInfo {
                                balance: s.account.balance,
                                equity: s.account.equity,
                                available_margin: s.account.available,
                            },
                            s.account.currency.clone(),
                        )
                    };

                    let open_positions = {
                        let s = state.read().await;
                        s.trades
                            .active
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
                        crate::risk::RiskVerdict::Approved(mut adjusted_trade) => {
                            // ── 12.4 VOLATILE regime — MARKET order (not LIMIT) ───
                            // LIMIT orders at mid_price in volatile conditions create dangling
                            // working orders on IG when price moves before fill.  Signal muting
                            // (VOLATILE_MUTE=0.5) + spread gate (12.3) already guard quality;
                            // signals reaching this point carry strong conviction and must be
                            // taken at market while the opportunity exists.
                            //
                            // VOLATILE scalp: halve position size — smaller risk, still captures
                            // the move.  Skip (don't clamp up) if the halved size falls below the
                            // instrument's IG minimum deal size (e.g. Gold min = 3.0 → half = 1.5,
                            // which IG would reject).  Clamping up to minimum would negate the
                            // half-size intent; skipping is the correct conservative action.
                            if volatile_scalp {
                                let min_size = risk_manager
                                    .get_instrument_spec(epic.as_str())
                                    .min_deal_size;
                                let half_size = adjusted_trade.size * 0.5;
                                if half_size < min_size {
                                    warn!(
                                        "[{}] VOLATILE scalp: half-size {:.2} < instrument minimum {:.2} — skipping (would be rejected by IG)",
                                        epic, half_size, min_size
                                    );
                                    let mut s = state.write().await;
                                    s.add_signal_record(
                                        ensemble_signal.clone(),
                                        false,
                                        Some(format!(
                                            "VOLATILE scalp skipped: half-size {:.2} < min {:.2}",
                                            half_size, min_size
                                        )),
                                    );
                                    continue;
                                }
                                adjusted_trade.size = half_size;
                                info!(
                                    "[{}] VOLATILE scalp: size halved → {:.2}",
                                    epic, adjusted_trade.size
                                );
                            } else if is_volatile_regime {
                                info!("[{}] VOLATILE full consensus → MARKET", epic);
                            }

                            if config.general.mode != EngineMode::Paper {
                                match order_manager
                                    .execute_trade(client, &adjusted_trade, &account_currency)
                                    .await
                                {
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
                                            currency: account_currency.clone(),
                                            strategy: adjusted_trade.strategy.clone(),
                                            opened_at: Utc::now(),
                                            is_virtual: false,
                                            opened_in_regime: current_regime_str.clone(), // 13.1
                                        };

                                        {
                                            let mut s = state.write().await;
                                            s.trades.active.push(position.clone());
                                            s.add_signal_record(
                                                ensemble_signal.clone(),
                                                true,
                                                None,
                                            );
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
                                            let _ = tg
                                                .send_trade_alert(
                                                    &t_epic, &t_dir, t_size, t_price, t_sl, t_tp,
                                                )
                                                .await;
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
                                        // Short cooldown on failure — prevents immediate retry
                                        // on the next bar while IG recovers (e.g. 500 errors).
                                        s.set_trade_cooldown(&ensemble_signal.epic, 300);
                                        // 5 min
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
                                    currency: account_currency.clone(),
                                    strategy: adjusted_trade.strategy.clone(),
                                    opened_at: Utc::now(),
                                    is_virtual: true,
                                    opened_in_regime: current_regime_str.clone(), // 13.1
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
            return Err(anyhow::anyhow!(
                "No market data available for {} to execute manual trigger",
                epic
            ));
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
        s.markets
            .indicators
            .get(&epic)
            .and_then(|m| m.get("HOUR"))
            .and_then(|i| i.snapshot())
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

    let (account_info, account_currency) = {
        let s = state.read().await;
        (
            crate::risk::AccountInfo {
                balance: s.account.balance,
                equity: s.account.equity,
                available_margin: s.account.available,
            },
            s.account.currency.clone(),
        )
    };

    let open_positions = {
        let s = state.read().await;
        s.trades
            .active
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
                match order_manager
                    .execute_trade(client, &adjusted_trade, &account_currency)
                    .await
                {
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
                            currency: account_currency.clone(),
                            strategy: "ManualTrigger".into(),
                            is_virtual: false,
                            opened_in_regime: None, // manual trigger — regime not tracked
                        };
                        s.trades.active.push(pos);

                        let _ = event_tx.send(EngineEvent::trade_executed(
                            execution.deal_id,
                            epic.clone(),
                            dir.to_string(),
                            adjusted_trade.size,
                            execution.fill_price,
                        ));

                        let _ = telegram
                            .send_trade_alert(
                                &epic,
                                &dir.to_string(),
                                adjusted_trade.size,
                                execution.fill_price,
                                adjusted_trade.stop_loss,
                                Some(adjusted_trade.take_profit),
                            )
                            .await;
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
                    currency: account_currency.clone(),
                    strategy: "ManualTrigger".into(),
                    is_virtual: true,
                    opened_in_regime: None,
                };
                s.trades.active.push(pos);
                info!(
                    "Paper Trade (Manual): Created virtual position for {}",
                    epic
                );
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

            let _ = event_tx.send(EngineEvent::risk_alert(alert_msg.clone(), "high".into()));

            let _ = telegram.send_instrument_risk_alert(&epic, &reason).await;
        }
    }

    Ok(())
}

// ── Phase B: Signal Boosters ──────────────────────────────────────────────────

/// Apply additive/multiplicative signal strength bonuses for favourable structural conditions.
///
/// Called BEFORE regime multipliers, so the bonuses feed into the regime-scaled composite.
/// All strengths are clamped to 10.0 after each bonus.
///
/// # Boosters
///
/// - **ATR Expansion**: Current bar range > 1.5×ATR14 → +1.0 to all signals.
///   Rationale: wide-range bars confirm the move is driven by real momentum, not noise.
///
/// - **Key Level Proximity**: Price within 0.1% of a round major level
///   (Gold: $50 increments; JPY pairs: 0.50 increments; EUR/USD: 0.0050 increments) →
///   ×1.2 to signals aligned with the breakout direction.
///   Rationale: institutional decision zones produce sharp, sustained moves.
fn apply_signal_boosters(
    signals: &mut [Signal],
    mid_price: f64,
    snap: &crate::indicators::IndicatorSnapshot,
) {
    // ── ATR Expansion Boost ──────────────────────────────────────────────────
    if let (Some(atr), Some(bar_range)) = (snap.atr, snap.last_bar_range) {
        if atr > 0.0 && bar_range > atr * 1.5 {
            for sig in signals.iter_mut() {
                let orig = sig.strength;
                sig.strength = (sig.strength + 1.0).min(10.0);
                debug!(
                    "ATR expansion boost: {} {:.1} → {:.1} (range={:.2} > 1.5×atr={:.2})",
                    sig.strategy, orig, sig.strength, bar_range, atr
                );
            }
        }
    }

    // ── Key Level Proximity Boost ────────────────────────────────────────────
    // Determine level grid based on instrument price magnitude.
    let level_size = if mid_price > 100.0 {
        50.0 // Gold ~$2,900 → every $50
    } else if mid_price > 10.0 {
        0.5 // JPY pairs ~150 → every 0.50
    } else {
        0.005 // EUR/USD ~1.10 → every 0.0050
    };

    let proximity = mid_price * 0.001; // 0.1% of price
    let nearest_level = (mid_price / level_size).round() * level_size;
    let dist = (mid_price - nearest_level).abs();

    if dist < proximity {
        // Above or at the level → breakout is to the upside
        let breakout_dir = if mid_price >= nearest_level {
            Direction::Buy
        } else {
            Direction::Sell
        };

        for sig in signals.iter_mut() {
            if sig.direction == breakout_dir {
                let orig = sig.strength;
                sig.strength = (sig.strength * 1.2).min(10.0);
                debug!(
                    "Key level boost: {} {:.1} → {:.1} (level={:.2} dist={:.4})",
                    sig.strategy, orig, sig.strength, nearest_level, dist
                );
            }
        }
    }
}

// ── M15 analysis ──────────────────────────────────────────────────────────────

/// Analyze markets using M15 (15-minute) strategies with H1 as directional filter.
/// Called every 60 seconds; only processes epics where a new M15 bar has closed.
#[allow(clippy::too_many_arguments)]
pub async fn analyze_market_m15(
    state: &Arc<RwLock<EngineState>>,
    client: &mut IGRestClient,
    m15_strategies: &[Box<dyn M15Strategy + Send + Sync>],
    m15_ensemble: &EnsembleVoter,
    risk_manager: &mut RiskManager,
    order_manager: &crate::engine::order_manager::OrderManager,
    event_tx: &broadcast::Sender<EngineEvent>,
    config: &EngineConfig,
    telegram: &TelegramNotifier,
) -> Result<()> {
    if m15_strategies.is_empty() {
        return Ok(());
    }

    let epics: Vec<String> = config.markets.epics.clone();

    for epic in &epics {
        // Get M15 and H1 indicator snapshots
        let (m15_snap, h1_snap) = {
            let s = state.read().await;
            let indicators = match s.markets.indicators.get(epic.as_str()) {
                Some(m) => m,
                None => continue,
            };
            let m15 = indicators.get("MINUTE_15").and_then(|i| i.snapshot());
            let h1 = indicators.get("HOUR").and_then(|i| i.snapshot());
            match (m15, h1) {
                (Some(m), Some(h)) => (m, h),
                _ => {
                    debug!(
                        "[M15] {} — M15 indicators not warmed up yet, skipping",
                        epic
                    );
                    continue;
                }
            }
        };

        // Check market state and get prices
        let (bid, offer, mid_price, mkt_state) = {
            let s = state.read().await;
            if let Some(ms) = s.markets.live.get(epic.as_str()) {
                (
                    ms.bid,
                    ms.ask,
                    (ms.bid + ms.ask) / 2.0,
                    ms.market_state.clone(),
                )
            } else {
                continue;
            }
        };

        if bid <= 0.0 || offer <= 0.0 {
            continue;
        }

        if let Some(ref state_str) = mkt_state {
            let upper = state_str.to_ascii_uppercase();
            if !upper.starts_with("TRADEABLE") {
                continue;
            }
        }

        // Read current regime
        let regime_str = crate::regime::read_regime(epic.as_str())
            .map(|r| r.kind.to_string())
            .unwrap_or_default();
        let is_volatile_regime = regime_str == "VOLATILE";

        // Run M15 strategies (Phase 17.E — promote per-strategy result to info!
        // so we can see WHICH strategy is firing/silent — diagnostic for the
        // "stuck at 1/3 consensus" issue blocking trades since Apr 28).
        let mut signals: Vec<Signal> = Vec::new();
        let mut fired_names: Vec<&str> = Vec::new();
        let mut silent_names: Vec<&str> = Vec::new();
        for strategy in m15_strategies {
            match strategy.evaluate_m15(epic, mid_price, &m15_snap, &h1_snap, &regime_str) {
                Some(sig) => {
                    info!(
                        "[M15] [{}] FIRE {}: {:?} strength={:.1}",
                        epic,
                        strategy.name(),
                        sig.direction,
                        sig.strength
                    );
                    fired_names.push(strategy.name());
                    signals.push(sig);
                }
                None => {
                    silent_names.push(strategy.name());
                }
            }
        }

        info!(
            "[M15] [{}] Bar analysis: {}/{} fired [{}] silent [{}]",
            epic,
            signals.len(),
            m15_strategies.len(),
            fired_names.join(","),
            silent_names.join(",")
        );

        if signals.is_empty() {
            continue;
        }

        // ── Per-Instrument Gate (Gold exceptions etc.) ────────────────────────
        // Applied before ensemble vote: filters/suppresses signals based on
        // per-instrument rules defined in InstrumentStrategyOverride config.
        // Gates (in order): daily limit → ATR ceiling → mean-rev suppression
        //                   → RSI extreme block → ADX trend-lock
        //
        // ADX fallback: when H1 ADX is unavailable (403 quota / cold start),
        // use M15 ADX as a proxy so mean-rev suppression still activates.
        let mut mean_rev_suppressed = false; // set true when Gate 3 removes signals
        {
            let ov_opt = config
                .strategies
                .instrument_overrides
                .get(epic.as_str())
                .cloned();
            if let Some(ref ov) = ov_opt {
                // Gate 1: per-instrument daily trade limit
                if let Some(max_daily) = ov.max_daily_trades {
                    let today_count = {
                        let s = state.read().await;
                        *s.metrics
                            .daily
                            .trades_by_epic
                            .get(epic.as_str())
                            .unwrap_or(&0)
                    };
                    if today_count >= max_daily {
                        info!(
                            "[M15] [{}] Per-instrument daily limit reached: {}/{} — skipping",
                            epic, today_count, max_daily
                        );
                        continue;
                    }
                }

                // Gate 2: ATR% volatility ceiling
                if let Some(atr_ceiling) = ov.atr_pct_max_entry {
                    if let Some(atr) = h1_snap.atr {
                        if mid_price > 0.0 {
                            let atr_pct = (atr / mid_price) * 100.0;
                            if atr_pct > atr_ceiling {
                                warn!(
                                    "[M15] [{}] ATR%={:.2} > max {:.2} — too volatile, skipping",
                                    epic, atr_pct, atr_ceiling
                                );
                                continue;
                            }
                        }
                    }
                }

                // Gate 3: Mean-reversion signal suppression in strong trends
                // When ADX > threshold, multiply RSI_Reversal and Bollinger strength
                // by the weight factor (0.0 = silence, 0.3 = heavily penalise).
                let mean_rev_strategies =
                    ["RSI_Reversal", "Bollinger_Bands", "M15_BollingerReversion"];
                if let (Some(suppress_weight), Some(suppress_adx)) = (
                    ov.mean_reversion_weight_in_strong_trend,
                    ov.mean_reversion_suppress_adx_min,
                ) {
                    // ADX fallback: H1 ADX not yet available (cold start / 403 quota) →
                    // fall back to M15 ADX so suppression still fires in strong trends.
                    let effective_adx = h1_snap.adx.or(m15_snap.adx);
                    if let Some(adx) = effective_adx {
                        if adx >= suppress_adx {
                            let adx_source = if h1_snap.adx.is_some() {
                                "H1"
                            } else {
                                "M15↑"
                            };
                            for sig in signals.iter_mut() {
                                if mean_rev_strategies.contains(&sig.strategy.as_str()) {
                                    let old = sig.strength;
                                    sig.strength *= suppress_weight;
                                    debug!(
                                        "[M15] [{}] Mean-rev suppression ({} ADX={:.1}>={:.1}): \
                                         {} strength {:.1}→{:.1}",
                                        epic,
                                        adx_source,
                                        adx,
                                        suppress_adx,
                                        sig.strategy,
                                        old,
                                        sig.strength
                                    );
                                }
                            }
                            // Remove fully-silenced signals (strength ≤ 0)
                            let before_count = signals.len();
                            signals.retain(|s| s.strength > 0.0);
                            if signals.len() < before_count {
                                mean_rev_suppressed = true; // lower consensus threshold later
                                info!(
                                    "[M15] [{}] Mean-rev suppression removed {}/{} signals \
                                     ({} ADX={:.1}>={}). Consensus will be lowered to 1.",
                                    epic,
                                    before_count - signals.len(),
                                    before_count,
                                    adx_source,
                                    adx,
                                    suppress_adx
                                );
                            }
                            if signals.is_empty() {
                                info!(
                                    "[M15] [{}] All signals silenced by mean-rev suppression \
                                     ({} ADX={:.1})",
                                    epic, adx_source, adx
                                );
                                continue;
                            }
                        }
                    }
                }

                // Gate 4: RSI extreme block
                // When ADX > rsi_extreme_block_adx_min AND RSI is at an extreme,
                // block mean-reversion signals in the "catching the knife" direction.
                // ADX/RSI fallback: use M15 values when H1 not yet warmed up.
                if let (Some(adx), Some(rsi)) =
                    (h1_snap.adx.or(m15_snap.adx), h1_snap.rsi.or(m15_snap.rsi))
                {
                    let adx_min = ov.rsi_extreme_block_adx_min.unwrap_or(f64::MAX);
                    if adx >= adx_min {
                        if let Some(floor) = ov.rsi_extreme_oversold_floor {
                            if rsi <= floor {
                                let before = signals.len();
                                signals.retain(|sig| {
                                    let blocked = sig.direction == Direction::Buy
                                        && mean_rev_strategies.contains(&sig.strategy.as_str());
                                    if blocked {
                                        warn!(
                                            "[M15] [{}] RSI extreme block: RSI={:.1}<={:.1} \
                                             ADX={:.1} — blocked {} BUY from {}",
                                            epic, rsi, floor, adx, sig.direction, sig.strategy
                                        );
                                    }
                                    !blocked
                                });
                                if signals.len() < before {
                                    info!(
                                        "[M15] [{}] RSI oversold block removed {} mean-rev BUY(s) \
                                         (RSI={:.1}, ADX={:.1})",
                                        epic,
                                        before - signals.len(),
                                        rsi,
                                        adx
                                    );
                                }
                            }
                        }
                        if let Some(ceiling) = ov.rsi_extreme_overbought_ceiling {
                            if rsi >= ceiling {
                                let before = signals.len();
                                signals.retain(|sig| {
                                    let blocked = sig.direction == Direction::Sell
                                        && mean_rev_strategies.contains(&sig.strategy.as_str());
                                    if blocked {
                                        warn!(
                                            "[M15] [{}] RSI extreme block: RSI={:.1}>={:.1} \
                                             ADX={:.1} — blocked {} SELL from {}",
                                            epic, rsi, ceiling, adx, sig.direction, sig.strategy
                                        );
                                    }
                                    !blocked
                                });
                                if signals.len() < before {
                                    info!(
                                        "[M15] [{}] RSI overbought block removed {} mean-rev \
                                         SELL(s) (RSI={:.1}, ADX={:.1})",
                                        epic,
                                        before - signals.len(),
                                        rsi,
                                        adx
                                    );
                                }
                            }
                        }
                    }
                }

                // Gate 5: ADX trend-lock
                // When ADX > threshold, block signals that oppose the DI-dominant direction.
                // Uses ema_short > ema_long as DI proxy (uptrend = ema_short above ema_long).
                if ov.adx_trend_lock_enabled {
                    if let Some(adx_thresh) = ov.adx_trend_lock_threshold {
                        if let Some(adx) = h1_snap.adx {
                            if adx >= adx_thresh {
                                // Determine dominant direction from 3-bar H1 price slope.
                                // Price slope is more current than EMA crossover in fast trends.
                                // EMA lags — in a 95-pt crash it may still read "uptrend".
                                // Fallback: EMA crossover if insufficient history.
                                let h1_lock_closes: Vec<f64> = {
                                    let s = state.read().await;
                                    s.markets
                                        .history
                                        .get_candles(epic.as_str(), "HOUR")
                                        .map(|v| v.iter().rev().take(4).map(|c| c.close).collect())
                                        .unwrap_or_default()
                                };
                                let (trend_up, trend_down) = if h1_lock_closes.len() >= 3 {
                                    let anchor = h1_lock_closes[h1_lock_closes.len() - 1];
                                    let slope = if anchor > 0.0 {
                                        (h1_lock_closes[0] - anchor) / anchor
                                    } else {
                                        0.0
                                    };
                                    (slope > 0.001, slope < -0.001)
                                } else {
                                    // fallback to EMA crossover
                                    let up = h1_snap
                                        .ema_short
                                        .zip(h1_snap.ema_long)
                                        .map(|(s, l)| s > l)
                                        .unwrap_or(false);
                                    let dn = h1_snap
                                        .ema_short
                                        .zip(h1_snap.ema_long)
                                        .map(|(s, l)| s < l)
                                        .unwrap_or(false);
                                    (up, dn)
                                };
                                if trend_up || trend_down {
                                    let before = signals.len();
                                    signals.retain(|sig| {
                                        let counter = (trend_up && sig.direction == Direction::Sell)
                                            || (trend_down && sig.direction == Direction::Buy);
                                        if counter {
                                            warn!(
                                                "[M15] [{}] ADX trend-lock ({:.1}>={:.1}, trend={}) \
                                                 — blocked {:?} from {}",
                                                epic, adx, adx_thresh,
                                                if trend_up { "UP" } else { "DOWN" },
                                                sig.direction, sig.strategy
                                            );
                                        }
                                        !counter
                                    });
                                    if signals.len() < before {
                                        info!(
                                            "[M15] [{}] ADX trend-lock: {} signal(s) removed \
                                             (ADX={:.1}, trend={})",
                                            epic,
                                            before - signals.len(),
                                            adx,
                                            if trend_up { "UP" } else { "DOWN" }
                                        );
                                    }
                                }
                            }
                        }
                    }
                }

                if signals.is_empty() {
                    debug!(
                        "[M15] [{}] No signals remaining after instrument gates",
                        epic
                    );
                    continue;
                }
            }
        }
        // ── End Per-Instrument Gate ───────────────────────────────────────────

        // Apply M15 regime multipliers
        apply_m15_regime_multipliers(&mut signals, &regime_str);

        // ── H1 Alignment Bonus (Phase 14.E) ──────────────────────────────────
        // When an M15 signal agrees with the H1 directional bias, boost its
        // strength before the ensemble vote.  This rewards timeframe confluence
        // and helps aligned signals clear the avg_strength threshold more easily.
        // Example: 7.0 × 1.2 = 8.4 — a borderline signal becomes a strong one.
        let h1_alignment_bonus = config.strategies.h1_alignment_bonus;
        if h1_alignment_bonus > 1.0 {
            let h1_dir: Option<Direction> = {
                let s = state.read().await;
                s.markets
                    .h1_bias
                    .get(epic.as_str())
                    .and_then(|b| b.direction.clone())
            };
            if let Some(ref h1_direction) = h1_dir {
                let mut boosted = 0usize;
                for sig in signals.iter_mut() {
                    if &sig.direction == h1_direction {
                        let orig = sig.strength;
                        sig.strength = (sig.strength * h1_alignment_bonus).min(10.0);
                        debug!(
                            "[M15] {} H1 alignment bonus: {} strength {:.1} → {:.1} (×{:.1})",
                            epic, sig.strategy, orig, sig.strength, h1_alignment_bonus
                        );
                        boosted += 1;
                    }
                }
                if boosted > 0 {
                    info!(
                        "[M15] {} — {} signal(s) boosted ×{:.1} (H1 {:?} alignment)",
                        epic, boosted, h1_alignment_bonus, h1_direction
                    );
                }
            }
        }

        // Ensemble vote — use per-instrument consensus/strength override if configured.
        // Strong-trend exception: when mean-rev signals were suppressed (mean_rev_suppressed=true),
        // drop min_consensus to 1. Rationale: after removing RSI/BB mean-rev signals in high-ADX
        // trends, the remaining signals are pure momentum — requiring 2/3 of the ORIGINAL count
        // unfairly penalises the remaining signal(s). All surviving signals must agree (handled
        // by vote_with_overrides), so consensus=1 just means "at least one momentum signal agrees".
        //
        // VOLATILE regime exception: in VOLATILE only 1 out of 3 M15 strategies fires per bar,
        // so a min_consensus=2 baseline would permanently block all M15 trades. We relax by
        // subtracting 1 (floor 1) so that a single strong signal can open a position.
        let maybe_signal = {
            let ov_opt = config.strategies.instrument_overrides.get(epic.as_str());
            let override_consensus = if mean_rev_suppressed {
                // Override to 1: require just 1 momentum signal after mean-rev suppression
                Some(1usize)
            } else {
                ov_opt.and_then(|o| o.min_consensus)
            };
            // VOLATILE fallback: relax consensus by 1 (min 1) so a single M15 signal suffices.
            let override_consensus = if regime_str == "VOLATILE" {
                let base = override_consensus.unwrap_or(m15_ensemble.min_consensus);
                let relaxed = base.saturating_sub(1).max(1);
                if relaxed < base {
                    info!(
                        "[M15] {} VOLATILE consensus relaxed: {} → {}",
                        epic, base, relaxed
                    );
                }
                Some(relaxed)
            } else {
                override_consensus
            };
            let override_strength = ov_opt.and_then(|o| o.min_avg_strength);
            if override_consensus.is_some() || override_strength.is_some() {
                let mut local = m15_ensemble.clone();
                if let Some(c) = override_consensus {
                    local.min_consensus = c;
                }
                if let Some(s) = override_strength {
                    local.min_avg_strength = s;
                }
                local.vote(&signals)
            } else {
                m15_ensemble.vote(&signals)
            }
        };

        if let Some(ensemble_signal) = maybe_signal {
            info!(
                "[M15] Ensemble signal: {} {} strength={:.2}",
                ensemble_signal.direction, epic, ensemble_signal.strength
            );

            // Check macro pause
            let macro_paused = {
                let s = state.read().await;
                s.metrics
                    .macro_pause_until
                    .map(|until| chrono::Utc::now() < until)
                    .unwrap_or(false)
            };
            if macro_paused {
                warn!("[M15] {} — macro pause active, skipping", epic);
                continue;
            }

            // Check M15 cooldown (max trades per H1 candle boundary)
            let h1_ts = (Utc::now().timestamp() / 3600) * 3600;
            let can_trade = {
                let s = state.read().await;
                s.m15_cooldown.can_trade(
                    epic.as_str(),
                    h1_ts,
                    config.strategies.m15_max_trades_per_h1,
                )
            };
            if !can_trade {
                info!(
                    "[M15] {} — cooldown: max {} M15 trades/H1 candle reached",
                    epic, config.strategies.m15_max_trades_per_h1
                );
                continue;
            }

            // ── ADX Price-Slope Bypass (Gold strong-trend fix) ────────────────
            // Problem: In strong trends (Gold ADX=62, RSI=8), H1 mean-reversion
            // strategies vote BUY (oversold), contaminating h1_bias → H1 gate
            // blocks valid M15 SELL signals even as price crashes 284 pts.
            // Fix: when ADX > instrument threshold, compute 3-bar H1 price slope.
            // If slope AGREES with the M15 signal, bypass the H1 strategy-vote gate.
            // This lets price momentum override mean-reversion bias in strong trends.
            let bypass_h1_gate = {
                let mut bypass = false;
                if config.strategies.h1_direction_gate_enabled {
                    if let Some(ov) = config.strategies.instrument_overrides.get(epic.as_str()) {
                        if ov.adx_trend_lock_enabled {
                            if let Some(thresh) = ov.adx_trend_lock_threshold {
                                // ADX fallback: use M15 ADX when H1 not yet warmed up.
                                let adx = h1_snap.adx.or(m15_snap.adx).unwrap_or(0.0);
                                let adx_src = if h1_snap.adx.is_some() {
                                    "H1"
                                } else {
                                    "M15↑"
                                };
                                if adx >= thresh {
                                    // Read last 4 H1 closes (most recent first) to compute slope
                                    let h1_closes: Vec<f64> = {
                                        let s = state.read().await;
                                        s.markets
                                            .history
                                            .get_candles(epic.as_str(), "HOUR")
                                            .map(|v| {
                                                v.iter().rev().take(4).map(|c| c.close).collect()
                                            })
                                            .unwrap_or_default()
                                    };
                                    if h1_closes.len() >= 3 {
                                        let anchor = h1_closes[h1_closes.len() - 1];
                                        let slope_pct = if anchor > 0.0 {
                                            (h1_closes[0] - anchor) / anchor
                                        } else {
                                            0.0
                                        };
                                        // 0.1% threshold: clear directional move over 3 H1 bars
                                        let slope_sell = slope_pct < -0.001;
                                        let slope_buy = slope_pct > 0.001;
                                        let sig_sell = ensemble_signal.direction == Direction::Sell;
                                        let sig_buy = ensemble_signal.direction == Direction::Buy;
                                        if (slope_sell && sig_sell) || (slope_buy && sig_buy) {
                                            bypass = true;
                                            info!(
                                                "[M15] [{}] {} ADX={:.1}>={:.1} price-slope bypass: \
                                                 slope={:+.3}% AGREES with {:?} — overriding H1 strategy-vote gate",
                                                epic, adx_src, adx, thresh, slope_pct * 100.0,
                                                ensemble_signal.direction
                                            );
                                        } else if slope_sell || slope_buy {
                                            // Slope exists but DISAGREES with signal — block it harder
                                            let slope_dir = if slope_sell { "DOWN" } else { "UP" };
                                            warn!(
                                                "[M15] [{}] {} ADX={:.1}>={:.1} price-slope block: \
                                                 slope={:+.3}% ({}) conflicts {:?} — blocking",
                                                epic, adx_src, adx, thresh, slope_pct * 100.0,
                                                slope_dir, ensemble_signal.direction
                                            );
                                            let mut s = state.write().await;
                                            s.add_signal_record(ensemble_signal.clone(), false,
                                                Some(format!("{adx_src} ADX={:.1} price-slope {slope_dir} conflicts {:?}",
                                                    adx, ensemble_signal.direction)));
                                            continue;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                bypass
            };

            // ── H1 Direction Gate (Phase 14.E / 16.A) ────────────────────────
            // Block M15 entries that contradict the prevailing H1 bias.
            // Phase 16.A: when require_h1_confirmation=true, also block during cold
            // start (no H1 data yet) and when H1 ran but zero strategies fired.
            // Skipped when bypass_h1_gate=true (ADX strong-trend price-slope agrees).
            // Phase 17.A: VOLATILE cold-start bypass — when H1 is not yet warmed and
            // the regime is VOLATILE (where H1 direction is choppy / unreliable), allow
            // strong M15 signals (strength >= 8.0) through rather than sitting idle for
            // up to 1 hour after every restart.
            if config.strategies.h1_direction_gate_enabled && !bypass_h1_gate {
                debug!(
                    "[M15] [{}] H1 gate check: regime={}, strength={:.2}, bypass=false",
                    epic, regime_str, ensemble_signal.strength
                );
                let blocked_reason: Option<String> = {
                    let s = state.read().await;
                    match s.markets.h1_bias.get(epic.as_str()) {
                        None => {
                            // Cold start: H1 analysis has not run yet for this epic.
                            // VOLATILE bypass: H1 direction is unreliable in choppy regimes,
                            // so a high-strength M15 signal is sufficient on its own.
                            const VOLATILE_COLD_START_BYPASS_STRENGTH: f64 = 8.0;
                            if is_volatile_regime
                                && ensemble_signal.strength
                                    >= VOLATILE_COLD_START_BYPASS_STRENGTH
                            {
                                tracing::info!(
                                    "[M15] {} VOLATILE cold-start bypass: H1 not warmed, allowing strong signal (strength={:.2})",
                                    epic,
                                    ensemble_signal.strength
                                );
                                None
                            } else if config.strategies.require_h1_confirmation {
                                Some(format!(
                                    "H1 direction gate: no H1 data yet (cold start) — blocking M15 {:?} until H1 warms up",
                                    ensemble_signal.direction
                                ))
                            } else {
                                None
                            }
                        }
                        Some(bias) if bias.buy_count == 0 && bias.sell_count == 0 => {
                            // H1 ran but no strategies fired — direction unknown.
                            // In VOLATILE regime, high-strength signals are reliable enough to trade
                            // without H1 confirmation (same as cold-start bypass).
                            const VOLATILE_BYPASS_STRENGTH: f64 = 8.0;
                            if is_volatile_regime && ensemble_signal.strength >= VOLATILE_BYPASS_STRENGTH {
                                tracing::info!(
                                    "[M15] {} VOLATILE H1-zero bypass: H1 has 0 signals but strength={:.2} >= {}, allowing trade",
                                    epic,
                                    ensemble_signal.strength,
                                    VOLATILE_BYPASS_STRENGTH
                                );
                                None
                            } else if config.strategies.require_h1_confirmation {
                                Some(format!(
                                    "H1 direction gate: H1 total signals = 0 (direction unknown) — blocking M15 {:?}",
                                    ensemble_signal.direction
                                ))
                            } else {
                                None
                            }
                        }
                        Some(bias) => {
                            // Normal: H1 has data — check for directional conflict.
                            match (&bias.direction, &ensemble_signal.direction) {
                                (Some(Direction::Buy), Direction::Sell) => Some(format!(
                                    "H1 direction gate: H1 leans BUY ({} buy vs {} sell strategies) — blocking M15 SELL",
                                    bias.buy_count, bias.sell_count
                                )),
                                (Some(Direction::Sell), Direction::Buy) => Some(format!(
                                    "H1 direction gate: H1 leans SELL ({} sell vs {} buy strategies) — blocking M15 BUY",
                                    bias.sell_count, bias.buy_count
                                )),
                                _ => None,
                            }
                        }
                    }
                };
                if let Some(reason) = blocked_reason {
                    warn!("[M15] {} — {}", epic, reason);
                    let mut s = state.write().await;
                    s.add_signal_record(ensemble_signal.clone(), false, Some(reason));
                    continue;
                }
            }

            let can_trade_engine = {
                let s = state.read().await;
                s.can_trade()
            };
            if !can_trade_engine {
                continue;
            }

            // ── Post-trade cooldown gate (M15) ───────────────────────────────
            let in_cooldown_m15 = {
                let s = state.read().await;
                s.is_in_cooldown(epic.as_str())
            };
            if in_cooldown_m15 {
                warn!(
                    "[M15][{}] Re-entry blocked — post-trade cooldown active",
                    epic
                );
                let mut s = state.write().await;
                s.add_signal_record(
                    ensemble_signal.clone(),
                    false,
                    Some("Post-trade cooldown active".to_string()),
                );
                continue;
            }

            let (account_info, account_currency) = {
                let s = state.read().await;
                (
                    crate::risk::AccountInfo {
                        balance: s.account.balance,
                        equity: s.account.equity,
                        available_margin: s.account.available,
                    },
                    s.account.currency.clone(),
                )
            };

            let open_positions: Vec<crate::risk::OpenPosition> = {
                let s = state.read().await;
                s.trades
                    .active
                    .iter()
                    .map(|p| crate::risk::OpenPosition {
                        epic: p.epic.clone(),
                        direction: p.direction.to_string(),
                        size: p.size,
                        entry_price: p.open_price,
                        stop_loss: p.stop_loss.unwrap_or(0.0),
                        take_profit: p.take_profit.unwrap_or(0.0),
                    })
                    .collect()
            };

            let direction_str = ensemble_signal.direction.to_string();
            let verdict = risk_manager.check_trade_m15(
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
                    // Record cooldown before execution attempt
                    {
                        let mut s = state.write().await;
                        s.m15_cooldown.record_trade(epic.as_str(), h1_ts);
                    }

                    if config.general.mode != EngineMode::Paper {
                        match order_manager
                            .execute_trade(client, &adjusted_trade, &account_currency)
                            .await
                        {
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
                                    currency: account_currency.clone(),
                                    strategy: adjusted_trade.strategy.clone(),
                                    opened_at: Utc::now(),
                                    is_virtual: false,
                                    opened_in_regime: if regime_str.is_empty() {
                                        None
                                    } else {
                                        Some(regime_str.clone())
                                    },
                                };
                                {
                                    let mut s = state.write().await;
                                    s.trades.active.push(position);
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
                                    let _ = tg
                                        .send_trade_alert(
                                            &t_epic, &t_dir, t_size, t_price, t_sl, t_tp,
                                        )
                                        .await;
                                });
                            }
                            Err(e) => {
                                error!("[M15] Failed to execute trade: {}", e);
                                let mut s = state.write().await;
                                s.add_signal_record(
                                    ensemble_signal.clone(),
                                    false,
                                    Some(format!("M15 execution failed: {}", e)),
                                );
                                // Short cooldown on failure — prevents retry on next bar
                                s.set_trade_cooldown(&ensemble_signal.epic, 300);
                                // 5 min
                            }
                        }
                    } else {
                        // Paper mode: virtual position
                        info!("[M15] Paper mode: virtual M15 position for {}", epic);
                        let position = Position {
                            deal_id: format!("m15_shadow_{}", ensemble_signal.id),
                            deal_reference: format!("m15_shadow_{}", ensemble_signal.id),
                            epic: ensemble_signal.epic.clone(),
                            direction: ensemble_signal.direction.clone(),
                            size: adjusted_trade.size,
                            open_price: mid_price,
                            stop_loss: Some(adjusted_trade.stop_loss),
                            take_profit: Some(adjusted_trade.take_profit),
                            trailing_stop: adjusted_trade.trailing_stop_distance,
                            current_price: mid_price,
                            pnl: 0.0,
                            currency: account_currency.clone(),
                            strategy: adjusted_trade.strategy.clone(),
                            opened_at: Utc::now(),
                            is_virtual: true,
                            opened_in_regime: if regime_str.is_empty() {
                                None
                            } else {
                                Some(regime_str.clone())
                            },
                        };
                        {
                            let mut s = state.write().await;
                            s.trades.active.push(position);
                            s.add_signal_record(
                                ensemble_signal.clone(),
                                true,
                                Some("M15 Paper mode".to_string()),
                            );
                        }
                    }
                }
                crate::risk::RiskVerdict::Rejected(reason) => {
                    warn!("[M15] Trade rejected: {}", reason);
                    let mut s = state.write().await;
                    s.add_signal_record(
                        ensemble_signal.clone(),
                        false,
                        Some(format!("M15 risk rejected: {}", reason)),
                    );
                }
            }
        }
    }

    Ok(())
}

/// Apply M15-specific regime multipliers to signal strengths.
///
/// These are separate from the H1 regime multipliers in `apply_regime_multipliers()`
/// since M15 strategies have different regime sensitivities.
fn apply_m15_regime_multipliers(signals: &mut [Signal], regime: &str) {
    for sig in signals.iter_mut() {
        let multiplier = match (sig.strategy.as_str(), regime) {
            ("M15_MomentumBurst", "VOLATILE") => 1.3,
            ("M15_MomentumBurst", "TRENDING") => 1.2,
            ("M15_MomentumBurst", "RANGING") => 1.0, // allowed at RSI extremes only
            ("M15_EmaMicrotrend", "TRENDING") => 1.2,
            ("M15_EmaMicrotrend", "VOLATILE") => 1.2, // EMA slope confirms volatile move direction
            ("M15_BollingerReversion", "RANGING") => 1.2,
            _ => 1.0,
        };
        if multiplier != 1.0 {
            sig.strength = (sig.strength * multiplier).min(10.0);
        }
    }
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
    let raw = std::fs::read_to_string(file_path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&raw).ok()?;

    let ts = json["timestamp"].as_i64()?;
    let score = json["score"].as_f64()?;
    let confidence = json["confidence"].as_f64().unwrap_or(0.5);
    let mode = json["mode"].as_str().unwrap_or("unknown").to_string();
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
        debug!(
            "Gold sentiment file is stale ({} s old) — skipping",
            age_secs
        );
        return None;
    }

    // ── Score threshold gate ───────────────────────────────────────────────────
    const THRESHOLD: f64 = 0.55;
    let direction = if score >= THRESHOLD {
        Direction::Buy
    } else if score <= -THRESHOLD {
        Direction::Sell
    } else {
        debug!(
            "Gold sentiment score {:.3} below threshold ±{} — skipping",
            score, THRESHOLD
        );
        return None;
    };

    // ── Signal strength: 6.0 (min consensus) + confidence bonus up to +3.5 ───
    let strength = (6.0_f64 + confidence * 3.5).min(9.5);

    // ── SL / TP from ATR, falling back to 0.5 % distance ─────────────────────
    let sl_mult = config.strategies.default_atr_sl_multiplier;
    let tp_mult = config.strategies.default_atr_tp_multiplier;

    let (stop_loss, take_profit) = match (atr, &direction) {
        (Some(a), Direction::Buy) => (mid_price - a * sl_mult, mid_price + a * tp_mult),
        (Some(a), Direction::Sell) => (mid_price + a * sl_mult, mid_price - a * tp_mult),
        (None, Direction::Buy) => {
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
        id: uuid::Uuid::new_v4().to_string(),
        epic: "CS.D.CFIGOLD.CFI.IP".to_string(),
        direction,
        strength,
        strategy: "Gold_Sentiment".to_string(),
        reason,
        price: mid_price,
        stop_loss,
        take_profit,
        trailing_stop_distance: None,
        timestamp: Utc::now(),
    })
}
