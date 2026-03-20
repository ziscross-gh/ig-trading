//! M15 Momentum Burst Strategy
//!
//! Signal logic: M15 RSI in momentum zone (55–75 bullish / 25–45 bearish)
//! + M15 MACD histogram expanding + H1 EMA200 trend confirmation.
//!
//! Active regimes: TRENDING, VOLATILE, RANGING (at RSI extremes < 35 or > 65 only)
//! Regime multipliers applied in analyze_market_m15(): VOLATILE 1.3×, TRENDING 1.2×, RANGING 1.0×
//! Risk: 0.75× M15 ATR stop, 1.5× ATR take profit

use crate::engine::state::{Direction, Signal};
use crate::indicators::IndicatorSnapshot;
use crate::strategy::traits::M15Strategy;
use chrono::Utc;

pub struct M15MomentumBurstStrategy {
    weight: f64,
    rsi_min: f64,
    rsi_max: f64,
    atr_sl_multiplier: f64,
    atr_tp_multiplier: f64,
}

impl M15MomentumBurstStrategy {
    pub fn new(weight: f64, rsi_min: f64, rsi_max: f64, atr_sl_multiplier: f64, atr_tp_multiplier: f64) -> Self {
        Self { weight, rsi_min, rsi_max, atr_sl_multiplier, atr_tp_multiplier }
    }
}

impl M15Strategy for M15MomentumBurstStrategy {
    fn name(&self) -> &str {
        "M15_MomentumBurst"
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
        // Active in TRENDING and VOLATILE fully.
        // Also active in RANGING but only at range extremes (RSI < 35 or RSI > 65):
        // momentum bursts at support/resistance boundaries are valid range trades.
        let ranging_mode = regime == "RANGING";
        match regime {
            "TRENDING" | "VOLATILE" | "RANGING" | "" => {}
            _ => return None,
        }

        let rsi = m15_snapshot.rsi?;

        // In RANGING: only fire at clear S/R extremes — momentum burst from oversold/overbought
        if ranging_mode && !(rsi < 35.0 || rsi > 65.0) {
            return None;
        }

        let macd_hist = m15_snapshot.macd_histogram?;
        let prev_macd_hist = m15_snapshot.prev_macd_histogram?;
        let atr = m15_snapshot.atr?;
        let h1_ema200 = h1_snapshot.ema_200?;

        // Strength: base 7.0 + ADX contribution
        let adx = m15_snapshot.adx.unwrap_or(0.0);
        let strength = 7.0_f64
            + if adx > 40.0 { 2.0 } else if adx > 30.0 { 1.0 } else { 0.0 };

        let sl_dist = atr * self.atr_sl_multiplier;
        let tp_dist = atr * self.atr_tp_multiplier;

        let direction = if rsi >= self.rsi_min && rsi <= self.rsi_max
            && macd_hist > 0.0
            && macd_hist > prev_macd_hist
            && price > h1_ema200
        {
            Direction::Buy
        } else if rsi >= (100.0 - self.rsi_max) && rsi <= (100.0 - self.rsi_min)
            && macd_hist < 0.0
            && macd_hist < prev_macd_hist
            && price < h1_ema200
        {
            Direction::Sell
        } else {
            return None;
        };

        let (stop_loss, take_profit) = match &direction {
            Direction::Buy  => (price - sl_dist, price + tp_dist),
            Direction::Sell => (price + sl_dist, price - tp_dist),
        };

        Some(Signal {
            id: uuid::Uuid::new_v4().to_string(),
            epic: epic.to_string(),
            direction,
            strength,
            strategy: self.name().to_string(),
            reason: format!(
                "M15 RSI={:.1} MACD_hist={:.4}>{:.4} H1_EMA200={:.2} ADX={:.1}",
                rsi, macd_hist, prev_macd_hist, h1_ema200, adx
            ),
            price,
            stop_loss,
            take_profit,
            trailing_stop_distance: None,
            timestamp: Utc::now(),
        })
    }
}
