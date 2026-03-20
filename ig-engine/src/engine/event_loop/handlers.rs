use anyhow::Result;
use chrono::Utc;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tracing::{debug, error, info};

use crate::api::rest_client::IGRestClient;
use crate::engine::event_loop::learning::build_learning_snapshot;
use crate::engine::state::{get_instrument_name, ClosedTrade, Direction, EngineState};
use crate::ipc::events::EngineEvent;
use crate::notifications::telegram::TelegramNotifier;
use crate::strategy::ensemble::EnsembleVoter;

pub async fn handle_position_monitoring(
    state: &Arc<RwLock<EngineState>>,
    client: &mut IGRestClient,
    order_manager: &crate::engine::order_manager::OrderManager,
    event_tx: &broadcast::Sender<EngineEvent>,
    telegram: &TelegramNotifier,
    ensemble: &mut EnsembleVoter,
) -> Result<()> {
    let (positions_to_close, stop_updates, cooldown_secs) = {
        let mut s = state.write().await;
        let mut to_close = Vec::new();
        let mut stop_updates = Vec::new();

        let price_map: std::collections::HashMap<String, f64> = s
            .markets
            .live
            .iter()
            .map(|(epic, ms)| (epic.clone(), (ms.bid + ms.ask) / 2.0))
            .collect();

        // Clone the small instrument_specs map so we can hold it alongside a
        // mutable borrow of s.trades.active without a borrow conflict.
        let instrument_specs = s.config.risk.instrument_specs.clone();
        let trailing_stop_min_pips = s.config.risk.trailing_stop_min_pips;
        // Read config values here — cannot acquire another lock inside this write lock
        let volatile_breakeven_trigger = s.config.risk.volatile_breakeven_trigger;
        let cooldown_secs = s.config.strategies.post_trade_cooldown_secs;

        for (idx, position) in s.trades.active.iter_mut().enumerate() {
            if let Some(&current_price) = price_map.get(&position.epic) {
                position.current_price = current_price;

                // Use pip_value (SGD per pip per lot) for correct account-currency PnL.
                // pip_value already encodes the FX conversion to SGD (e.g. USD/SGD, JPY/SGD).
                let spec = instrument_specs
                    .get(&position.epic)
                    .cloned()
                    .or_else(|| crate::risk::InstrumentSpec::from_epic_fallback(&position.epic));
                let (pip_scale, pip_value) = spec
                    .map(|s| (s.pip_scale, s.pip_value))
                    .unwrap_or((0.0001, 1.0));
                let price_diff = if position.direction == Direction::Buy {
                    current_price - position.open_price
                } else {
                    position.open_price - current_price
                };
                position.pnl = (price_diff / pip_scale) * pip_value * position.size;

                if let Some(trail_dist) = position.trailing_stop {
                    // ── 13.2 Management Personalities ────────────────────────────
                    // Adjust ratchet behaviour based on the regime when the trade was opened.
                    //
                    // VOLATILE birth → aggressive break-even: once price has moved trail_dist
                    //   in our favour, snap SL to open_price immediately (micro-stop).
                    // TRENDING birth + current VOLATILE regime → preserve wide stop (skip
                    //   ratchet tightening so we don't get stopped out by whipsaw noise).
                    let birth_regime = position.opened_in_regime.as_deref().unwrap_or("UNKNOWN");
                    let current_regime =
                        crate::regime::read_regime(&position.epic).map(|r| r.kind.to_string());
                    let current_is_volatile = current_regime.as_deref() == Some("VOLATILE");

                    // VOLATILE birth: snap to break-even aggressively
                    // ── 15.C Early Break-even ─────────────────────────────────────
                    // Lesson: trades were sitting in significant profit (88–117 pips)
                    // but the old trigger required 100% of trail_dist (237 pips) before
                    // snapping to BE. Price reversed, turning profit into loss.
                    // Fix: trigger BE at `volatile_breakeven_trigger` fraction of trail_dist
                    // (default 0.3 = 30%, e.g. 71 pips on a 237-pip SL).
                    if birth_regime == "VOLATILE" {
                        let profit_dist = match position.direction {
                            Direction::Buy => current_price - position.open_price,
                            Direction::Sell => position.open_price - current_price,
                        };
                        let be_trigger = volatile_breakeven_trigger;
                        let trigger_dist = trail_dist * be_trigger;
                        if profit_dist >= trigger_dist {
                            let be_sl = position.open_price;
                            let already_protected = match position.direction {
                                Direction::Buy => {
                                    position.stop_loss.map(|sl| sl >= be_sl).unwrap_or(false)
                                }
                                Direction::Sell => {
                                    position.stop_loss.map(|sl| sl <= be_sl).unwrap_or(false)
                                }
                            };
                            if !already_protected {
                                info!(
                                    "[{}] VOLATILE BE snap: profit={:.5} >= {:.0}% of trail_dist={:.5} → SL locked to breakeven {:.5}",
                                    position.epic, profit_dist, be_trigger * 100.0, trail_dist, be_sl
                                );
                                position.stop_loss = Some(be_sl);
                                stop_updates.push((position.clone(), be_sl));
                            }
                        }
                    }

                    // Reduce API spam by requiring SL to move by at least X pips
                    let spec = instrument_specs.get(&position.epic).cloned().or_else(|| {
                        crate::risk::InstrumentSpec::from_epic_fallback(&position.epic)
                    });
                    let min_step = spec
                        .map(|sp| sp.pip_scale * trailing_stop_min_pips)
                        .unwrap_or(0.0005);

                    let new_sl = match position.direction {
                        Direction::Buy => current_price - trail_dist,
                        Direction::Sell => current_price + trail_dist,
                    };

                    // TRENDING birth in VOLATILE current regime: skip ratchet tightening
                    // (preserve wide stop so whipsaw doesn't prematurely close the trade).
                    if birth_regime == "TRENDING" && current_is_volatile {
                        debug!(
                            "TRENDING-birth management: skipping ratchet tightening for {} (current regime VOLATILE)",
                            position.epic
                        );
                    } else {
                        let should_update = match position.direction {
                            Direction::Buy => {
                                position.stop_loss.is_none_or(|sl| new_sl >= sl + min_step)
                            }
                            Direction::Sell => {
                                position.stop_loss.is_none_or(|sl| new_sl <= sl - min_step)
                            }
                        };
                        if should_update {
                            debug!(
                                "Trailing SL ratchet for {}: {:.5} -> {:.5}",
                                position.epic,
                                position.stop_loss.unwrap_or(0.0),
                                new_sl
                            );
                            position.stop_loss = Some(new_sl);
                            stop_updates.push((position.clone(), new_sl));
                        }
                    }
                }

                if let Some(stop_loss) = position.stop_loss {
                    if (position.direction == Direction::Buy && current_price <= stop_loss)
                        || (position.direction == Direction::Sell && current_price >= stop_loss)
                    {
                        to_close.push((idx, position.clone(), "Stop Loss"));
                    }
                }

                if let Some(take_profit) = position.take_profit {
                    if (position.direction == Direction::Buy && current_price >= take_profit)
                        || (position.direction == Direction::Sell && current_price <= take_profit)
                    {
                        to_close.push((idx, position.clone(), "Take Profit"));
                    }
                }
            }
        }

        // Filter out positions that are about to be closed from the stop_updates list
        let to_close_ids: std::collections::HashSet<String> =
            to_close.iter().map(|(_, p, _)| p.deal_id.clone()).collect();
        stop_updates.retain(|(p, _)| !to_close_ids.contains(&p.deal_id));

        for (idx, _, _) in to_close.iter().rev() {
            s.trades.active.remove(*idx);
        }

        // Set cooldown NOW — before dropping the write lock — so the gap between
        // "position removed from active" and "REST close_position() returns" is
        // covered. Without this, a concurrent M15 bar can pass both the
        // "already open" check and the cooldown check and fire a reversal trade.
        for (_, position, _) in &to_close {
            s.set_trade_cooldown(&position.epic, cooldown_secs);
        }

        (to_close, stop_updates, cooldown_secs)
    };

    // 1. Perform stop updates on IG
    for (position, new_sl) in stop_updates {
        if position.is_virtual {
            continue; // Virtual positions already updated in state
        }
        if let Err(e) = order_manager
            .update_stop_loss(client, &position, new_sl)
            .await
        {
            error!(
                "Failed to update trailing stop on IG for {}: {}",
                position.deal_id, e
            );
        }
    }

    for (_, position, reason) in positions_to_close {
        info!(
            "Closing position due to {}: deal_id={}, epic={}, pnl={}",
            reason, position.deal_id, position.epic, position.pnl
        );

        if position.is_virtual {
            let close_pnl = position.pnl;
            {
                let mut s = state.write().await;
                s.record_trade_result_for_epic(close_pnl, Some(&position.epic));
                s.add_closed_trade(ClosedTrade {
                    deal_id: position.deal_id.clone(),
                    epic: position.epic.clone(),
                    direction: position.direction.clone(),
                    size: position.size,
                    entry_price: position.open_price,
                    exit_price: position.current_price,
                    stop_loss: position.stop_loss.unwrap_or(0.0),
                    take_profit: position.take_profit,
                    pnl: close_pnl,
                    strategy: position.strategy.clone(),
                    status: format!("virtual_{}", reason.to_lowercase().replace(' ', "_")),
                    opened_at: position.opened_at,
                    closed_at: Utc::now(),
                    is_virtual: true,
                    opened_in_regime: position.opened_in_regime.clone(), // 13.3
                });

                // Update scorecard for learning
                if let Some(scorecard) = &mut s.learning.scorecard {
                    scorecard.update_virtual(&position, close_pnl);
                }
                // Set re-entry cooldown
                s.set_trade_cooldown(&position.epic, cooldown_secs);
            }

            let _ = event_tx.send(EngineEvent::position_closed(
                position.deal_id.clone(),
                close_pnl,
            ));

            // Send Telegram notification for virtual position close
            let tg = telegram.clone();
            let v_name = get_instrument_name(&position.epic);
            let v_direction = format!("{}", position.direction);
            let v_pnl = close_pnl;
            let v_reason = reason.to_string();
            tokio::spawn(async move {
                let msg = format!(
                    "{} <b>VIRTUAL POSITION CLOSED</b>\n\n<b>Instrument:</b> {}\n<b>Direction:</b> {}\n<b>Reason:</b> {}\n<b>P&L:</b> {:.2}\n<b>Time:</b> {}",
                    if v_pnl >= 0.0 { "✅" } else { "❌" },
                    v_name, v_direction, v_reason, v_pnl,
                    (chrono::Utc::now() + chrono::Duration::hours(8)).format("%H:%M:%S SGT")
                );
                let _ = tg.send_message(&msg).await;
            });
            continue;
        }

        match order_manager.close_position(client, &position).await {
            Ok(close_result) => {
                let closed_trade = ClosedTrade {
                    deal_id: close_result.deal_id.clone(),
                    epic: position.epic.clone(),
                    direction: position.direction.clone(),
                    size: position.size,
                    entry_price: position.open_price,
                    exit_price: close_result.close_price,
                    stop_loss: position.stop_loss.unwrap_or(0.0),
                    take_profit: position.take_profit,
                    pnl: close_result.pnl,
                    strategy: position.strategy.clone(),
                    status: reason.to_lowercase().replace(' ', "_"),
                    opened_at: position.opened_at,
                    closed_at: Utc::now(),
                    is_virtual: position.is_virtual,
                    opened_in_regime: position.opened_in_regime.clone(), // 13.3
                };

                {
                    let mut s = state.write().await;
                    s.record_trade_result_for_epic(close_result.pnl, Some(&position.epic));
                    s.add_closed_trade(closed_trade.clone());
                    // Set re-entry cooldown
                    s.set_trade_cooldown(&position.epic, cooldown_secs);
                }

                {
                    let mut s = state.write().await;
                    if let Some(ref mut scorecard) = s.learning.scorecard {
                        scorecard.update(&closed_trade);
                    }
                    let new_weights = {
                        let state_ref = &mut *s;
                        if let (Some(sc), Some(wm)) = (
                            &state_ref.learning.scorecard,
                            &mut state_ref.learning.weight_manager,
                        ) {
                            wm.maybe_recalculate(sc)
                        } else {
                            None
                        }
                    };
                    if let Some(weights) = new_weights {
                        ensemble.update_weights(weights);
                        info!("🧠 Ensemble weights updated by adaptive learning system");
                    }
                    let snap = {
                        let state_ref = &*s;
                        match (
                            &state_ref.learning.scorecard,
                            &state_ref.learning.weight_manager,
                        ) {
                            (Some(sc), Some(wm)) => Some(build_learning_snapshot(sc, wm)),
                            _ => None,
                        }
                    };
                    if let Some(snap) = snap {
                        s.learning.snapshot = snap;
                    }
                }

                let _ = event_tx.send(EngineEvent::position_closed(
                    close_result.deal_id.clone(),
                    close_result.pnl,
                ));

                let tg = telegram.clone();
                let name = get_instrument_name(&position.epic);
                let direction = format!("{}", position.direction);
                let pnl_val = close_result.pnl;
                let reason_str = reason.to_string();
                tokio::spawn(async move {
                    let msg = format!(
                        "{} <b>POSITION CLOSED</b>\n\n<b>Instrument:</b> {}\n<b>Direction:</b> {}\n<b>Reason:</b> {}\n<b>P&L:</b> {:.2}\n<b>Time:</b> {}",
                        if pnl_val >= 0.0 { "✅" } else { "❌" },
                        name, direction, reason_str, pnl_val,
                        (chrono::Utc::now() + chrono::Duration::hours(8)).format("%H:%M:%S SGT")
                    );
                    let _ = tg.send_message(&msg).await;
                });
            }
            Err(e) => {
                error!("Failed to close position {}: {}", position.deal_id, e);
            }
        }
    }

    // Persist active positions every monitor tick for crash recovery.
    // On next startup, mod.rs compares this snapshot against IG live positions
    // to detect any trades that closed while the engine was offline.
    {
        let s = state.read().await;
        s.save_active_positions();
    }

    Ok(())
}
