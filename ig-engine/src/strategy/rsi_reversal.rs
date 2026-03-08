use uuid::Uuid;
use chrono::Utc;

use crate::indicators::IndicatorSnapshot;
use crate::engine::state::{Direction, Signal};
use super::traits::Strategy;

/// RSI Mean Reversion Strategy
/// Generates BUY/SELL signals when RSI reaches extreme levels with confirmation signals
#[derive(Clone, Debug)]
pub struct RSIReversalStrategy {
    pub period: usize,
    pub overbought: f64,
    pub oversold: f64,
    #[allow(dead_code)]
    pub weight: f64,            // reserved for ensemble weight override; ensemble manages weights by name
    pub detect_divergence: bool,
    pub atr_sl_multiplier: f64,
    pub atr_tp_multiplier: f64,
    pub trailing_stop_pips: Option<f64>,
    // State for divergence
    last_rsi_low: Option<f64>,
    last_price_low: Option<f64>,
    last_rsi_high: Option<f64>,
    last_price_high: Option<f64>,
}

impl RSIReversalStrategy {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        period: usize,
        overbought: f64,
        oversold: f64,
        weight: f64,
        detect_divergence: bool,
        atr_sl_multiplier: f64,
        atr_tp_multiplier: f64,
        trailing_stop_pips: Option<f64>,
    ) -> Self {
        Self {
            period,
            overbought,
            oversold,
            weight,
            detect_divergence,
            atr_sl_multiplier,
            atr_tp_multiplier,
            trailing_stop_pips,
            last_rsi_low: None,
            last_price_low: None,
            last_rsi_high: None,
            last_price_high: None,
        }
    }
}

impl Default for RSIReversalStrategy {
    fn default() -> Self {
        Self::new(14, 70.0, 30.0, 1.0, true, 1.5, 3.0, Some(12.0))
    }
}

impl RSIReversalStrategy {
    fn calculate_signal_strength(&self, indicators: &IndicatorSnapshot, is_buy: bool) -> f64 {
        let mut strength: f64 = 6.0;

        // RSI extremeness check
        if let Some(rsi) = indicators.rsi {
            if (is_buy && rsi < 20.0) || (!is_buy && rsi > 80.0) {
                strength += 2.0;
            } else if (is_buy && rsi < 30.0) || (!is_buy && rsi > 70.0) {
                strength += 1.0;
            }
        }

        // Bollinger Band confirmation
        if let Some(percent_b) = indicators.bollinger_percent_b {
            if (is_buy && percent_b < 0.1) || (!is_buy && percent_b > 0.9) {
                strength += 1.0;
            }
        }

        // Stochastic confirmation
        if let Some(stoch_k) = indicators.stochastic_k {
            if (is_buy && stoch_k < 20.0) || (!is_buy && stoch_k > 80.0) {
                strength += 1.0;
            }
        }

        strength.min(10.0)
    }

    fn calculate_stops_and_targets(
        &self,
        direction: Direction,
        price: f64,
        indicators: &IndicatorSnapshot,
    ) -> (f64, f64, Option<f64>) {
        let atr = indicators.atr.unwrap_or(price * 0.02);

        let (stop_loss, take_profit) = match direction {
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
        };

        let trailing_stop_distance = self.trailing_stop_pips.map(|pips| {
            let pip_scale = if price > 50.0 { 0.01 } else { 0.0001 };
            pips * pip_scale
        });

        (stop_loss, take_profit, trailing_stop_distance)
    }

    fn generate_buy_signal(&self, epic: &str, price: f64, indicators: &IndicatorSnapshot) -> Signal {
        let rsi = indicators.rsi.unwrap_or(0.0);
        let strength = self.calculate_signal_strength(indicators, true);
        let (stop_loss, take_profit, trailing_stop_distance) = self.calculate_stops_and_targets(Direction::Buy, price, indicators);

        Signal {
            id: Uuid::new_v4().to_string(),
            epic: epic.to_string(),
            direction: Direction::Buy,
            strength,
            strategy: "RSI_Reversal".to_string(),
            reason: format!("RSI Reversal BUY: RSI={:.2} (oversold), MACD cross positive", rsi),
            price,
            stop_loss,
            take_profit,
            trailing_stop_distance,
            timestamp: Utc::now(),
        }
    }

    fn generate_sell_signal(&self, epic: &str, price: f64, indicators: &IndicatorSnapshot) -> Signal {
        let rsi = indicators.rsi.unwrap_or(0.0);
        let strength = self.calculate_signal_strength(indicators, false);
        let (stop_loss, take_profit, trailing_stop_distance) = self.calculate_stops_and_targets(Direction::Sell, price, indicators);

        Signal {
            id: Uuid::new_v4().to_string(),
            epic: epic.to_string(),
            direction: Direction::Sell,
            strength,
            strategy: "RSI_Reversal".to_string(),
            reason: format!("RSI Reversal SELL: RSI={:.2} (overbought), MACD cross negative", rsi),
            price,
            stop_loss,
            take_profit,
            trailing_stop_distance,
            timestamp: Utc::now(),
        }
    }
}

impl Strategy for RSIReversalStrategy {
    fn name(&self) -> &str {
        "RSI_Reversal"
    }

    fn evaluate(&self, epic: &str, price: f64, indicators_map: &std::collections::HashMap<String, IndicatorSnapshot>) -> Option<Signal> {
        // Fallback to "HOUR" timeframe for single-TF backward compatibility
        let indicators = indicators_map.get("HOUR")?;

        let rsi = indicators.rsi?;
        let macd_histogram = indicators.macd_histogram?;
        let prev_macd_histogram = indicators.prev_macd_histogram?;

        // Update divergence state (simplified peak/trough detection)
        if rsi < 30.0 {
            // Potential Bullish Divergence check (Price Lower Low, RSI Higher Low)
            if let (Some(last_rsi), Some(last_price)) = (self.last_rsi_low, self.last_price_low) {
                if price < last_price && rsi > last_rsi && self.detect_divergence {
                    // Bullish Divergence detected!
                    let mut signal = self.generate_buy_signal(epic, price, indicators);
                    signal.reason = format!("{} [Bullish Divergence]", signal.reason);
                    signal.strength += 2.0;
                    return Some(signal);
                }
            }
        } else if rsi > 70.0 {
            // Potential Bearish Divergence check (Price Higher High, RSI Lower High)
             if let (Some(last_rsi), Some(last_price)) = (self.last_rsi_high, self.last_price_high) {
                if price > last_price && rsi < last_rsi && self.detect_divergence {
                    // Bearish Divergence detected!
                    let mut signal = self.generate_sell_signal(epic, price, indicators);
                    signal.reason = format!("{} [Bearish Divergence]", signal.reason);
                    signal.strength += 2.0;
                    return Some(signal);
                }
            }
        }

        // BUY Signal: RSI is oversold AND MACD histogram is turning positive
        if rsi < self.oversold && prev_macd_histogram < 0.0 && macd_histogram > 0.0 {
            return Some(self.generate_buy_signal(epic, price, indicators));
        }

        // SELL Signal: RSI is overbought AND MACD histogram is turning negative
        if rsi > self.overbought && prev_macd_histogram > 0.0 && macd_histogram < 0.0 {
            return Some(self.generate_sell_signal(epic, price, indicators));
        }

        None
    }

    fn warmup_period(&self) -> usize {
        self.period + 50 // Ensure sufficient data for all indicators
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rsi_reversal_creation() {
        let strategy = RSIReversalStrategy::new(14, 70.0, 30.0, 1.0, true, 2.0, 3.0, Some(12.0));
        assert_eq!(strategy.name(), "RSI_Reversal");
    }

    #[test]
    fn test_warmup_period() {
        let strategy = RSIReversalStrategy::new(14, 70.0, 30.0, 1.0, true, 2.0, 3.0, Some(12.0));
        assert_eq!(strategy.warmup_period(), 64);
    }
}
