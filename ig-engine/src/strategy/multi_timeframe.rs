use uuid::Uuid;
use chrono::Utc;
use std::collections::HashMap;

use crate::indicators::IndicatorSnapshot;
use crate::engine::state::{Direction, Signal};
use crate::strategy::traits::Strategy;

/// Multi-Timeframe Alignment Strategy
/// Provides high-conviction signals when Trend, Signal, and Entry timeframes align
#[derive(Clone, Debug)]
pub struct MultiTimeframeStrategy {
    pub trend_tf: String,
    pub signal_tf: String,
    pub entry_tf: String,
    #[allow(dead_code)]
    pub weight: f64,
    pub atr_sl_multiplier: f64,
    pub atr_tp_multiplier: f64,
}

impl MultiTimeframeStrategy {
    pub fn new(
        trend_tf: String,
        signal_tf: String,
        entry_tf: String,
        weight: f64,
        atr_sl_multiplier: f64,
        atr_tp_multiplier: f64,
    ) -> Self {
        Self {
            trend_tf,
            signal_tf,
            entry_tf,
            weight,
            atr_sl_multiplier,
            atr_tp_multiplier,
        }
    }

    fn calculate_stops_and_targets(
        &self,
        direction: Direction,
        price: f64,
        entry_indicators: &IndicatorSnapshot,
    ) -> (f64, f64) {
        let atr = entry_indicators.atr.unwrap_or(price * 0.02);

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

impl Strategy for MultiTimeframeStrategy {
    fn name(&self) -> &str {
        "Multi_Timeframe"
    }

    fn evaluate(&self, epic: &str, price: f64, indicators_map: &HashMap<String, IndicatorSnapshot>) -> Option<Signal> {
        let trend_ind = indicators_map.get(&self.trend_tf)?;
        let signal_ind = indicators_map.get(&self.signal_tf)?;
        let entry_ind = indicators_map.get(&self.entry_tf)?;

        // 1. Evaluate Trend Timeframe (e.g. 4H)
        let trend_ema_short = trend_ind.ema_short?;
        let trend_ema_long = trend_ind.ema_long?;
        let trend_adx = trend_ind.adx.unwrap_or(0.0);
        
        // We only trade if the trend timeframe is trending strongly
        if trend_adx < 20.0 {
            return None;
        }

        let is_trend_bullish = trend_ema_short > trend_ema_long;
        let is_trend_bearish = trend_ema_short < trend_ema_long;

        // 2. Evaluate Signal Timeframe (e.g. 1H)
        let sig_macd = signal_ind.macd_histogram?;
        let sig_prev_macd = signal_ind.prev_macd_histogram?;
        
        let is_signal_bullish = sig_macd > 0.0 && sig_macd > sig_prev_macd;
        let is_signal_bearish = sig_macd < 0.0 && sig_macd < sig_prev_macd;

        // 3. Evaluate Entry Timeframe (e.g. 15MIN)
        let entry_rsi = entry_ind.rsi?;
        
        // Check alignment
        if is_trend_bullish && is_signal_bullish {
            // Wait for a slight pullback on the entry timeframe before firing
            if entry_rsi < 45.0 {
                let strength = 9.0; // MTF signals are high conviction
                let (stop_loss, take_profit) = self.calculate_stops_and_targets(Direction::Buy, price, entry_ind);

                let reason = format!(
                    "MTF Alignment BUY: {} trend bullish, {} signal bullish, {} entry RSI={:.2}",
                    self.trend_tf, self.signal_tf, self.entry_tf, entry_rsi
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
        }

        if is_trend_bearish && is_signal_bearish {
             // Wait for a slight pullback on the entry timeframe before firing
             if entry_rsi > 55.0 {
                let strength = 9.0; 
                let (stop_loss, take_profit) = self.calculate_stops_and_targets(Direction::Sell, price, entry_ind);

                let reason = format!(
                    "MTF Alignment SELL: {} trend bearish, {} signal bearish, {} entry RSI={:.2}",
                     self.trend_tf, self.signal_tf, self.entry_tf, entry_rsi
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
        }

        None
    }

    fn warmup_period(&self) -> usize {
        // Assume large enough for trend timeframe indicator sets
        100 
    }
}
