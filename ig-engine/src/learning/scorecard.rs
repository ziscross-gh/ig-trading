//! Strategy Scorecard — tracks rolling performance metrics per strategy/market/session.
//!
//! Fed by ClosedTrade records. Provides win rates, profit factors, and session
//! breakdowns that the AdaptiveWeightManager uses to adjust ensemble weights.

use std::collections::HashMap;
use chrono::Timelike;
use serde::{Deserialize, Serialize};
use tracing::debug;

use crate::engine::state::{ClosedTrade, Session};


/// Performance snapshot for one strategy (optionally filtered by market/session).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyPerf {
    pub strategy: String,
    pub total_trades: usize,
    pub wins: usize,
    pub losses: usize,
    pub win_rate: f64,           // 0.0–1.0
    pub avg_pnl: f64,
    pub profit_factor: f64,      // gross_wins / gross_losses (∞ if no losses)
    pub avg_hold_secs: f64,
    pub max_consecutive_losses: u32,
}

/// A single trade record enriched with session info, for internal use.
#[derive(Debug, Clone)]
struct TradeRecord {
    #[allow(dead_code)]
    strategy: String,
    epic: String,
    session: Session,
    pnl: f64,
    hold_secs: i64,
    is_win: bool,
}

/// Rolling scorecard tracking the last N trades per strategy.
pub struct StrategyScorecard {
    /// Maximum trades to keep per strategy (rolling window).
    window_size: usize,
    /// strategy_name -> Vec<TradeRecord> (newest last)
    records: HashMap<String, Vec<TradeRecord>>,
    /// Total trades processed (lifetime, not rolling)
    pub total_trades_processed: u64,
}

impl StrategyScorecard {
    pub fn new(window_size: usize) -> Self {
        Self {
            window_size,
            records: HashMap::new(),
            total_trades_processed: 0,
        }
    }

    /// Record a closed trade into the scorecard.
    pub fn update(&mut self, trade: &ClosedTrade) {
        let session = Session::from_utc_hour(trade.opened_at.hour());
        let hold_secs = (trade.closed_at - trade.opened_at).num_seconds();

        let record = TradeRecord {
            strategy: trade.strategy.clone(),
            epic: trade.epic.clone(),
            session,
            pnl: trade.pnl,
            hold_secs,
            is_win: trade.pnl > 0.0,
        };

        self.push_record(trade.strategy.clone(), record);
    }

    /// Record a virtual trade closure.
    pub fn update_virtual(&mut self, position: &crate::engine::state::Position, pnl: f64) {
        let session = Session::from_utc_hour(position.opened_at.hour());
        let hold_secs = (chrono::Utc::now() - position.opened_at).num_seconds();

        let record = TradeRecord {
            strategy: position.strategy.clone(),
            epic: position.epic.clone(),
            session,
            pnl,
            hold_secs,
            is_win: pnl > 0.0,
        };

        self.push_record(position.strategy.clone(), record);
    }

    fn push_record(&mut self, strategy: String, record: TradeRecord) {
        let entries = self.records.entry(strategy.clone()).or_default();
        entries.push(record);

        if entries.len() > self.window_size {
            let excess = entries.len() - self.window_size;
            entries.drain(..excess);
        }

        self.total_trades_processed += 1;
        debug!(
            "Scorecard updated ({}): {} — {} trades in window, lifetime={}",
            if self.total_trades_processed % 1 == 0 { "trade" } else { "virtual" },
            strategy,
            entries.len(),
            self.total_trades_processed
        );
    }

    /// Get overall performance for a strategy across all markets and sessions.
    pub fn get_performance(&self, strategy: &str) -> Option<StrategyPerf> {
        let records = self.records.get(strategy)?;
        if records.is_empty() {
            return None;
        }
        Some(Self::compute_perf(strategy, records))
    }

    /// Get performance for a strategy in a specific session.
    pub fn get_session_performance(&self, strategy: &str, session: Session) -> Option<StrategyPerf> {
        let records = self.records.get(strategy)?;
        let filtered: Vec<_> = records.iter().filter(|r| r.session == session).cloned().collect();
        if filtered.is_empty() {
            return None;
        }
        Some(Self::compute_perf(strategy, &filtered))
    }

    /// Get performance for a strategy on a specific market.
    pub fn get_market_performance(&self, strategy: &str, epic: &str) -> Option<StrategyPerf> {
        let records = self.records.get(strategy)?;
        let filtered: Vec<_> = records.iter().filter(|r| r.epic == epic).cloned().collect();
        if filtered.is_empty() {
            return None;
        }
        Some(Self::compute_perf(strategy, &filtered))
    }

    /// List all strategies that have recorded trades.
    pub fn strategies(&self) -> Vec<String> {
        self.records.keys().cloned().collect()
    }

    /// Get trade count for a strategy.
    pub fn trade_count(&self, strategy: &str) -> usize {
        self.records.get(strategy).map_or(0, |r| r.len())
    }

    /// Compute performance metrics from a slice of trade records.
    fn compute_perf(strategy: &str, records: &[TradeRecord]) -> StrategyPerf {
        let total = records.len();
        let wins = records.iter().filter(|r| r.is_win).count();
        let losses = total - wins;
        let win_rate = if total > 0 { wins as f64 / total as f64 } else { 0.0 };

        let total_pnl: f64 = records.iter().map(|r| r.pnl).sum();
        let avg_pnl = if total > 0 { total_pnl / total as f64 } else { 0.0 };

        let gross_wins: f64 = records.iter().filter(|r| r.pnl > 0.0).map(|r| r.pnl).sum();
        let gross_losses: f64 = records.iter().filter(|r| r.pnl < 0.0).map(|r| r.pnl.abs()).sum();
        let profit_factor = if gross_losses > 0.0 { gross_wins / gross_losses } else { f64::INFINITY };

        let avg_hold_secs = if total > 0 {
            records.iter().map(|r| r.hold_secs as f64).sum::<f64>() / total as f64
        } else {
            0.0
        };

        // Max consecutive losses
        let mut max_consec = 0u32;
        let mut current_consec = 0u32;
        for r in records {
            if !r.is_win {
                current_consec += 1;
                max_consec = max_consec.max(current_consec);
            } else {
                current_consec = 0;
            }
        }

        StrategyPerf {
            strategy: strategy.to_string(),
            total_trades: total,
            wins,
            losses,
            win_rate,
            avg_pnl,
            profit_factor,
            avg_hold_secs,
            max_consecutive_losses: max_consec,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use crate::engine::state::{ClosedTrade, Direction};

    fn make_trade(strategy: &str, epic: &str, pnl: f64, hour: u32) -> ClosedTrade {
        let opened = Utc::now()
            .date_naive()
            .and_hms_opt(hour, 0, 0)
            .unwrap()
            .and_utc();
        ClosedTrade {
            deal_id: "test".into(),
            epic: epic.into(),
            direction: Direction::Buy,
            size: 1.0,
            entry_price: 100.0,
            exit_price: if pnl > 0.0 { 101.0 } else { 99.0 },
            stop_loss: 99.0,
            take_profit: Some(102.0),
            pnl,
            strategy: strategy.into(),
            status: "closed".into(),
            opened_at: opened,
            closed_at: opened + chrono::Duration::minutes(30),
            is_virtual: false,
        }
    }

    #[test]
    fn test_scorecard_basic_metrics() {
        let mut sc = StrategyScorecard::new(20);

        // 7 wins, 3 losses = 70% win rate
        for i in 0..10 {
            let pnl = if i < 7 { 50.0 } else { -30.0 };
            sc.update(&make_trade("MA_Crossover", "EURUSD", pnl, 10));
        }

        let perf = sc.get_performance("MA_Crossover").unwrap();
        assert_eq!(perf.total_trades, 10);
        assert_eq!(perf.wins, 7);
        assert!((perf.win_rate - 0.7).abs() < 0.01);
        assert!(perf.profit_factor > 1.0);
    }

    #[test]
    fn test_scorecard_rolling_window() {
        let mut sc = StrategyScorecard::new(5);

        // Add 8 trades — window should keep only last 5
        for _ in 0..8 {
            sc.update(&make_trade("RSI", "GBPUSD", 10.0, 3));
        }

        assert_eq!(sc.trade_count("RSI"), 5);
        assert_eq!(sc.total_trades_processed, 8);
    }

    #[test]
    fn test_session_detection() {
        assert_eq!(Session::from_utc_hour(3), Session::Asia);
        assert_eq!(Session::from_utc_hour(10), Session::London);
        assert_eq!(Session::from_utc_hour(14), Session::UsOverlap);
    }

    #[test]
    fn test_session_performance() {
        let mut sc = StrategyScorecard::new(50);

        // Asia trades: all winners
        for _ in 0..5 {
            sc.update(&make_trade("MACD", "EURUSD", 20.0, 3));
        }
        // London trades: all losers
        for _ in 0..5 {
            sc.update(&make_trade("MACD", "EURUSD", -15.0, 10));
        }

        let asia = sc.get_session_performance("MACD", Session::Asia).unwrap();
        assert!((asia.win_rate - 1.0).abs() < 0.01);

        let london = sc.get_session_performance("MACD", Session::London).unwrap();
        assert!((london.win_rate - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_max_consecutive_losses() {
        let mut sc = StrategyScorecard::new(20);
        let pnls = [10.0, -5.0, -5.0, -5.0, 10.0, -5.0];
        for pnl in pnls {
            sc.update(&make_trade("BB", "USDJPY", pnl, 12));
        }
        let perf = sc.get_performance("BB").unwrap();
        assert_eq!(perf.max_consecutive_losses, 3);
    }
}
