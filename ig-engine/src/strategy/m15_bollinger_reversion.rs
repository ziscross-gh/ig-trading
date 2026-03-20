//! M15 Bollinger Reversion Strategy
//!
//! Signal logic: M15 %B < 0.05 (oversold) + M15 RSI < rsi_threshold (35)
//! + H1 RSI > h1_rsi_confirm (35) — mean reversion to middle band.
//!
//! Active regime: RANGING ONLY
//! Take profit: Bollinger middle band (not ATR-based)
//! Risk: 0.75× M15 ATR stop

use crate::engine::state::{Direction, Signal};
use crate::indicators::IndicatorSnapshot;
use crate::strategy::traits::M15Strategy;
use chrono::Utc;

pub struct M15BollingerReversionStrategy {
    weight: f64,
    percent_b_threshold: f64,
    rsi_threshold: f64,
    h1_rsi_confirm: f64,
    atr_sl_multiplier: f64,
}

impl M15BollingerReversionStrategy {
    pub fn new(
        weight: f64,
        percent_b_threshold: f64,
        rsi_threshold: f64,
        h1_rsi_confirm: f64,
        atr_sl_multiplier: f64,
    ) -> Self {
        Self {
            weight,
            percent_b_threshold,
            rsi_threshold,
            h1_rsi_confirm,
            atr_sl_multiplier,
        }
    }
}

impl M15Strategy for M15BollingerReversionStrategy {
    fn name(&self) -> &str {
        "M15_BollingerReversion"
    }

    fn weight(&self) -> f64 {
        self.weight
    }

    fn warmup_period(&self) -> usize {
        50
    }

    fn evaluate_m15(
        &self,
        epic: &str,
        price: f64,
        m15_snapshot: &IndicatorSnapshot,
        h1_snapshot: &IndicatorSnapshot,
        regime: &str,
    ) -> Option<Signal> {
        // RANGING ONLY — this strategy makes no sense in trending or volatile markets
        if regime != "RANGING" && !regime.is_empty() {
            return None;
        }

        let percent_b = m15_snapshot.bollinger_percent_b?;
        let bb_middle = m15_snapshot.bollinger_middle?;
        let rsi = m15_snapshot.rsi?;
        let h1_rsi = h1_snapshot.rsi?;
        let atr = m15_snapshot.atr?;

        let sl_dist = atr * self.atr_sl_multiplier;

        // BUY: price at lower band, oversold, H1 RSI not extreme
        let direction = if percent_b < self.percent_b_threshold
            && rsi < self.rsi_threshold
            && h1_rsi > self.h1_rsi_confirm
        {
            Direction::Buy
        } else if percent_b > (1.0 - self.percent_b_threshold)
            && rsi > (100.0 - self.rsi_threshold)
            && h1_rsi < (100.0 - self.h1_rsi_confirm)
        {
            Direction::Sell
        } else {
            return None;
        };

        // Strength based on how extreme the %B is
        let strength = 7.0_f64
            + if !(0.02..=0.98).contains(&percent_b) {
                1.5
            } else if !(0.04..=0.96).contains(&percent_b) {
                0.5
            } else {
                0.0
            };

        let (stop_loss, take_profit) = match &direction {
            Direction::Buy => (price - sl_dist, bb_middle),
            Direction::Sell => (price + sl_dist, bb_middle),
        };

        // Validate the R:R is at least 1.0
        let risk = (price - stop_loss).abs();
        let reward = (take_profit - price).abs();
        if risk <= 0.0 || reward < risk {
            return None;
        }

        Some(Signal {
            id: uuid::Uuid::new_v4().to_string(),
            epic: epic.to_string(),
            direction,
            strength,
            strategy: self.name().to_string(),
            reason: format!(
                "M15 %B={:.3} RSI={:.1} H1_RSI={:.1} target=BB_mid={:.4}",
                percent_b, rsi, h1_rsi, bb_middle
            ),
            price,
            stop_loss,
            take_profit,
            trailing_stop_distance: None,
            timestamp: Utc::now(),
        })
    }
}
