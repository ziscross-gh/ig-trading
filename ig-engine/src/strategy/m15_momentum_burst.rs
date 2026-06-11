//! M15 Momentum Burst Strategy
//!
//! Signal logic: M15 RSI in momentum zone (55–75 bullish / 25–45 bearish)
//! + M15 MACD histogram expanding + H1 EMA200 trend confirmation.
//!
//! Active regimes: TRENDING, VOLATILE, RANGING (at RSI extremes < 35 or > 65 only)
//! Regime multipliers applied in analyze_market_m15(): VOLATILE 1.3×, TRENDING 1.2×, RANGING 1.0×
//! Risk: SL/TP from config atr_sl_multiplier / atr_tp_multiplier (default.toml: 1.5× / 4.0× M15 ATR);
//! per-instrument overrides in [strategies.instrument_overrides] may recompute them post-ensemble.

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
    pub fn new(
        weight: f64,
        rsi_min: f64,
        rsi_max: f64,
        atr_sl_multiplier: f64,
        atr_tp_multiplier: f64,
    ) -> Self {
        Self {
            weight,
            rsi_min,
            rsi_max,
            atr_sl_multiplier,
            atr_tp_multiplier,
        }
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
        if ranging_mode && (35.0..=65.0).contains(&rsi) {
            return None;
        }

        let macd_hist = m15_snapshot.macd_histogram?;
        let prev_macd_hist = m15_snapshot.prev_macd_histogram?;
        let atr = m15_snapshot.atr?;
        let h1_ema200 = h1_snapshot.ema_200?;

        let sl_dist = atr * self.atr_sl_multiplier;
        let tp_dist = atr * self.atr_tp_multiplier;

        // Phase 17.E (option A): dropped the MACD-histogram *expansion* requirement
        // (macd_hist vs prev_macd_hist). In mature trends (ADX 50+) momentum decelerates,
        // so the histogram is rarely still expanding even when the trend is intact — this
        // made MomentumBurst go 100% silent in TRENDING/VOLATILE and pinned the M15 ensemble
        // at 1/3 consensus (unreachable barrier of 2). We keep the three robust confirmations:
        // RSI momentum zone + MACD sign + price vs H1 EMA200. prev_macd_hist is retained for
        // the reason string and contributes to strength below.
        let macd_decelerating = match () {
            _ if macd_hist > 0.0 => macd_hist < prev_macd_hist, // bullish hist shrinking
            _ if macd_hist < 0.0 => macd_hist > prev_macd_hist, // bearish hist shrinking
            _ => false,
        };
        let bull =
            rsi >= self.rsi_min && rsi <= self.rsi_max && macd_hist > 0.0 && price > h1_ema200;
        let bear = rsi >= (100.0 - self.rsi_max)
            && rsi <= (100.0 - self.rsi_min)
            && macd_hist < 0.0
            && price < h1_ema200;
        let direction = if bull {
            Direction::Buy
        } else if bear {
            Direction::Sell
        } else {
            return None;
        };

        // Strength: base 7.0 + ADX contribution, minus a penalty when the histogram is
        // decelerating (the old hard expansion gate is now a soft quality score instead).
        let adx = m15_snapshot.adx.unwrap_or(0.0);
        let strength = (7.0_f64
            + if adx > 40.0 {
                2.0
            } else if adx > 30.0 {
                1.0
            } else {
                0.0
            }
            - if macd_decelerating { 1.5 } else { 0.0 })
        .max(0.0);

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
