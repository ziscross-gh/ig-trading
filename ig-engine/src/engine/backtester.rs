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
}

pub struct BacktestEngine {
    initial_balance: f64,
    current_balance: f64,
    risk_per_trade_pct: f64,
    stop_loss_pct: f64,
    take_profit_pct: f64,
}

impl BacktestEngine {
    pub fn new(initial_balance: f64, risk_pct: f64) -> Self {
        Self {
            initial_balance,
            current_balance: initial_balance,
            risk_per_trade_pct: risk_pct,
            stop_loss_pct: 1.5,
            take_profit_pct: 3.0,
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
        
        // Track stats for drawdown
        let mut equity_curve = vec![self.initial_balance];
        let mut peak = self.initial_balance;
        let mut max_drawdown = 0.0;

        for candle in candles {
            indicators.update(candle);
            
            if !indicators.is_warmed_up() {
                continue;
            }

            // 1. Check if we need to close an active trade
            if let Some(mut trade) = active_trade.take() {
                let mut closed = false;
                
                // Check Stop Loss / Take Profit
                let pnl_pct = if trade.direction == Direction::Buy {
                    (candle.close - trade.entry_price) / trade.entry_price * 100.0
                } else {
                    (trade.entry_price - candle.close) / trade.entry_price * 100.0
                };

                if pnl_pct <= -self.stop_loss_pct || pnl_pct >= self.take_profit_pct {
                    trade.exit_price = Some(candle.close);
                    trade.exit_time = Some(candle.timestamp);
                    let trade_pnl = trade.size * (pnl_pct / 100.0) * trade.entry_price;
                    trade.pnl = Some(trade_pnl);
                    
                    self.current_balance += trade_pnl;
                    trades.push(trade);
                    closed = true;
                } else {
                    // Still active
                    active_trade = Some(trade);
                }

                if closed {
                    // Update peak and drawdown
                    if self.current_balance > peak {
                        peak = self.current_balance;
                    }
                    let dd = (peak - self.current_balance) / peak * 100.0;
                    if dd > max_drawdown {
                        max_drawdown = dd;
                    }
                    equity_curve.push(self.current_balance);
                }
            }

            // 2. Check for new signals if no active trade
            if active_trade.is_none() {
                if let Some(snapshot) = indicators.snapshot() {
                    let mut snaps = std::collections::HashMap::new();
                    snaps.insert("HOUR".to_string(), snapshot);
                    if let Some(signal) = strategy.evaluate(epic, candle.close, &snaps) {
                        // Calculate size based on risk
                        let risk_amount = self.current_balance * (self.risk_per_trade_pct / 100.0);
                        let stop_loss_dist = candle.close * (self.stop_loss_pct / 100.0);
                        let size = (risk_amount / stop_loss_dist).round().max(1.0);

                        active_trade = Some(BacktestTrade {
                            epic: epic.to_string(),
                            direction: signal.direction,
                            entry_price: candle.close,
                            exit_price: None,
                            entry_time: candle.timestamp,
                            exit_time: None,
                            size,
                            pnl: None,
                            strategy: strategy.name().to_string(),
                        });
                    }
                }
            }
        }

        // Close any remaining open trade at last price
        if let Some(mut trade) = active_trade {
            if let Some(last_candle) = candles.last() {
                trade.exit_price = Some(last_candle.close);
                trade.exit_time = Some(last_candle.timestamp);
                let pnl_pct = if trade.direction == Direction::Buy {
                    (last_candle.close - trade.entry_price) / trade.entry_price * 100.0
                } else {
                    (trade.entry_price - last_candle.close) / trade.entry_price * 100.0
                };
                let trade_pnl = trade.size * (pnl_pct / 100.0) * trade.entry_price;
                trade.pnl = Some(trade_pnl);
                self.current_balance += trade_pnl;
                trades.push(trade);
            }
        }

        // Calculate final stats
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
            10.0 // Arbitrary high value
        } else {
            0.0
        };

        // Calculate Sharpe Ratio (simplified, per trade)
        let sharpe_ratio = if total_trades > 1 {
            let returns: Vec<f64> = trades.iter()
                .map(|t| t.pnl.unwrap_or(0.0) / self.initial_balance)
                .collect();
            
            let mean = returns.iter().sum::<f64>() / returns.len() as f64;
            let variance = returns.iter()
                .map(|r| (r - mean).powi(2))
                .sum::<f64>() / (returns.len() - 1) as f64;
            let std_dev = variance.sqrt();
            
            if std_dev > 0.0 {
                (mean / std_dev) * (total_trades as f64).sqrt()
            } else {
                0.0
            }
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
            sharpe_ratio,
            trades,
        }
    }
}
