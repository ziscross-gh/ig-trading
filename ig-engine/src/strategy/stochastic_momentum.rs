use uuid::Uuid;
use chrono::Utc;

use crate::indicators::IndicatorSnapshot;
use crate::engine::state::{Direction, Signal};
use super::traits::Strategy;

/// Stochastic Momentum Strategy
///
/// Designed for VOLATILE and RANGING regimes where oscillator crossovers carry
/// more weight than trend-following indicators.
///
/// Entry conditions (1H bar close):
/// - **SELL**: `%K` crosses below `%D` AND both are above 50 (bearish cross from overbought territory)
/// - **BUY**:  `%K` crosses above `%D` AND both are below 50 (bullish cross from oversold territory)
///
/// Strength bonuses (applied on top of base 7.0):
/// - ADX > 40: +1.5 (momentum is confirmed by directional strength)
/// - ADX > 25: +0.5 (moderate trend confirms direction)
/// - RSI confirms direction (RSI > 50 for buy, RSI < 50 for sell): +0.5
/// - Stochastic extremes (K > 70 for sell, K < 30 for buy): +1.0
#[derive(Clone, Debug)]
pub struct StochasticMomentumStrategy {
    #[allow(dead_code)]
    pub weight: f64,
    pub overbought: f64,
    pub oversold: f64,
    pub atr_sl_multiplier: f64,
    pub atr_tp_multiplier: f64,
    pub trailing_stop_pips: Option<f64>,
}

impl StochasticMomentumStrategy {
    pub fn new(
        weight: f64,
        overbought: f64,
        oversold: f64,
        atr_sl_multiplier: f64,
        atr_tp_multiplier: f64,
        trailing_stop_pips: Option<f64>,
    ) -> Self {
        Self {
            weight,
            overbought,
            oversold,
            atr_sl_multiplier,
            atr_tp_multiplier,
            trailing_stop_pips,
        }
    }
}

impl Default for StochasticMomentumStrategy {
    fn default() -> Self {
        Self::new(1.0, 70.0, 30.0, 1.5, 3.0, Some(15.0))
    }
}

impl StochasticMomentumStrategy {
    fn calculate_signal_strength(&self, indicators: &IndicatorSnapshot, is_buy: bool) -> f64 {
        let mut strength: f64 = 7.0;

        // ADX confirmation — strong directional momentum
        if let Some(adx) = indicators.adx {
            if adx > 40.0 {
                strength += 1.5;
            } else if adx > 25.0 {
                strength += 0.5;
            }
        }

        // RSI directional confirmation
        if let Some(rsi) = indicators.rsi {
            if (is_buy && rsi < 50.0) || (!is_buy && rsi > 50.0) {
                strength += 0.5;
            }
        }

        // Stochastic extremeness bonus
        if let Some(k) = indicators.stochastic_k {
            if (is_buy && k < self.oversold) || (!is_buy && k > self.overbought) {
                strength += 1.0;
            }
        }

        strength.min(10.0)
    }

    fn calculate_stops(
        &self,
        direction: Direction,
        price: f64,
        indicators: &IndicatorSnapshot,
    ) -> (f64, f64, Option<f64>) {
        let atr = indicators.atr.unwrap_or(price * 0.01);

        let (stop_loss, take_profit) = match direction {
            Direction::Buy => (
                price - self.atr_sl_multiplier * atr,
                price + self.atr_tp_multiplier * atr,
            ),
            Direction::Sell => (
                price + self.atr_sl_multiplier * atr,
                price - self.atr_tp_multiplier * atr,
            ),
        };

        let trailing_stop_distance = self.trailing_stop_pips.map(|pips| {
            let pip_scale = if price > 50.0 { 0.01 } else { 0.0001 };
            pips * pip_scale
        });

        (stop_loss, take_profit, trailing_stop_distance)
    }
}

impl Strategy for StochasticMomentumStrategy {
    fn name(&self) -> &str {
        "Stochastic_Momentum"
    }

    fn evaluate(
        &self,
        epic: &str,
        price: f64,
        indicators_map: &std::collections::HashMap<String, IndicatorSnapshot>,
    ) -> Option<Signal> {
        let indicators = indicators_map.get("HOUR")?;

        let k = indicators.stochastic_k?;
        let d = indicators.stochastic_d?;
        let prev_k = indicators.prev_stochastic_k?;
        let prev_d = indicators.prev_stochastic_d?;

        // Bearish crossover: K crossed below D, both above midline (overbought region)
        let bearish_cross = prev_k >= prev_d && k < d && k > 50.0 && d > 50.0;
        // Bullish crossover: K crossed above D, both below midline (oversold region)
        let bullish_cross = prev_k <= prev_d && k > d && k < 50.0 && d < 50.0;

        if !bearish_cross && !bullish_cross {
            return None;
        }

        let (direction, is_buy) = if bullish_cross {
            (Direction::Buy, true)
        } else {
            (Direction::Sell, false)
        };

        let strength = self.calculate_signal_strength(indicators, is_buy);
        let (stop_loss, take_profit, trailing_stop_distance) =
            self.calculate_stops(direction.clone(), price, indicators);

        let reason = format!(
            "Stochastic {} cross: K={:.1} D={:.1} (prev K={:.1} D={:.1}), ADX={:.1}",
            if is_buy { "bullish" } else { "bearish" },
            k,
            d,
            prev_k,
            prev_d,
            indicators.adx.unwrap_or(0.0),
        );

        Some(Signal {
            id: Uuid::new_v4().to_string(),
            epic: epic.to_string(),
            direction,
            strength,
            strategy: "Stochastic_Momentum".to_string(),
            reason,
            price,
            stop_loss,
            take_profit,
            trailing_stop_distance,
            timestamp: Utc::now(),
        })
    }

    fn warmup_period(&self) -> usize {
        // stoch_period(14) + D smoothing(3) + buffer; warm after 50 bars
        50
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn make_snapshot(k: f64, d: f64, prev_k: f64, prev_d: f64, adx: f64, rsi: f64, atr: f64) -> IndicatorSnapshot {
        IndicatorSnapshot {
            stochastic_k: Some(k),
            stochastic_d: Some(d),
            prev_stochastic_k: Some(prev_k),
            prev_stochastic_d: Some(prev_d),
            adx: Some(adx),
            rsi: Some(rsi),
            atr: Some(atr),
            ..Default::default()
        }
    }

    #[test]
    fn test_name() {
        let s = StochasticMomentumStrategy::default();
        assert_eq!(s.name(), "Stochastic_Momentum");
    }

    #[test]
    fn test_bullish_cross_fires() {
        let s = StochasticMomentumStrategy::default();
        let snap = make_snapshot(32.0, 28.0, 20.0, 22.0, 35.0, 42.0, 15.0);
        let mut map = HashMap::new();
        map.insert("HOUR".to_string(), snap);
        let sig = s.evaluate("TEST", 100.0, &map);
        assert!(sig.is_some());
        let sig = sig.unwrap();
        assert_eq!(sig.direction, Direction::Buy);
    }

    #[test]
    fn test_bearish_cross_fires() {
        let s = StochasticMomentumStrategy::default();
        let snap = make_snapshot(68.0, 72.0, 75.0, 70.0, 40.0, 62.0, 15.0);
        let mut map = HashMap::new();
        map.insert("HOUR".to_string(), snap);
        let sig = s.evaluate("TEST", 100.0, &map);
        assert!(sig.is_some());
        let sig = sig.unwrap();
        assert_eq!(sig.direction, Direction::Sell);
    }

    #[test]
    fn test_no_cross_no_signal() {
        let s = StochasticMomentumStrategy::default();
        // K still above D — no crossover yet
        let snap = make_snapshot(75.0, 70.0, 78.0, 68.0, 30.0, 60.0, 15.0);
        let mut map = HashMap::new();
        map.insert("HOUR".to_string(), snap);
        assert!(s.evaluate("TEST", 100.0, &map).is_none());
    }

    #[test]
    fn test_cross_wrong_zone_no_signal() {
        let s = StochasticMomentumStrategy::default();
        // K crosses below D but both below 50 — not valid bearish cross (need K > 50)
        let snap = make_snapshot(45.0, 48.0, 52.0, 50.0, 30.0, 55.0, 15.0);
        let mut map = HashMap::new();
        map.insert("HOUR".to_string(), snap);
        assert!(s.evaluate("TEST", 100.0, &map).is_none());
    }

    #[test]
    fn test_adx_bonus() {
        let s = StochasticMomentumStrategy::default();
        // Strong ADX (>40) → +1.5 bonus
        let snap = make_snapshot(32.0, 28.0, 20.0, 22.0, 45.0, 42.0, 15.0);
        let mut map = HashMap::new();
        map.insert("HOUR".to_string(), snap);
        let sig = s.evaluate("TEST", 100.0, &map).unwrap();
        // Base 7.0 + ADX>40 1.5 + RSI<50 0.5 + K<30 1.0 = 10.0 (capped)
        assert!(sig.strength >= 9.0);
    }
}
