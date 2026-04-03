//! M15 EMA Microtrend Strategy
//!
//! Signal logic: M15 EMA9 > EMA21 (bullish alignment) + EMA21 slope positive
//! + H1 EMA21 slope confirmation.
//!
//! Active regimes: TRENDING, VOLATILE
//! Regime multipliers applied in analyze_market_m15(): TRENDING 1.2×
//! Risk: 0.75× M15 ATR stop, 1.5× ATR take profit

use crate::engine::state::{Direction, Signal};
use crate::indicators::IndicatorSnapshot;
use crate::strategy::traits::M15Strategy;
use chrono::Utc;

pub struct M15EmaMicrotrendStrategy {
    weight: f64,
    atr_sl_multiplier: f64,
    atr_tp_multiplier: f64,
}

impl M15EmaMicrotrendStrategy {
    pub fn new(weight: f64, atr_sl_multiplier: f64, atr_tp_multiplier: f64) -> Self {
        Self {
            weight,
            atr_sl_multiplier,
            atr_tp_multiplier,
        }
    }
}

impl M15Strategy for M15EmaMicrotrendStrategy {
    fn name(&self) -> &str {
        "M15_EmaMicrotrend"
    }

    fn weight(&self) -> f64 {
        self.weight
    }

    fn warmup_period(&self) -> usize {
        210
    }

    fn evaluate_m15(
        &self,
        epic: &str,
        price: f64,
        m15_snapshot: &IndicatorSnapshot,
        h1_snapshot: &IndicatorSnapshot,
        regime: &str,
    ) -> Option<Signal> {
        // Only active in TRENDING and VOLATILE regimes
        match regime {
            "TRENDING" | "VOLATILE" | "" => {}
            _ => return None,
        }

        let ema_short = m15_snapshot.ema_short?; // EMA9
        let ema_long = m15_snapshot.ema_long?; // EMA21
        let prev_ema_long = m15_snapshot.prev_ema_long?;
        let h1_ema_long = h1_snapshot.ema_long?;
        let h1_prev_ema_long = h1_snapshot.prev_ema_long?;
        let atr = m15_snapshot.atr?;

        let adx = m15_snapshot.adx.unwrap_or(0.0);
        let strength = 7.5_f64
            + if adx > 35.0 {
                1.5
            } else if adx > 25.0 {
                0.5
            } else {
                0.0
            };

        let sl_dist = atr * self.atr_sl_multiplier;
        let tp_dist = atr * self.atr_tp_multiplier;

        let direction = if ema_short > ema_long
            && ema_long > prev_ema_long        // M15 EMA21 slope positive
            && h1_ema_long > h1_prev_ema_long
        // H1 EMA21 slope positive (trend confirmation)
        {
            Direction::Buy
        } else if ema_short < ema_long
            && ema_long < prev_ema_long        // M15 EMA21 slope negative
            && h1_ema_long < h1_prev_ema_long
        // H1 EMA21 slope negative
        {
            Direction::Sell
        } else {
            return None;
        };

        let (stop_loss, take_profit) = match &direction {
            Direction::Buy => (price - sl_dist, price + tp_dist),
            Direction::Sell => (price + sl_dist, price - tp_dist),
        };

        Some(Signal {
            id: uuid::Uuid::new_v4().to_string(),
            epic: epic.to_string(),
            direction,
            strength,
            strategy: self.name().to_string(),
            reason: format!(
                "M15 EMA9={:.4} vs EMA21={:.4} slope={:+.5} H1_slope={:+.5} ADX={:.1}",
                ema_short,
                ema_long,
                ema_long - prev_ema_long,
                h1_ema_long - h1_prev_ema_long,
                adx
            ),
            price,
            stop_loss,
            take_profit,
            trailing_stop_distance: None,
            timestamp: Utc::now(),
        })
    }
}
