//! Adaptive Weight Manager — dynamically adjusts strategy weights based on scorecard performance.
//!
//! Every N closed trades, recalculates weights using rolling win rate and profit factor.
//! Uses EMA smoothing to prevent whip-sawing and clamps weights to safe bounds.

use std::collections::HashMap;
use tracing::{info, debug};
use serde::{Deserialize, Serialize};

use super::scorecard::StrategyScorecard;

/// Configuration for the adaptive weight manager.
#[derive(Debug, Clone)]
pub struct AdaptiveConfig {
    /// Minimum trades before adjusting a strategy's weight.
    pub min_trades_for_adjustment: usize,
    /// Recalculate weights every N trades.
    pub recalc_interval: u64,
    /// EMA smoothing factor (0.0–1.0). Higher = more responsive.
    pub ema_alpha: f64,
    /// Minimum weight multiplier (floor).
    pub min_weight_multiplier: f64,
    /// Maximum weight multiplier (ceiling).
    pub max_weight_multiplier: f64,
}

impl Default for AdaptiveConfig {
    fn default() -> Self {
        Self {
            min_trades_for_adjustment: 20,
            recalc_interval: 10,
            ema_alpha: 0.3,
            min_weight_multiplier: 0.3,
            max_weight_multiplier: 2.0,
        }
    }
}

/// A weight adjustment event for logging/dashboard display.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeightAdjustment {
    pub strategy: String,
    pub old_weight: f64,
    pub new_weight: f64,
    pub win_rate: f64,
    pub profit_factor: f64,
    pub trade_count: usize,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Manages dynamic strategy weights based on live performance.
pub struct AdaptiveWeightManager {
    config: AdaptiveConfig,
    /// Base weights from config (never mutated).
    base_weights: HashMap<String, f64>,
    /// Current smoothed weight multipliers.
    current_multipliers: HashMap<String, f64>,
    /// Last N weight adjustment events (for dashboard).
    pub adjustment_log: Vec<WeightAdjustment>,
    /// Trades processed at last recalc.
    last_recalc_at: u64,
}

impl AdaptiveWeightManager {
    pub fn new(base_weights: HashMap<String, f64>, config: AdaptiveConfig) -> Self {
        let multipliers: HashMap<String, f64> = base_weights.keys()
            .map(|k| (k.clone(), 1.0))
            .collect();

        Self {
            config,
            base_weights,
            current_multipliers: multipliers,
            adjustment_log: Vec::new(),
            last_recalc_at: 0,
        }
    }

    /// Check if it's time to recalculate and do so if needed.
    /// Returns Some(new_weights) if weights were updated, None otherwise.
    pub fn maybe_recalculate(
        &mut self,
        scorecard: &StrategyScorecard,
    ) -> Option<HashMap<String, f64>> {
        let total = scorecard.total_trades_processed;

        // Only recalculate every N trades
        if total < self.last_recalc_at + self.config.recalc_interval {
            return None;
        }

        self.last_recalc_at = total;
        let mut changed = false;

        for strategy in scorecard.strategies() {
            let trade_count = scorecard.trade_count(&strategy);

            // Don't adjust until we have enough data
            if trade_count < self.config.min_trades_for_adjustment {
                debug!(
                    "Adaptive: {} has only {} trades (need {}), keeping base weight",
                    strategy, trade_count, self.config.min_trades_for_adjustment
                );
                continue;
            }

            if let Some(perf) = scorecard.get_performance(&strategy) {
                let raw_multiplier = Self::compute_multiplier(perf.win_rate, perf.profit_factor);

                // EMA smooth the multiplier
                let prev = self.current_multipliers.get(&strategy).copied().unwrap_or(1.0);
                let smoothed = prev * (1.0 - self.config.ema_alpha) + raw_multiplier * self.config.ema_alpha;

                // Clamp to safe bounds
                let clamped = smoothed
                    .max(self.config.min_weight_multiplier)
                    .min(self.config.max_weight_multiplier);

                let old_weight = self.base_weights.get(&strategy).copied().unwrap_or(1.0) * prev;
                let new_weight = self.base_weights.get(&strategy).copied().unwrap_or(1.0) * clamped;

                if (prev - clamped).abs() > 0.01 {
                    changed = true;
                    info!(
                        "🧠 Adaptive weight: {} {:.2} → {:.2} (win_rate={:.0}%, pf={:.2}, trades={})",
                        strategy, old_weight, new_weight,
                        perf.win_rate * 100.0, perf.profit_factor, trade_count
                    );

                    self.adjustment_log.push(WeightAdjustment {
                        strategy: strategy.clone(),
                        old_weight,
                        new_weight,
                        win_rate: perf.win_rate,
                        profit_factor: perf.profit_factor,
                        trade_count,
                        timestamp: chrono::Utc::now(),
                    });

                    // Keep log to last 100 entries
                    if self.adjustment_log.len() > 100 {
                        self.adjustment_log.drain(..self.adjustment_log.len() - 100);
                    }
                }

                self.current_multipliers.insert(strategy, clamped);
            }
        }

        if changed {
            Some(self.get_effective_weights())
        } else {
            None
        }
    }

    /// Get current effective weights (base × multiplier).
    pub fn get_effective_weights(&self) -> HashMap<String, f64> {
        self.base_weights
            .iter()
            .map(|(k, base)| {
                let mult = self.current_multipliers.get(k).copied().unwrap_or(1.0);
                (k.clone(), base * mult)
            })
            .collect()
    }

    /// Get current multipliers for dashboard display.
    pub fn get_multipliers(&self) -> &HashMap<String, f64> {
        &self.current_multipliers
    }

    /// Compute raw multiplier from win rate and profit factor.
    ///
    /// Win rate contribution (70% of signal):
    ///   - `>= 60%` → boost (1.0–1.3)
    ///   - `40–60%` → neutral (0.8–1.0)
    ///   - `< 40%`  → penalise (0.5–0.8)
    ///
    /// Profit factor contribution (30% of signal):
    ///   - `>= 1.5` → boost
    ///   - `0.8–1.5` → neutral
    ///   - `< 0.8`  → penalise
    fn compute_multiplier(win_rate: f64, profit_factor: f64) -> f64 {
        let wr_score = if win_rate >= 0.6 {
            1.0 + (win_rate - 0.6) * 0.75  // 0.6→1.0, 0.8→1.15, 1.0→1.3
        } else if win_rate >= 0.4 {
            0.8 + (win_rate - 0.4) * 1.0    // 0.4→0.8, 0.5→0.9, 0.6→1.0
        } else {
            0.5 + win_rate * 0.75           // 0.0→0.5, 0.2→0.65, 0.4→0.8
        };

        let pf_score = if profit_factor >= 1.5 {
            1.0 + ((profit_factor - 1.5) * 0.1).min(0.3) // Cap bonus at +0.3
        } else if profit_factor >= 0.8 {
            0.85 + (profit_factor - 0.8) * 0.21  // 0.8→0.85, 1.5→1.0
        } else {
            0.5 + profit_factor * 0.44            // 0.0→0.5, 0.8→0.85
        };

        // Weighted blend: 70% win rate, 30% profit factor
        wr_score * 0.7 + pf_score * 0.3
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::learning::scorecard::StrategyScorecard;
    use crate::engine::state::{ClosedTrade, Direction};

    fn make_trade(strategy: &str, pnl: f64) -> ClosedTrade {
        let now = chrono::Utc::now();
        ClosedTrade {
            deal_id: "t".into(),
            epic: "EURUSD".into(),
            direction: Direction::Buy,
            size: 1.0,
            entry_price: 100.0,
            exit_price: 100.0 + pnl,
            stop_loss: 99.0,
            take_profit: Some(102.0),
            pnl,
            strategy: strategy.into(),
            status: "closed".into(),
            opened_at: now,
            closed_at: now + chrono::Duration::minutes(30),
            is_virtual: false,
        }
    }

    #[test]
    fn test_no_adjustment_below_min_trades() {
        let mut sc = StrategyScorecard::new(50);
        let base_weights: HashMap<String, f64> = [("MA".into(), 1.0)].into();
        let mut mgr = AdaptiveWeightManager::new(base_weights, AdaptiveConfig {
            min_trades_for_adjustment: 20,
            recalc_interval: 5,
            ..Default::default()
        });

        // Only 10 trades — should not adjust
        for _ in 0..10 {
            sc.update(&make_trade("MA", 10.0));
        }

        let result = mgr.maybe_recalculate(&sc);
        // Either None (no change) or Some with weight still ~1.0
        if let Some(weights) = result {
            let w = weights.get("MA").expect("MA weight should exist");
            assert!((*w - 1.0).abs() < 0.01, "Should not adjust with only 10 trades");
        }
    }

    #[test]
    fn test_winning_strategy_gets_boosted() {
        let mut sc = StrategyScorecard::new(50);
        let base_weights: HashMap<String, f64> = [("MA".into(), 1.0)].into();
        let mut mgr = AdaptiveWeightManager::new(base_weights, AdaptiveConfig {
            min_trades_for_adjustment: 10,
            recalc_interval: 5,
            ..Default::default()
        });

        // 80% win rate — strong performer
        for i in 0..25 {
            let pnl = if i % 5 < 4 { 20.0 } else { -10.0 };
            sc.update(&make_trade("MA", pnl));
        }

        let result = mgr.maybe_recalculate(&sc);
        assert!(result.is_some(), "Should have recalculated");
        let w = result.expect("Recalculation failed").get("MA").expect("MA weight should exist").clone();
        assert!(w > 1.0, "Winning strategy should have weight > 1.0, got {}", w);
    }

    #[test]
    fn test_losing_strategy_gets_penalised() {
        let mut sc = StrategyScorecard::new(50);
        let base_weights: HashMap<String, f64> = [("BAD".into(), 1.0)].into();
        let mut mgr = AdaptiveWeightManager::new(base_weights, AdaptiveConfig {
            min_trades_for_adjustment: 10,
            recalc_interval: 5,
            ..Default::default()
        });

        // 20% win rate — poor performer
        for i in 0..25 {
            let pnl = if i % 5 == 0 { 10.0 } else { -15.0 };
            sc.update(&make_trade("BAD", pnl));
        }

        let result = mgr.maybe_recalculate(&sc);
        assert!(result.is_some());
        let w = result.expect("Recalculation failed").get("BAD").expect("BAD weight should exist").clone();
        assert!(w < 1.0, "Losing strategy should have weight < 1.0, got {}", w);
    }

    #[test]
    fn test_multiplier_clamping() {
        // Extreme win rate shouldn't exceed max multiplier
        let mult = AdaptiveWeightManager::compute_multiplier(1.0, 5.0);
        assert!(mult <= 2.0 + 0.01, "Multiplier should be clamped, got {}", mult);

        // Extreme loss rate shouldn't go below min
        let mult = AdaptiveWeightManager::compute_multiplier(0.0, 0.0);
        assert!(mult >= 0.3 - 0.01, "Multiplier floor, got {}", mult);
    }
}
