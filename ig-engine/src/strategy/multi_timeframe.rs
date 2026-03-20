use chrono::Utc;
use std::collections::HashMap;
use uuid::Uuid;

use crate::engine::state::{Direction, Signal};
use crate::indicators::IndicatorSnapshot;
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
    pub trailing_stop_pips: Option<f64>,
}

impl MultiTimeframeStrategy {
    pub fn new(
        trend_tf: String,
        signal_tf: String,
        entry_tf: String,
        weight: f64,
        atr_sl_multiplier: f64,
        atr_tp_multiplier: f64,
        trailing_stop_pips: Option<f64>,
    ) -> Self {
        Self {
            trend_tf,
            signal_tf,
            entry_tf,
            weight,
            atr_sl_multiplier,
            atr_tp_multiplier,
            trailing_stop_pips,
        }
    }

    fn calculate_signal_strength(
        &self,
        trend_adx: f64,
        sig_macd: f64,
        sig_prev_macd: f64,
        entry_rsi: f64,
        is_buy: bool,
        using_fallback_tf: bool,
    ) -> f64 {
        // Base: 7.5 — 3-TF alignment is already high conviction
        // (lower than the raw 9.0 so bonuses make a meaningful difference)
        let mut strength = 7.5_f64;

        // Penalty when falling back to HOUR data for trend/entry TF
        if using_fallback_tf {
            strength -= 1.0;
        }

        // Trend ADX strength
        if trend_adx > 40.0 {
            strength += 1.5;
        } else if trend_adx > 30.0 {
            strength += 0.75;
        }

        // MACD momentum: expanding histogram = accelerating move
        let macd_expansion = sig_macd.abs() > sig_prev_macd.abs() * 1.5;
        if macd_expansion {
            strength += 0.5;
        }

        // Entry RSI confirms pullback depth (deeper pullback = better entry)
        if (is_buy && entry_rsi < 35.0) || (!is_buy && entry_rsi > 65.0) {
            strength += 0.5;
        }

        strength.min(10.0)
    }

    fn calculate_stops_and_targets(
        &self,
        direction: Direction,
        price: f64,
        entry_indicators: &IndicatorSnapshot,
    ) -> (f64, f64, Option<f64>) {
        let atr = entry_indicators.atr.unwrap_or(price * 0.02);

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

impl Strategy for MultiTimeframeStrategy {
    fn name(&self) -> &str {
        "Multi_Timeframe"
    }

    fn evaluate(
        &self,
        epic: &str,
        price: f64,
        indicators_map: &HashMap<String, IndicatorSnapshot>,
    ) -> Option<Signal> {
        // Fall back to signal_tf (HOUR) if trend_tf (HOUR_4) or entry_tf (MINUTE_15) not warmed up yet.
        // This lets the strategy participate in ensemble voting using HOUR data on all three levels
        // until multi-resolution historical data is available.
        let trend_ind = indicators_map
            .get(&self.trend_tf)
            .or_else(|| indicators_map.get(&self.signal_tf))?;
        let signal_ind = indicators_map.get(&self.signal_tf)?;
        let entry_ind = indicators_map
            .get(&self.entry_tf)
            .or_else(|| indicators_map.get(&self.signal_tf))?;

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

        // Detect if we are using the fallback (HOUR for trend/entry TF)
        let using_fallback_tf = indicators_map.get(&self.trend_tf).is_none()
            || indicators_map.get(&self.entry_tf).is_none();

        // Check alignment
        if is_trend_bullish && is_signal_bullish {
            // Wait for a slight pullback on the entry timeframe before firing
            if entry_rsi < 45.0 {
                let strength = self.calculate_signal_strength(
                    trend_adx,
                    sig_macd,
                    sig_prev_macd,
                    entry_rsi,
                    true,
                    using_fallback_tf,
                );
                let (stop_loss, take_profit, trailing_stop_distance) =
                    self.calculate_stops_and_targets(Direction::Buy, price, entry_ind);

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
                    trailing_stop_distance,
                    timestamp: Utc::now(),
                });
            }
        }

        if is_trend_bearish && is_signal_bearish {
            // Wait for a slight pullback on the entry timeframe before firing
            if entry_rsi > 55.0 {
                let strength = self.calculate_signal_strength(
                    trend_adx,
                    sig_macd,
                    sig_prev_macd,
                    entry_rsi,
                    false,
                    using_fallback_tf,
                );
                let (stop_loss, take_profit, trailing_stop_distance) =
                    self.calculate_stops_and_targets(Direction::Sell, price, entry_ind);

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
                    trailing_stop_distance,
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
