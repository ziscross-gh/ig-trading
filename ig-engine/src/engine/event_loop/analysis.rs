use std::sync::Arc;
use tokio::sync::{RwLock, broadcast};
use anyhow::Result;
use tracing::{info, warn, error, debug};
use chrono::Utc;

use crate::engine::config::{EngineConfig, EngineMode};
use crate::engine::state::{EngineState, Direction, Position};
use crate::api::rest_client::IGRestClient;
use crate::risk::RiskManager;
use crate::strategy::traits::Strategy;
use crate::strategy::ensemble::EnsembleVoter;
use crate::ipc::events::EngineEvent;
use crate::notifications::telegram::TelegramNotifier;

/// Analyze one or more markets and potentially execute trades
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
        let (bid, offer, mid_price) = {
            let s = state.read().await;
            if let Some(ms) = s.markets.live.get(epic) {
                (ms.bid, ms.ask, (ms.bid + ms.ask) / 2.0)
            } else {
                debug!("No market data yet for {} (waiting for Lightstreamer tick)", epic);
                continue;
            }
        };

        if bid <= 0.0 || offer <= 0.0 {
            continue;
        }

        let snapshot = {
            let s = state.read().await;
            if let Some(indicator_set) = s.markets.indicators.get(epic) {
                indicator_set.snapshot()
            } else {
                None
            }
        };

        if let Some(snapshot) = snapshot {
            let _ = event_tx.send(EngineEvent::indicator_update(epic.clone(), snapshot.clone()));

            let mut signals = Vec::new();
            for strategy in strategies {
                if let Some(signal) = strategy.evaluate(epic, mid_price, &snapshot) {
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
