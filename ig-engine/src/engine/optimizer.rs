use serde::{Deserialize, Serialize};
use crate::indicators::Candle;
use crate::strategy::ma_crossover::MACrossoverStrategy;
use super::backtester::{BacktestEngine, BacktestResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationResult {
    pub best_pnl: f64,
    pub best_parameters: String,
    pub top_runs: Vec<OptimizationRun>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationRun {
    pub parameters: String,
    pub result: BacktestResult,
}

pub struct Optimizer {
    candles: Vec<Candle>,
    initial_balance: f64,
}

impl Optimizer {
    pub fn new(candles: Vec<Candle>, initial_balance: f64) -> Self {
        Self {
            candles,
            initial_balance,
        }
    }

    /// Optimizes the MA Crossover strategy by sweeping short/long periods and ADX threshold
    pub async fn optimize_ma_crossover(
        &self,
        epic: &str,
        short_range: std::ops::Range<usize>,
        long_range: std::ops::Range<usize>,
        adx_range: Vec<f64>,
    ) -> OptimizationResult {
        let mut runs = Vec::new();

        for short in short_range.step_by(2) {
            for long in long_range.clone().step_by(5) {
                if short >= long {
                    continue;
                }

                for &adx in &adx_range {
                    let strategy = MACrossoverStrategy::new(
                        short,
                        long,
                        adx,
                        1.0,  // weight
                        2.0,  // ATR SL multiplier
                        3.0,  // ATR TP multiplier
                    );

                    let mut engine = BacktestEngine::new(self.initial_balance, 1.0);
                    let result = engine.run(epic, &self.candles, &strategy);

                    runs.push(OptimizationRun {
                        parameters: format!("Short: {}, Long: {}, ADX: {}", short, long, adx),
                        result,
                    });
                }
            }
        }

        // Sort by PnL descending
        runs.sort_by(|a, b| b.result.total_pnl.partial_cmp(&a.result.total_pnl).unwrap());

        // Keep top 10
        let top_runs = runs.iter().take(10).cloned().collect::<Vec<_>>();

        let best = &top_runs[0];

        OptimizationResult {
            best_pnl: best.result.total_pnl,
            best_parameters: best.parameters.clone(),
            top_runs,
        }
    }
}
