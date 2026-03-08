use std::sync::Arc;
use tokio::sync::{RwLock, broadcast};
use anyhow::Result;
use tracing::{info, error, debug};
use chrono::Utc;

use crate::engine::state::{EngineState, Direction, ClosedTrade};
use crate::api::rest_client::IGRestClient;
use crate::strategy::ensemble::EnsembleVoter;
use crate::ipc::events::EngineEvent;
use crate::notifications::telegram::{TelegramNotifier, get_instrument_name};
use crate::engine::event_loop::learning::{build_learning_snapshot};

pub async fn handle_position_monitoring(
    state: &Arc<RwLock<EngineState>>,
    client: &mut IGRestClient,
    order_manager: &crate::engine::order_manager::OrderManager,
    event_tx: &broadcast::Sender<EngineEvent>,
    telegram: &TelegramNotifier,
    ensemble: &mut EnsembleVoter,
) -> Result<()> {
    let (positions_to_close, stop_updates) = {
        let mut s = state.write().await;
        let mut to_close = Vec::new();
        let mut stop_updates = Vec::new();

        let price_map: std::collections::HashMap<String, f64> = s
            .markets.live
            .iter()
            .map(|(epic, ms)| (epic.clone(), (ms.bid + ms.ask) / 2.0))
            .collect();

        // Clone the small instrument_specs map so we can hold it alongside a
        // mutable borrow of s.trades.active without a borrow conflict.
        let instrument_specs = s.config.risk.instrument_specs.clone();

        for (idx, position) in s.trades.active.iter_mut().enumerate() {
            if let Some(&current_price) = price_map.get(&position.epic) {
                position.current_price = current_price;

                position.pnl = if position.direction == Direction::Buy {
                    (current_price - position.open_price) * position.size
                } else {
                    (position.open_price - current_price) * position.size
                };

                if let Some(trail_dist) = position.trailing_stop {
                    // Reduce API spam by requiring SL to move by at least 5 pips
                    let spec = instrument_specs.get(&position.epic)
                        .cloned()
                        .or_else(|| crate::risk::InstrumentSpec::from_epic_fallback(&position.epic));
                    let min_step = spec.map(|sp| sp.pip_scale * 5.0).unwrap_or(0.0005);

                    let new_sl = match position.direction {
                        Direction::Buy => current_price - trail_dist,
                        Direction::Sell => current_price + trail_dist,
                    };
                    
                    let should_update = match position.direction {
                        Direction::Buy => position.stop_loss.map_or(true, |sl| new_sl >= sl + min_step),
                        Direction::Sell => position.stop_loss.map_or(true, |sl| new_sl <= sl - min_step),
                    };
                    if should_update {
                        debug!(
                            "Trailing SL ratchet for {}: {:.5} -> {:.5}",
                            position.epic,
                            position.stop_loss.unwrap_or(0.0),
                            new_sl
                        );
                        position.stop_loss = Some(new_sl);
                        
                        // We will perform the API update outside the lock
                        stop_updates.push((position.clone(), new_sl));
                    }
                }

                if let Some(stop_loss) = position.stop_loss {
                    if position.direction == Direction::Buy && current_price <= stop_loss {
                        to_close.push((idx, position.clone(), "Stop Loss"));
                    } else if position.direction == Direction::Sell && current_price >= stop_loss {
                        to_close.push((idx, position.clone(), "Stop Loss"));
                    }
                }

                if let Some(take_profit) = position.take_profit {
                    if position.direction == Direction::Buy && current_price >= take_profit {
                        to_close.push((idx, position.clone(), "Take Profit"));
                    } else if position.direction == Direction::Sell && current_price <= take_profit {
                        to_close.push((idx, position.clone(), "Take Profit"));
                    }
                }
            }
        }

        // Filter out positions that are about to be closed from the stop_updates list
        let to_close_ids: std::collections::HashSet<String> = to_close.iter().map(|(_, p, _)| p.deal_id.clone()).collect();
        stop_updates.retain(|(p, _)| !to_close_ids.contains(&p.deal_id));

        for (idx, _, _) in to_close.iter().rev() {
            s.trades.active.remove(*idx);
        }

        (to_close, stop_updates)
    };

    // 1. Perform stop updates on IG
    for (position, new_sl) in stop_updates {
        if position.is_virtual {
            continue; // Virtual positions already updated in state
        }
        if let Err(e) = order_manager.update_stop_loss(client, &position, new_sl).await {
            error!("Failed to update trailing stop on IG for {}: {}", position.deal_id, e);
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
                s.record_trade_result(close_pnl);
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
                });

                // Update scorecard for learning
                if let Some(scorecard) = &mut s.learning.scorecard {
                    scorecard.update_virtual(&position, close_pnl);
                }
            }

            let _ = event_tx.send(EngineEvent::position_closed(
                position.deal_id.clone(),
                close_pnl,
            ));

            // Send Telegram notification for virtual position close
            let tg = telegram.clone();
            let v_name = get_instrument_name(&position.epic);
            let v_epic = position.epic.clone();
            let v_direction = format!("{}", position.direction);
            let v_pnl = close_pnl;
            let v_reason = reason.to_string();
            tokio::spawn(async move {
                let msg = format!(
                    "{} <b>VIRTUAL POSITION CLOSED</b>\n\n<b>Instrument:</b> {} ({})\n<b>Direction:</b> {}\n<b>Reason:</b> {}\n<b>P&amp;L:</b> {:.2}",
                    if v_pnl >= 0.0 { "✅" } else { "❌" },
                    v_name, v_epic, v_direction, v_reason, v_pnl
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
                };

                {
                    let mut s = state.write().await;
                    s.record_trade_result(close_result.pnl);
                    s.add_closed_trade(closed_trade.clone());
                }

                {
                    let mut s = state.write().await;
                    if let Some(ref mut scorecard) = s.learning.scorecard {
                        scorecard.update(&closed_trade);
                    }
                    let new_weights = {
                        let state_ref = &mut *s;
                        if let (Some(sc), Some(wm)) = (&state_ref.learning.scorecard, &mut state_ref.learning.weight_manager) {
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
                        match (&state_ref.learning.scorecard, &state_ref.learning.weight_manager) {
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
                let epic = position.epic.clone();
                let direction = format!("{}", position.direction);
                let pnl_val = close_result.pnl;
                let reason_str = reason.to_string();
                tokio::spawn(async move {
                    let msg = format!(
                        "{} <b>POSITION CLOSED</b>\n\n<b>Instrument:</b> {} ({})\n<b>Direction:</b> {}\n<b>Reason:</b> {}\n<b>P&amp;L:</b> {:.2}",
                        if pnl_val >= 0.0 { "✅" } else { "❌" },
                        name, epic, direction, reason_str, pnl_val
                    );
                    let _ = tg.send_message(&msg).await;
                });
            }
            Err(e) => {
                error!(
                    "Failed to close position {}: {}",
                    position.deal_id, e
                );
            }
        }
    }

    Ok(())
}
