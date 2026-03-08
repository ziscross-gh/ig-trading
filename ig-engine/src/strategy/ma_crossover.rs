use uuid::Uuid;
use chrono::Utc;

use crate::indicators::IndicatorSnapshot;
use crate::engine::state::{Direction, Signal};
use super::traits::Strategy;

/// Adaptive Moving Average Crossover Strategy
/// Generates BUY/SELL signals based on EMA crossovers with trend confirmation via ADX
#[derive(Clone, Debug)]
pub struct MACrossoverStrategy {
    #[allow(dead_code)]
    pub short_period: usize,    // stored for config round-trip; EMA calc is done by IndicatorSet
    pub long_period: usize,
    pub adx_threshold: f64,
    #[allow(dead_code)]
    pub weight: f64,            // reserved for ensemble weight override; ensemble manages weights by name
    pub atr_sl_multiplier: f64,
    pub atr_tp_multiplier: f64,
    pub trailing_stop_pips: Option<f64>,
}

impl MACrossoverStrategy {
    pub fn new(
        short_period: usize,
        long_period: usize,
        adx_threshold: f64,
        weight: f64,
        atr_sl_multiplier: f64,
        atr_tp_multiplier: f64,
        trailing_stop_pips: Option<f64>,
    ) -> Self {
        Self {
            short_period,
            long_period,
            adx_threshold,
            weight,
            atr_sl_multiplier,
            atr_tp_multiplier,
            trailing_stop_pips,
        }
    }
}

impl Default for MACrossoverStrategy {
    fn default() -> Self {
        Self::new(12, 26, 25.0, 1.0, 2.0, 3.0, Some(15.0))
    }
}

impl MACrossoverStrategy {
    fn calculate_signal_strength(&self, indicators: &IndicatorSnapshot) -> f64 {
        let mut strength: f64 = 6.0; // Base strength for crossover

        // ADX confirmation (standardizing: 25+ is good, 35+ is strong)
        if let Some(adx) = indicators.adx {
            if adx > 35.0 {
                strength += 2.0;
            } else if adx > 25.0 {
                strength += 1.0;
            }
        }

        // MACD confirmation
        if let (Some(_macd), Some(_prev_macd)) = (indicators.macd, indicators.prev_macd) {
            let current_histogram = indicators.macd_histogram.unwrap_or(0.0);
            let prev_histogram = indicators.prev_macd_histogram.unwrap_or(0.0);

            // Strong MACD confirmation if histogram is increasing and positive
            if current_histogram > prev_histogram && current_histogram > 0.0 {
                strength += 1.0;
            }
        }

        // RSI favorable zone (40-60 is neutral trend, > 60 for buy strength)
        if let Some(rsi) = indicators.rsi {
            if rsi > 60.0 {
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
        let atr = indicators.atr.unwrap_or(price * 0.02); // Default 2% if no ATR

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
}

impl Strategy for MACrossoverStrategy {
    fn name(&self) -> &str {
        "MA_Crossover"
    }

    fn evaluate(&self, epic: &str, price: f64, indicators_map: &std::collections::HashMap<String, IndicatorSnapshot>) -> Option<Signal> {
        // Fallback to "HOUR" timeframe for single-TF backward compatibility
        let indicators = indicators_map.get("HOUR")?;

        // Check if we have the required indicators
        let ema_short = indicators.ema_short?;
        let ema_long = indicators.ema_long?;
        let prev_ema_short = indicators.prev_ema_short?;
        let prev_ema_long = indicators.prev_ema_long?;
        let adx = indicators.adx?;

        // Filter: only trade if ADX indicates a trending market
        if adx < self.adx_threshold {
            return None;
        }

        let ema_200 = indicators.ema_200.unwrap_or(price);

        // BUY Signal: Short EMA crosses above Long EMA
        if ema_short > ema_long && prev_ema_short <= prev_ema_long {
            // Filter: price must be above EMA trend (200-period)
            if price < ema_200 {
                return None;
            }

            let strength = self.calculate_signal_strength(indicators);
            let (stop_loss, take_profit, trailing_stop_distance) = self.calculate_stops_and_targets(Direction::Buy, price, indicators);

            let reason = format!(
                "EMA Crossover BUY: Short({:.2}) > Long({:.2}), ADX={:.2}",
                ema_short, ema_long, adx
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
                trailing_stop_distance,
                timestamp: Utc::now(),
            });
        }

        // SELL Signal: Short EMA crosses below Long EMA
        if ema_short < ema_long && prev_ema_short >= prev_ema_long {
            // Filter: price must be below EMA trend (200-period)
            if price > ema_200 {
                return None;
            }

            let strength = self.calculate_signal_strength(indicators);
            let (stop_loss, take_profit, trailing_stop_distance) = self.calculate_stops_and_targets(Direction::Sell, price, indicators);

            let reason = format!(
                "EMA Crossover SELL: Short({:.2}) < Long({:.2}), ADX={:.2}",
                ema_short, ema_long, adx
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
                trailing_stop_distance,
                timestamp: Utc::now(),
            });
        }

        None
    }

    fn warmup_period(&self) -> usize {
        self.long_period + 50 // Ensure sufficient data for all indicators
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ma_crossover_creation() {
        let strategy = MACrossoverStrategy::new(12, 26, 20.0, 1.0, 2.0, 3.0, Some(15.0));
        assert_eq!(strategy.name(), "MA_Crossover");
        assert_eq!(strategy.short_period, 12);
        assert_eq!(strategy.long_period, 26);
        assert_eq!(strategy.adx_threshold, 20.0);
    }

    #[test]
    fn test_warmup_period() {
        let strategy = MACrossoverStrategy::new(12, 26, 20.0, 1.0, 2.0, 3.0, Some(15.0));
        assert_eq!(strategy.warmup_period(), 76); // 26 + 50
    }
}
