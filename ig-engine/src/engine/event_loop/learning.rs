use std::collections::HashMap;
use crate::engine::state::{LearningSnapshot, StrategyLearningEntry, SessionStats, Session};
use crate::learning::scorecard::{StrategyScorecard};
use crate::learning::adaptive_weights::AdaptiveWeightManager;

/// Build a serialisable `LearningSnapshot` from the current scorecard and weight manager.
pub fn build_learning_snapshot(
    scorecard: &StrategyScorecard,
    weight_manager: &AdaptiveWeightManager,
) -> LearningSnapshot {
    let multipliers = weight_manager.get_multipliers();
    let effective = weight_manager.get_effective_weights();
    let sessions = [Session::Asia, Session::London, Session::UsOverlap];

    let strategies: Vec<StrategyLearningEntry> = scorecard
        .strategies()
        .into_iter()
        .filter_map(|name| {
            let perf = scorecard.get_performance(&name)?;
            let current_multiplier = *multipliers.get(&name).unwrap_or(&1.0);
            let effective_weight = *effective.get(&name).unwrap_or(&1.0);

            // Per-session breakdown
            let session_map: HashMap<String, SessionStats> = sessions
                .iter()
                .filter_map(|&s| {
                    let sp = scorecard.get_session_performance(&name, s)?;
                    let pf = if sp.profit_factor.is_finite() { sp.profit_factor } else { 99.0 };
                    Some((
                        s.label().to_string(),
                        SessionStats {
                            win_rate: sp.win_rate,
                            profit_factor: pf,
                        },
                    ))
                })
                .collect();

            Some(StrategyLearningEntry {
                name,
                win_rate: perf.win_rate,
                profit_factor: if perf.profit_factor.is_finite() { perf.profit_factor } else { 99.0 },
                current_multiplier,
                effective_weight,
                max_consecutive_losses: perf.max_consecutive_losses,
                trades_in_window: perf.total_trades,
                sessions: session_map,
            })
        })
        .collect();

    // Keep only the last 20 adjustments for the dashboard
    let adjustments: Vec<_> = weight_manager
        .adjustment_log
        .iter()
        .rev()
        .take(20)
        .cloned()
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();

    LearningSnapshot {
        total_trades_processed: scorecard.total_trades_processed,
        strategies,
        recent_adjustments: adjustments,
    }
}
