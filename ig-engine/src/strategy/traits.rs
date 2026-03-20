use crate::engine::state::Signal;
use crate::indicators::IndicatorSnapshot;

/// Core strategy trait that all trading strategies must implement
pub trait Strategy: Send + Sync {
    /// Returns the name of the strategy
    fn name(&self) -> &str;

    /// Evaluates the strategy based on current price and indicators
    /// Returns Some(Signal) if a trade signal is generated, None otherwise
    fn evaluate(
        &self,
        epic: &str,
        price: f64,
        indicators: &std::collections::HashMap<String, IndicatorSnapshot>,
    ) -> Option<Signal>;

    #[allow(dead_code)]
    fn warmup_period(&self) -> usize;
}

/// M15 strategy trait — dual-timeframe signals using M15 as primary, H1 as directional filter.
pub trait M15Strategy: Send + Sync {
    fn name(&self) -> &str;
    fn weight(&self) -> f64;
    /// Evaluate M15 signal using both M15 and H1 indicator snapshots.
    /// `regime` is the current ML regime string: "TRENDING", "RANGING", "VOLATILE", or "".
    fn evaluate_m15(
        &self,
        epic: &str,
        price: f64,
        m15_snapshot: &IndicatorSnapshot,
        h1_snapshot: &IndicatorSnapshot,
        regime: &str,
    ) -> Option<crate::engine::state::Signal>;
    fn warmup_period(&self) -> usize;
}
