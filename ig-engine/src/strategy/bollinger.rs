use uuid::Uuid;
use chrono::Utc;

use crate::indicators::IndicatorSnapshot;
use crate::engine::state::{Direction, Signal};
use super::traits::Strategy;

/// Bollinger Band Mean Reversion Strategy
/// Generates BUY/SELL signals when price extremes are confirmed by other indicators
#[derive(Clone, Debug)]
pub struct BollingerStrategy {
    pub period: usize,
    #[allow(dead_code)]
    pub std_dev: f64,          // stored for config round-trip; band calc is done by IndicatorSet
    #[allow(dead_code)]
    pub weight: f64,            // reserved for ensemble weight override; ensemble manages weights by name
    pub atr_sl_multiplier: f64,
    #[allow(dead_code)]
    pub atr_tp_multiplier: f64, // reserved for future ATR-based TP; currently uses middle-band TP
}

impl BollingerStrategy {
    pub fn new(period: usize, std_dev: f64, weight: f64, atr_sl_multiplier: f64, atr_tp_multiplier: f64) -> Self {
        Self {
            period,
            std_dev,
            weight,
            atr_sl_multiplier,
            atr_tp_multiplier,
        }
    }
}

impl Default for BollingerStrategy {
    fn default() -> Self {
        Self::new(20, 2.0, 1.0, 1.5, 3.0)
    }
}

impl BollingerStrategy {
    fn calculate_signal_strength(&self, indicators: &IndicatorSnapshot, is_buy: bool) -> f64 {
        let mut strength: f64 = 7.0; // Base strength for Bollinger touch

        // RSI confirmation
        if let Some(rsi) = indicators.rsi {
            if is_buy && rsi < 35.0 {
                strength += 1.0;
            } else if !is_buy && rsi > 65.0 {
                strength += 1.0;
            }
        }

        // Stochastic confirmation
        if let Some(stoch_k) = indicators.stochastic_k {
            if is_buy && stoch_k < 20.0 {
                strength += 1.0;
            } else if !is_buy && stoch_k > 80.0 {
                strength += 1.0;
            }
        }

        strength.min(10.0)
    }

    fn is_squeeze_detected(&self, indicators: &IndicatorSnapshot) -> bool {
        if let Some(bandwidth) = indicators.bollinger_bandwidth {
            bandwidth < 0.01
        } else {
            false
        }
    }

    fn calculate_stops_and_targets(
        &self,
        direction: Direction,
        price: f64,
        indicators: &IndicatorSnapshot,
        middle_band: f64,
    ) -> (f64, f64) {
        let atr = indicators.atr.unwrap_or(price * 0.02);

        match direction {
            Direction::Buy => {
                let stop_loss = price - (self.atr_sl_multiplier * atr);
                // Target is middle band for mean reversion
                let take_profit = middle_band;
                (stop_loss, take_profit)
            }
            Direction::Sell => {
                let stop_loss = price + (self.atr_sl_multiplier * atr);
                let take_profit = middle_band;
                (stop_loss, take_profit)
            }
        }
    }
}

impl Strategy for BollingerStrategy {
    fn name(&self) -> &str {
        "Bollinger_Bands"
    }

    fn evaluate(&self, epic: &str, price: f64, indicators: &IndicatorSnapshot) -> Option<Signal> {
        let percent_b = indicators.bollinger_percent_b?;
        let rsi = indicators.rsi?;
        let middle_band = indicators.bollinger_middle?;

        // BUY Signal: Price near lower band AND RSI is not overbought
        if percent_b < 0.05 && rsi < 40.0 {
            let mut strength = self.calculate_signal_strength(indicators, true);

            // Squeeze bonus: if bandwidth is very low, expect breakout
            if self.is_squeeze_detected(indicators) {
                strength += 0.5;
            }

            let (stop_loss, take_profit) = self.calculate_stops_and_targets(
                Direction::Buy,
                price,
                indicators,
                middle_band,
            );

            let reason = format!(
                "Bollinger Mean Reversion BUY: percent_b={:.4} (near lower), RSI={:.2}",
                percent_b, rsi
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

        // SELL Signal: Price near upper band AND RSI is not oversold
        if percent_b > 0.95 && rsi > 60.0 {
            let mut strength = self.calculate_signal_strength(indicators, false);

            // Squeeze bonus: if bandwidth is very low, expect breakout
            if self.is_squeeze_detected(indicators) {
                strength += 0.5;
            }

            let (stop_loss, take_profit) = self.calculate_stops_and_targets(
                Direction::Sell,
                price,
                indicators,
                middle_band,
            );

            let reason = format!(
                "Bollinger Mean Reversion SELL: percent_b={:.4} (near upper), RSI={:.2}",
                percent_b, rsi
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
        self.period + 50
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bollinger_creation() {
        let strategy = BollingerStrategy::new(20, 2.0, 1.0, 1.5, 2.0);
        assert_eq!(strategy.name(), "Bollinger_Bands");
        assert_eq!(strategy.period, 20);
        assert_eq!(strategy.std_dev, 2.0);
    }

    #[test]
    fn test_warmup_period() {
        let strategy = BollingerStrategy::new(20, 2.0, 1.0, 1.5, 2.0);
        assert_eq!(strategy.warmup_period(), 70); // 20 + 50
    }

    #[test]
    fn test_squeeze_detection() {
        let strategy = BollingerStrategy::new(20, 2.0, 1.0, 1.5, 2.0);

        let mut indicators = IndicatorSnapshot {
            bollinger_bandwidth: Some(0.005),
            ..Default::default()
        };

        assert!(strategy.is_squeeze_detected(&indicators));

        indicators.bollinger_bandwidth = Some(0.02);
        assert!(!strategy.is_squeeze_detected(&indicators));
    }
}
