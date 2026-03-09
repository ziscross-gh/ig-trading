use serde::{Deserialize, Serialize};
use crate::indicators::{Candle, IndicatorSet};
use crate::strategy::traits::Strategy;
use crate::engine::state::Direction;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestTrade {
    pub epic: String,
    pub direction: Direction,
    pub entry_price: f64,
    pub exit_price: Option<f64>,
    pub entry_time: i64,
    pub exit_time: Option<i64>,
    pub size: f64,
    pub pnl: Option<f64>,
    pub strategy: String,
    pub stop_loss: f64,
    pub take_profit: f64,
    pub trailing_stop_distance: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestResult {
    pub total_trades: usize,
    pub winning_trades: usize,
    pub losing_trades: usize,
    pub win_rate: f64,
    pub total_pnl: f64,
    pub total_pnl_pct: f64,
    pub max_drawdown_pct: f64,
    pub profit_factor: f64,
    pub sharpe_ratio: f64,
    pub trades: Vec<BacktestTrade>,
    pub candle_count: usize,
}

pub struct BacktestEngine {
    initial_balance: f64,
    current_balance: f64,
    risk_per_trade_pct: f64,
    default_stop_loss_pct: f64,
    default_take_profit_pct: f64,
}

impl BacktestEngine {
    pub fn new(initial_balance: f64, risk_pct: f64) -> Self {
        Self {
            initial_balance,
            current_balance: initial_balance,
            risk_per_trade_pct: risk_pct,
            default_stop_loss_pct: 1.5,
            default_take_profit_pct: 3.0,
        }
    }

    pub fn run(
        &mut self,
        epic: &str,
        candles: &[Candle],
        strategy: &dyn Strategy,
    ) -> BacktestResult {
        let mut trades = Vec::new();
        let mut active_trade: Option<BacktestTrade> = None;
        let mut indicators = IndicatorSet::default_config();
        
        let mut peak = self.initial_balance;
        let mut max_drawdown = 0.0;

        for candle in candles {
            indicators.update(candle);
            
            if !indicators.is_warmed_up() {
                continue;
            }

            let current_price = candle.close;

            // 1. Check if we need to close or update an active trade
            if let Some(mut trade) = active_trade.take() {
                // Update trailing stop if active
                if let Some(dist) = trade.trailing_stop_distance {
                    let new_sl = match trade.direction {
                        Direction::Buy => current_price - dist,
                        Direction::Sell => current_price + dist,
                    };
                    
                    let should_update = match trade.direction {
                        Direction::Buy => new_sl > trade.stop_loss,
                        Direction::Sell => new_sl < trade.stop_loss,
                    };
                    
                    if should_update {
                        trade.stop_loss = new_sl;
                    }
                }

                // Check Exit Conditions (SL / TP)
                let exit_reason = if trade.direction == Direction::Buy {
                    if candle.low <= trade.stop_loss { Some("SL") }
                    else if candle.high >= trade.take_profit { Some("TP") }
                    else { None }
                } else if candle.high >= trade.stop_loss { Some("SL") }
                else if candle.low <= trade.take_profit { Some("TP") }
                else { None };

                if let Some(_reason) = exit_reason {
                    // Use candle high/low for exit price if hit during the bar for higher fidelity
                    let exit_price = if _reason == "SL" { trade.stop_loss } else { trade.take_profit };
                    
                    trade.exit_price = Some(exit_price);
                    trade.exit_time = Some(candle.timestamp);
                    
                    let pnl_pct = if trade.direction == Direction::Buy {
                        (exit_price - trade.entry_price) / trade.entry_price
                    } else {
                        (trade.entry_price - exit_price) / trade.entry_price
                    };
                    
                    let trade_pnl = trade.size * pnl_pct * trade.entry_price;
                    trade.pnl = Some(trade_pnl);
                    
                    self.current_balance += trade_pnl;
                    trades.push(trade);

                    // Update peak and drawdown
                    if self.current_balance > peak {
                        peak = self.current_balance;
                    }
                    let dd = (peak - self.current_balance) / peak * 100.0;
                    if dd > max_drawdown {
                        max_drawdown = dd;
                    }
                } else {
                    // Still active
                    active_trade = Some(trade);
                }
            }

            // 2. Check for new signals if no active trade
            if active_trade.is_none() {
                if let Some(snapshot) = indicators.snapshot() {
                    let mut snaps = std::collections::HashMap::new();
                    snaps.insert("HOUR".to_string(), snapshot);
                    
                    if let Some(signal) = strategy.evaluate(epic, current_price, &snaps) {
                        // Use signal's stops if provided, else defaults
                        let stop_loss = if signal.stop_loss > 0.0 { signal.stop_loss } else {
                            match signal.direction {
                                Direction::Buy => current_price * (1.0 - self.default_stop_loss_pct / 100.0),
                                Direction::Sell => current_price * (1.0 + self.default_stop_loss_pct / 100.0),
                            }
                        };
                        
                        let take_profit = if signal.take_profit > 0.0 { signal.take_profit } else {
                            match signal.direction {
                                Direction::Buy => current_price * (1.0 + self.default_take_profit_pct / 100.0),
                                Direction::Sell => current_price * (1.0 - self.default_take_profit_pct / 100.0),
                            }
                        };

                        let stop_dist = (current_price - stop_loss).abs();
                        let risk_amount = self.current_balance * (self.risk_per_trade_pct / 100.0);
                        let size = (risk_amount / stop_dist).max(1.0);

                        active_trade = Some(BacktestTrade {
                            epic: epic.to_string(),
                            direction: signal.direction,
                            entry_price: current_price,
                            exit_price: None,
                            entry_time: candle.timestamp,
                            exit_time: None,
                            size,
                            pnl: None,
                            strategy: strategy.name().to_string(),
                            stop_loss,
                            take_profit,
                            trailing_stop_distance: signal.trailing_stop_distance,
                        });
                    }
                }
            }
        }

        // Final stats calculation...
        let total_trades = trades.len();
        let winning_trades = trades.iter().filter(|t| t.pnl.unwrap_or(0.0) > 0.0).count();
        let losing_trades = total_trades - winning_trades;
        let win_rate = if total_trades > 0 {
            (winning_trades as f64 / total_trades as f64) * 100.0
        } else {
            0.0
        };

        let total_pnl = self.current_balance - self.initial_balance;
        let total_pnl_pct = (total_pnl / self.initial_balance) * 100.0;

        let total_gain: f64 = trades.iter()
            .map(|t| t.pnl.unwrap_or(0.0))
            .filter(|p| *p > 0.0)
            .sum();
        let total_loss: f64 = trades.iter()
            .map(|t| t.pnl.unwrap_or(0.0))
            .filter(|p| *p < 0.0)
            .sum::<f64>()
            .abs();
        
        let profit_factor = if total_loss > 0.0 {
            total_gain / total_loss
        } else if total_gain > 0.0 {
            10.0 
        } else {
            0.0
        };

        BacktestResult {
            total_trades,
            winning_trades,
            losing_trades,
            win_rate,
            total_pnl,
            total_pnl_pct,
            max_drawdown_pct: max_drawdown,
            profit_factor,
            sharpe_ratio: 0.0, // Simplified for now
            trades,
            candle_count: candles.len(),
        }
    }
}
