use std::collections::HashMap;
use uuid::Uuid;
use chrono::Utc;

use crate::engine::state::{Direction, Signal};

/// Ensemble Voter combines multiple strategy signals using weighted consensus voting
#[derive(Clone, Debug)]
pub struct EnsembleVoter {
    pub min_consensus: usize,
    pub min_avg_strength: f64,
    pub strategy_weights: HashMap<String, f64>,
}

impl EnsembleVoter {
    pub fn new(min_consensus: usize, min_avg_strength: f64) -> Self {
        Self {
            min_consensus,
            min_avg_strength,
            strategy_weights: HashMap::new(),
        }
    }

    /// Add or update a strategy weight
    pub fn set_strategy_weight(&mut self, strategy_name: String, weight: f64) {
        self.strategy_weights.insert(strategy_name, weight);
    }

    /// Bulk-update strategy weights from the adaptive weight manager.
    pub fn update_weights(&mut self, weights: HashMap<String, f64>) {
        for (name, weight) in weights {
            self.strategy_weights.insert(name, weight);
        }
    }

    /// Votes on a collection of signals and returns a combined signal if consensus is reached
    pub fn vote(&self, signals: &[Signal]) -> Option<Signal> {
        if signals.is_empty() {
            return None;
        }

        // Group signals by direction
        let mut buy_signals = Vec::new();
        let mut sell_signals = Vec::new();

        for signal in signals {
            match signal.direction {
                Direction::Buy => buy_signals.push(signal),
                Direction::Sell => sell_signals.push(signal),
            }
        }

        // Determine dominant direction — capture counts before moving
        let buy_count = buy_signals.len();
        let sell_count = sell_signals.len();
        let (dominant_signals, direction) = if buy_count >= sell_count {
            (buy_signals, Direction::Buy)
        } else {
            (sell_signals, Direction::Sell)
        };

        // Check if we have minimum consensus
        if dominant_signals.len() < self.min_consensus {
            return None;
        }

        // Calculate weighted average strength
        let mut total_weight = 0.0;
        let mut weighted_strength = 0.0;

        for signal in &dominant_signals {
            let weight = self
                .strategy_weights
                .get(&signal.strategy)
                .copied()
                .unwrap_or(1.0);
            total_weight += weight;
            weighted_strength += signal.strength * weight;
        }

        let avg_strength = if total_weight > 0.0 {
            weighted_strength / total_weight
        } else {
            0.0
        };

        // Check if average strength meets minimum threshold
        if avg_strength < self.min_avg_strength {
            return None;
        }

        // === Conflict Detection Penalty ===
        // If the minority direction has signals, reduce confidence proportionally.
        let minority_count = if buy_count >= sell_count { sell_count } else { buy_count };

        let final_strength = if minority_count > 0 {
            let conflict_ratio = minority_count as f64 / dominant_signals.len() as f64;
            let penalty = 1.0 - (conflict_ratio * 0.35).min(0.5); // Cap penalty at 50%
            let penalised = avg_strength * penalty;
            tracing::debug!(
                "Ensemble conflict detected: {} dissenting signal(s), ratio={:.2}, strength {:.2} -> {:.2}",
                minority_count, conflict_ratio, avg_strength, penalised
            );
            penalised
        } else {
            avg_strength
        };

        // Re-check threshold after penalty
        if final_strength < self.min_avg_strength {
            tracing::debug!("Signal rejected after conflict penalty: {:.2} < {:.2}", final_strength, self.min_avg_strength);
            return None;
        }

        // Select best stop loss (tightest for the direction)
        let stop_loss = match direction {
            Direction::Buy => {
                // For buys, we want the highest stop loss (less restrictive)
                dominant_signals
                    .iter()
                    .map(|s| s.stop_loss)
                    .fold(f64::NEG_INFINITY, f64::max)
            }
            Direction::Sell => {
                // For sells, we want the lowest stop loss (less restrictive)
                dominant_signals
                    .iter()
                    .map(|s| s.stop_loss)
                    .fold(f64::INFINITY, f64::min)
            }
        };

        // Select most conservative take profit
        let take_profit = match direction {
            Direction::Buy => {
                // For buys, take the lowest TP (most conservative)
                dominant_signals
                    .iter()
                    .map(|s| s.take_profit)
                    .fold(f64::INFINITY, f64::min)
            }
            Direction::Sell => {
                // For sells, take the highest TP (most conservative)
                dominant_signals
                    .iter()
                    .map(|s| s.take_profit)
                    .fold(f64::NEG_INFINITY, f64::max)
            }
        };

        // Use the first signal's price and epic as reference
        let reference_signal = dominant_signals[0];

        // Build a reason string showing all contributing strategies
        let strategy_list = dominant_signals
            .iter()
            .map(|s| s.strategy.as_str())
            .collect::<Vec<_>>()
            .join(", ");

        let reason = format!(
            "Ensemble Consensus ({} signals from: {}), Avg Strength: {:.2}",
            dominant_signals.len(),
            strategy_list,
            avg_strength
        );

        Some(Signal {
            id: Uuid::new_v4().to_string(),
            epic: reference_signal.epic.clone(),
            direction,
            strength: final_strength,
            strategy: "Ensemble".to_string(),
            reason,
            price: reference_signal.price,
            stop_loss,
            take_profit,
            timestamp: Utc::now(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_signal(
        epic: &str,
        direction: Direction,
        strength: f64,
        strategy: &str,
        price: f64,
        stop_loss: f64,
        take_profit: f64,
    ) -> Signal {
        Signal {
            id: Uuid::new_v4().to_string(),
            epic: epic.to_string(),
            direction,
            strength,
            strategy: strategy.to_string(),
            reason: "Test signal".to_string(),
            price,
            stop_loss,
            take_profit,
            timestamp: Utc::now(),
        }
    }

    #[test]
    fn test_ensemble_voter_creation() {
        let voter = EnsembleVoter::new(2, 6.0);
        assert_eq!(voter.min_consensus, 2);
        assert_eq!(voter.min_avg_strength, 6.0);
    }

    #[test]
    fn test_empty_signals() {
        let voter = EnsembleVoter::new(2, 6.0);
        let signals = vec![];
        assert!(voter.vote(&signals).is_none());
    }

    #[test]
    fn test_insufficient_consensus() {
        let voter = EnsembleVoter::new(3, 6.0);
        let signals = vec![
            create_test_signal("EUR/USD", Direction::Buy, 7.0, "MA_Crossover", 1.1000, 1.0950, 1.1100),
        ];
        assert!(voter.vote(&signals).is_none());
    }

    #[test]
    fn test_weak_average_strength() {
        let voter = EnsembleVoter::new(2, 7.0);
        let signals = vec![
            create_test_signal("EUR/USD", Direction::Buy, 5.0, "MA_Crossover", 1.1000, 1.0950, 1.1100),
            create_test_signal("EUR/USD", Direction::Buy, 6.0, "RSI_Reversal", 1.1000, 1.0950, 1.1100),
        ];
        // Average strength is 5.5, below 7.0 threshold
        assert!(voter.vote(&signals).is_none());
    }

    #[test]
    fn test_successful_consensus_buy() {
        let mut voter = EnsembleVoter::new(2, 6.0);
        voter.set_strategy_weight("MA_Crossover".to_string(), 1.0);
        voter.set_strategy_weight("RSI_Reversal".to_string(), 1.0);

        let signals = vec![
            create_test_signal("EUR/USD", Direction::Buy, 7.0, "MA_Crossover", 1.1000, 1.0950, 1.1100),
            create_test_signal("EUR/USD", Direction::Buy, 7.0, "RSI_Reversal", 1.1000, 1.0950, 1.1100),
        ];

        let result = voter.vote(&signals);
        assert!(result.is_some());

        let combined = result.unwrap();
        assert_eq!(combined.epic, "EUR/USD");
        assert_eq!(combined.direction, Direction::Buy);
        assert_eq!(combined.strength, 7.0);
        assert_eq!(combined.strategy, "Ensemble");
    }

    #[test]
    fn test_weighted_average_strength() {
        let mut voter = EnsembleVoter::new(2, 6.0);
        voter.set_strategy_weight("MA_Crossover".to_string(), 2.0); // Double weight
        voter.set_strategy_weight("RSI_Reversal".to_string(), 1.0);

        let signals = vec![
            create_test_signal("EUR/USD", Direction::Buy, 6.0, "MA_Crossover", 1.1000, 1.0950, 1.1100),
            create_test_signal("EUR/USD", Direction::Buy, 9.0, "RSI_Reversal", 1.1000, 1.0950, 1.1100),
        ];

        let result = voter.vote(&signals);
        assert!(result.is_some());

        let combined = result.unwrap();
        // Weighted average: (6.0 * 2.0 + 9.0 * 1.0) / (2.0 + 1.0) = 21/3 = 7.0
        assert_eq!(combined.strength, 7.0);
    }

    #[test]
    fn test_best_stop_loss_buy() {
        let voter = EnsembleVoter::new(2, 6.0);

        let signals = vec![
            create_test_signal("EUR/USD", Direction::Buy, 7.0, "MA_Crossover", 1.1000, 1.0900, 1.1100),
            create_test_signal("EUR/USD", Direction::Buy, 7.0, "RSI_Reversal", 1.1000, 1.0950, 1.1100),
        ];

        let result = voter.vote(&signals);
        assert!(result.is_some());

        let combined = result.unwrap();
        // For buys, we want the highest stop loss (1.0950 > 1.0900)
        assert_eq!(combined.stop_loss, 1.0950);
    }

    #[test]
    fn test_most_conservative_take_profit_buy() {
        let voter = EnsembleVoter::new(2, 6.0);

        let signals = vec![
            create_test_signal("EUR/USD", Direction::Buy, 7.0, "MA_Crossover", 1.1000, 1.0950, 1.1150),
            create_test_signal("EUR/USD", Direction::Buy, 7.0, "RSI_Reversal", 1.1000, 1.0950, 1.1100),
        ];

        let result = voter.vote(&signals);
        assert!(result.is_some());

        let combined = result.unwrap();
        // For buys, we want the lowest take profit (most conservative)
        assert_eq!(combined.take_profit, 1.1100);
    }

    #[test]
    fn test_dominant_direction_sell() {
        // 2 sell signals, 1 dissenting buy signal.
        // Conflict ratio = 1/2 = 0.5, penalty = 0.825.
        // Strength 9.0 * 0.825 = 7.425 — still above min_avg_strength=6.0.
        let voter = EnsembleVoter::new(1, 6.0);

        let signals = vec![
            create_test_signal("EUR/USD", Direction::Sell, 9.0, "MA_Crossover", 1.1000, 1.1050, 1.0900),
            create_test_signal("EUR/USD", Direction::Sell, 9.0, "RSI_Reversal", 1.1000, 1.1050, 1.0900),
            create_test_signal("EUR/USD", Direction::Buy, 9.0, "MACD_Momentum", 1.1000, 1.0950, 1.1100),
        ];

        let result = voter.vote(&signals);
        assert!(result.is_some(), "Expected signal despite conflict (strength still above threshold after penalty)");

        let combined = result.unwrap();
        assert_eq!(combined.direction, Direction::Sell);
        // Verify conflict penalty was applied (penalised strength < raw strength)
        assert!(combined.strength < 9.0, "Conflict penalty should reduce strength below 9.0");
    }

    #[test]
    fn test_conflict_penalty_blocks_weak_signals() {
        // 2 sell signals, 1 buy — but strength is low enough that penalty blocks it.
        let voter = EnsembleVoter::new(1, 6.0);

        let signals = vec![
            create_test_signal("EUR/USD", Direction::Sell, 7.0, "MA_Crossover", 1.1000, 1.1050, 1.0900),
            create_test_signal("EUR/USD", Direction::Sell, 7.0, "RSI_Reversal", 1.1000, 1.1050, 1.0900),
            create_test_signal("EUR/USD", Direction::Buy,  7.0, "MACD_Momentum", 1.1000, 1.0950, 1.1100),
        ];

        // conflict_ratio=0.5, penalty=0.825 → 7.0*0.825=5.775 < 6.0 → blocked
        let result = voter.vote(&signals);
        assert!(result.is_none(), "Conflicted weak signal should be filtered out by penalty");
    }
}
