use uuid::Uuid;
use chrono::Utc;

use crate::indicators::IndicatorSnapshot;
use crate::engine::state::{Direction, Signal};
use super::traits::Strategy;

/// MACD Momentum Strategy
/// Generates BUY/SELL signals based on MACD histogram zero-line crossovers with momentum confirmation
#[derive(Clone, Debug)]
pub struct MACDMomentumStrategy {
    #[allow(dead_code)]
    pub weight: f64,            // reserved for ensemble weight override; ensemble manages weights by name
    pub atr_sl_multiplier: f64,
    pub atr_tp_multiplier: f64,
}

impl MACDMomentumStrategy {
    pub fn new(weight: f64, atr_sl_multiplier: f64, atr_tp_multiplier: f64) -> Self {
        Self {
            weight,
            atr_sl_multiplier,
            atr_tp_multiplier,
        }
    }
}

impl Default for MACDMomentumStrategy {
    fn default() -> Self {
        Self::new(1.0, 2.0, 3.0)
    }
}

impl MACDMomentumStrategy {
    fn calculate_signal_strength(&self, indicators: &IndicatorSnapshot, is_buy: bool) -> f64 {
        let mut strength: f64 = 7.0; // Base strength for MACD cross

        // ADX trend confirmation
        if let Some(adx) = indicators.adx {
            if adx > 35.0 {
                strength += 2.0;
            } else if adx > 25.0 {
                strength += 1.0;
            }
        }

        // MA alignment confirmation
        if let (Some(ema_short), Some(ema_long)) = (indicators.ema_short, indicators.ema_long) {
            if is_buy && ema_short > ema_long {
                strength += 1.0;
            } else if !is_buy && ema_short < ema_long {
                strength += 1.0;
            }
        }

        strength.min(10.0)
    }

    fn histogram_is_expanding(&self, indicators: &IndicatorSnapshot) -> bool {
        if let (Some(histogram), Some(prev_histogram)) =
            (indicators.macd_histogram, indicators.prev_macd_histogram)
        {
            histogram.abs() > prev_histogram.abs()
        } else {
            false
        }
    }

    fn calculate_stops_and_targets(
        &self,
        direction: Direction,
        price: f64,
        indicators: &IndicatorSnapshot,
    ) -> (f64, f64) {
        let atr = indicators.atr.unwrap_or(price * 0.02);

        match direction {
            Direction::Buy => {
                let stop_loss = price - (self.atr_sl_multiplier * atr);
                let take_profit = price + (self.atr_tp_multiplier * atr);
                (stop_loss, take_profit)
            }
            Direction::Sell => {
                let stop_loss = price + (self.atr_sl_multiplier * atr);
                let take_profit = price - (self.atr_tp_multiplier * atr);
                (stop_loss, take_profit)
            }
        }
    }
}

impl Strategy for MACDMomentumStrategy {
    fn name(&self) -> &str {
        "MACD_Momentum"
    }

    fn evaluate(&self, epic: &str, price: f64, indicators_map: &std::collections::HashMap<String, IndicatorSnapshot>) -> Option<Signal> {
        // Fallback to "HOUR" timeframe for single-TF backward compatibility
        let indicators = indicators_map.get("HOUR")?;

        let histogram = indicators.macd_histogram?;
        let prev_histogram = indicators.prev_macd_histogram?;
        let adx = indicators.adx?;

        // Require ADX > 20 for trend confirmation
        if adx < 20.0 {
            return None;
        }

        // BUY Signal: Histogram crosses above zero AND histogram is expanding
        if prev_histogram < 0.0 && histogram > 0.0 && self.histogram_is_expanding(indicators) {
            let strength = self.calculate_signal_strength(indicators, true);
            let (stop_loss, take_profit) = self.calculate_stops_and_targets(Direction::Buy, price, indicators);

            let reason = format!(
                "MACD Momentum BUY: Histogram crossing zero (prev={:.6}, curr={:.6}), ADX={:.2}",
                prev_histogram, histogram, adx
            );

            return Some(Signal {
                id: Uuid::new_v4().to_string(),
                epic: epic.to_string(),
                direction: Direction::Buy,
                strength,
                strategy: self.name().to_string(),
                reason,
                price,
                stop_loss,
                take_profit,
                timestamp: Utc::now(),
            });
        }

        // SELL Signal: Histogram crosses below zero AND histogram is contracting
        if prev_histogram > 0.0 && histogram < 0.0 && self.histogram_is_expanding(indicators) {
            let strength = self.calculate_signal_strength(indicators, false);
            let (stop_loss, take_profit) = self.calculate_stops_and_targets(Direction::Sell, price, indicators);

            let reason = format!(
                "MACD Momentum SELL: Histogram crossing zero (prev={:.6}, curr={:.6}), ADX={:.2}",
                prev_histogram, histogram, adx
            );

            return Some(Signal {
                id: Uuid::new_v4().to_string(),
                epic: epic.to_string(),
                direction: Direction::Sell,
                strength,
                strategy: self.name().to_string(),
                reason,
                price,
                stop_loss,
                take_profit,
                timestamp: Utc::now(),
            });
        }

        None
    }

    fn warmup_period(&self) -> usize {
        50 // MACD needs reasonable history but less than MA periods
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_macd_momentum_creation() {
        let strategy = MACDMomentumStrategy::new(1.0, 2.0, 3.0);
        assert_eq!(strategy.name(), "MACD_Momentum");
    }

    #[test]
    fn test_warmup_period() {
        let strategy = MACDMomentumStrategy::new(1.0, 2.0, 3.0);
        assert_eq!(strategy.warmup_period(), 50);
    }
}
