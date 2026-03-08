use crate::indicators::IndicatorSnapshot;
use crate::engine::state::Signal;

/// Core strategy trait that all trading strategies must implement
pub trait Strategy: Send + Sync {
    /// Returns the name of the strategy
    fn name(&self) -> &str;

    /// Evaluates the strategy based on current price and indicators
    /// Returns Some(Signal) if a trade signal is generated, None otherwise
    fn evaluate(&self, epic: &str, price: f64, indicators: &std::collections::HashMap<String, IndicatorSnapshot>) -> Option<Signal>;

    #[allow(dead_code)]
    fn warmup_period(&self) -> usize;
}
