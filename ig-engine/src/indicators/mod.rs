#![allow(dead_code)]
//! Technical Indicators Library
//!
//! All indicators use ring buffers for O(1) updates on new candles.
//! Calculations use f64 for speed — we only need rust_decimal for order sizing.

pub mod sma;
pub mod ema;
pub mod rsi;
pub mod macd;
pub mod atr;
pub mod bollinger;
pub mod adx;
pub mod stochastic;

use serde::{Deserialize, Serialize};

/// A single OHLCV candle
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Candle {
    pub timestamp: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: u64,
}

/// Complete indicator snapshot for a given moment.
/// All fields are Option<f64> to support strategies using the `?` operator
/// for graceful handling of missing/unwarmed indicators.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IndicatorSnapshot {
    // Simple Moving Averages
    pub sma_short: Option<f64>,
    pub sma_long: Option<f64>,

    // Exponential Moving Averages
    pub ema_short: Option<f64>,
    pub ema_long: Option<f64>,
    pub ema_200: Option<f64>,        // 200-period EMA for trend filter

    // Previous EMA values (for crossover detection)
    pub prev_ema_short: Option<f64>,
    pub prev_ema_long: Option<f64>,

    // RSI
    pub rsi: Option<f64>,

    // MACD (flattened)
    pub macd: Option<f64>,           // MACD line
    pub macd_signal: Option<f64>,    // Signal line
    pub macd_histogram: Option<f64>,
    pub prev_macd: Option<f64>,      // Previous MACD line
    pub prev_macd_histogram: Option<f64>,

    // ATR
    pub atr: Option<f64>,

    // Bollinger Bands (flattened)
    pub bollinger_upper: Option<f64>,
    pub bollinger_middle: Option<f64>,
    pub bollinger_lower: Option<f64>,
    pub bollinger_bandwidth: Option<f64>,
    pub bollinger_percent_b: Option<f64>,

    // ADX (flattened)
    pub adx: Option<f64>,
    pub plus_di: Option<f64>,
    pub minus_di: Option<f64>,

    // Stochastic (flattened)
    pub stochastic_k: Option<f64>,
    pub stochastic_d: Option<f64>,
}

/// Helper sub-structs used internally during calculation (not in snapshot)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BollingerValues {
    pub upper: f64,
    pub middle: f64,
    pub lower: f64,
    pub bandwidth: f64,
    pub percent_b: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ADXValues {
    pub adx: f64,
    pub plus_di: f64,
    pub minus_di: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StochasticValues {
    pub k: f64,
    pub d: f64,
}

/// Ring buffer for efficient indicator computation
#[derive(Debug, Clone)]
pub struct RingBuffer {
    data: Vec<f64>,
    capacity: usize,
    head: usize,
    len: usize,
}

impl RingBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            data: vec![0.0; capacity],
            capacity,
            head: 0,
            len: 0,
        }
    }

    pub fn push(&mut self, value: f64) {
        self.data[self.head] = value;
        self.head = (self.head + 1) % self.capacity;
        if self.len < self.capacity {
            self.len += 1;
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn is_full(&self) -> bool {
        self.len == self.capacity
    }

    /// Get the most recent N values (newest first)
    pub fn recent(&self, n: usize) -> Vec<f64> {
        let n = n.min(self.len);
        let mut result = Vec::with_capacity(n);
        for i in 0..n {
            let idx = (self.head + self.capacity - 1 - i) % self.capacity;
            result.push(self.data[idx]);
        }
        result
    }

    /// Get the last (most recent) value
    pub fn last(&self) -> Option<f64> {
        if self.len == 0 {
            None
        } else {
            Some(self.data[(self.head + self.capacity - 1) % self.capacity])
        }
    }

    /// Get value at offset from most recent (0 = most recent, 1 = previous, etc.)
    pub fn at(&self, offset: usize) -> Option<f64> {
        if offset >= self.len {
            None
        } else {
            let idx = (self.head + self.capacity - 1 - offset) % self.capacity;
            Some(self.data[idx])
        }
    }

    /// Sum of all values in buffer
    pub fn sum(&self) -> f64 {
        if self.len == self.capacity {
            self.data.iter().sum()
        } else {
            self.data[..self.len].iter().sum()
        }
    }

    /// Get all values in chronological order (oldest first)
    pub fn as_slice_ordered(&self) -> Vec<f64> {
        let mut result = Vec::with_capacity(self.len);
        for i in 0..self.len {
            let idx = (self.head + self.capacity - self.len + i) % self.capacity;
            result.push(self.data[idx]);
        }
        result
    }
}

/// Full indicator set that maintains state across candle updates
#[derive(Debug, Clone)]
pub struct IndicatorSet {
    // Configuration
    pub short_period: usize,
    pub long_period: usize,
    pub trend_period: usize,
    pub rsi_period: usize,
    pub atr_period: usize,
    pub bb_period: usize,
    pub bb_std_dev: f64,
    pub adx_period: usize,
    pub stoch_period: usize,

    // State
    closes: RingBuffer,
    highs: RingBuffer,
    lows: RingBuffer,

    // EMA state (we keep running EMA values)
    ema_short: Option<f64>,
    ema_long: Option<f64>,
    ema_trend: Option<f64>,
    prev_ema_short: Option<f64>,
    prev_ema_long: Option<f64>,

    // RSI state
    avg_gain: Option<f64>,
    avg_loss: Option<f64>,
    prev_close: Option<f64>,

    // MACD state
    macd_ema_fast: Option<f64>,
    macd_ema_slow: Option<f64>,
    macd_signal: Option<f64>,
    prev_macd_line: Option<f64>,
    prev_histogram: f64,

    // ATR state
    atr_value: Option<f64>,

    // ADX state
    prev_high: Option<f64>,
    prev_low: Option<f64>,
    plus_dm_ema: Option<f64>,
    minus_dm_ema: Option<f64>,
    tr_ema: Option<f64>,
    adx_ema: Option<f64>,

    // Count of updates for warmup tracking
    update_count: usize,
}

impl IndicatorSet {
    #[allow(clippy::too_many_arguments)]
    // TODO: bundle args into an IndicatorConfig struct to reduce parameter count
    pub fn new(
        short_period: usize,
        long_period: usize,
        trend_period: usize,
        rsi_period: usize,
        atr_period: usize,
        bb_period: usize,
        bb_std_dev: f64,
        adx_period: usize,
        stoch_period: usize,
    ) -> Self {
        let max_period = *[short_period, long_period, trend_period, bb_period, adx_period, stoch_period, 200]
            .iter()
            .max()
            .unwrap_or(&200);

        Self {
            short_period,
            long_period,
            trend_period,
            rsi_period,
            atr_period,
            bb_period,
            bb_std_dev,
            adx_period,
            stoch_period,
            closes: RingBuffer::new(max_period + 50),
            highs: RingBuffer::new(max_period + 50),
            lows: RingBuffer::new(max_period + 50),
            ema_short: None,
            ema_long: None,
            ema_trend: None,
            prev_ema_short: None,
            prev_ema_long: None,
            avg_gain: None,
            avg_loss: None,
            prev_close: None,
            macd_ema_fast: None,
            macd_ema_slow: None,
            macd_signal: None,
            prev_macd_line: None,
            prev_histogram: 0.0,
            atr_value: None,
            prev_high: None,
            prev_low: None,
            plus_dm_ema: None,
            minus_dm_ema: None,
            tr_ema: None,
            adx_ema: None,
            update_count: 0,
        }
    }

    /// Default configuration matching the TOML defaults
    pub fn default_config() -> Self {
        Self::new(9, 21, 200, 14, 14, 20, 2.0, 14, 14)
    }

    /// Minimum candles needed before indicators are valid
    pub fn warmup_period(&self) -> usize {
        self.trend_period + 10 // 200 + buffer
    }

    pub fn is_warmed_up(&self) -> bool {
        self.update_count >= self.warmup_period()
    }

    /// Update all indicators with a new candle
    pub fn update(&mut self, candle: &Candle) {
        let close = candle.close;
        let high = candle.high;
        let low = candle.low;

        self.closes.push(close);
        self.highs.push(high);
        self.lows.push(low);

        // Update EMAs (save previous values first for crossover detection)
        self.prev_ema_short = self.ema_short;
        self.prev_ema_long = self.ema_long;
        self.ema_short = Some(ema::update_ema(self.ema_short, close, self.short_period));
        self.ema_long = Some(ema::update_ema(self.ema_long, close, self.long_period));
        self.ema_trend = Some(ema::update_ema(self.ema_trend, close, self.trend_period));

        // Update RSI
        if let Some(prev) = self.prev_close {
            let change = close - prev;
            let gain = if change > 0.0 { change } else { 0.0 };
            let loss = if change < 0.0 { -change } else { 0.0 };

            let period = self.rsi_period as f64;
            self.avg_gain = Some(match self.avg_gain {
                Some(ag) => (ag * (period - 1.0) + gain) / period,
                None => gain,
            });
            self.avg_loss = Some(match self.avg_loss {
                Some(al) => (al * (period - 1.0) + loss) / period,
                None => loss,
            });
        }
        self.prev_close = Some(close);

        // Update MACD (12, 26, 9)
        // Save previous MACD line before recalculating
        if let (Some(fast), Some(slow)) = (self.macd_ema_fast, self.macd_ema_slow) {
            self.prev_macd_line = Some(fast - slow);
        }
        self.macd_ema_fast = Some(ema::update_ema(self.macd_ema_fast, close, 12));
        self.macd_ema_slow = Some(ema::update_ema(self.macd_ema_slow, close, 26));
        if let (Some(fast), Some(slow)) = (self.macd_ema_fast, self.macd_ema_slow) {
            let macd_line = fast - slow;
            let old_signal = self.macd_signal;
            self.macd_signal = Some(ema::update_ema(self.macd_signal, macd_line, 9));
            if let Some(sig) = old_signal {
                self.prev_histogram = macd_line - sig;
            }
        }

        // Update ATR
        if let Some(prev_c) = self.closes.at(1) {
            let tr = (high - low)
                .max((high - prev_c).abs())
                .max((low - prev_c).abs());

            self.atr_value = Some(match self.atr_value {
                Some(atr) => {
                    let period = self.atr_period as f64;
                    (atr * (period - 1.0) + tr) / period
                }
                None => tr,
            });
        }

        // Update ADX
        if let (Some(prev_h), Some(prev_l)) = (self.prev_high, self.prev_low) {
            let plus_dm = if high - prev_h > prev_l - low && high - prev_h > 0.0 {
                high - prev_h
            } else {
                0.0
            };
            let minus_dm = if prev_l - low > high - prev_h && prev_l - low > 0.0 {
                prev_l - low
            } else {
                0.0
            };

            let tr = if let Some(prev_c) = self.closes.at(1) {
                (high - low)
                    .max((high - prev_c).abs())
                    .max((low - prev_c).abs())
            } else {
                high - low
            };

            let period = self.adx_period as f64;
            self.plus_dm_ema = Some(match self.plus_dm_ema {
                Some(v) => (v * (period - 1.0) + plus_dm) / period,
                None => plus_dm,
            });
            self.minus_dm_ema = Some(match self.minus_dm_ema {
                Some(v) => (v * (period - 1.0) + minus_dm) / period,
                None => minus_dm,
            });
            self.tr_ema = Some(match self.tr_ema {
                Some(v) => (v * (period - 1.0) + tr) / period,
                None => tr,
            });

            if let (Some(tr_e), Some(pdm), Some(mdm)) =
                (self.tr_ema, self.plus_dm_ema, self.minus_dm_ema)
            {
                if tr_e > 0.0 {
                    let plus_di = 100.0 * pdm / tr_e;
                    let minus_di = 100.0 * mdm / tr_e;
                    let di_sum = plus_di + minus_di;
                    if di_sum > 0.0 {
                        let dx = 100.0 * (plus_di - minus_di).abs() / di_sum;
                        self.adx_ema = Some(match self.adx_ema {
                            Some(v) => (v * (period - 1.0) + dx) / period,
                            None => dx,
                        });
                    }
                }
            }
        }
        self.prev_high = Some(high);
        self.prev_low = Some(low);

        self.update_count += 1;
    }

    /// Get current indicator snapshot with flat Option<f64> fields
    pub fn snapshot(&self) -> Option<IndicatorSnapshot> {
        if !self.is_warmed_up() {
            return None;
        }

        let closes_ordered = self.closes.as_slice_ordered();

        // SMA
        let sma_short = sma::calculate(&closes_ordered, self.short_period);
        let sma_long = sma::calculate(&closes_ordered, self.long_period);

        // RSI
        let rsi = match (self.avg_gain, self.avg_loss) {
            (Some(ag), Some(al)) if al > 0.0 => Some(100.0 - (100.0 / (1.0 + ag / al))),
            (Some(_), Some(_)) => Some(100.0),
            _ => Some(50.0),
        };

        // MACD
        let macd_line = match (self.macd_ema_fast, self.macd_ema_slow) {
            (Some(fast), Some(slow)) => Some(fast - slow),
            _ => None,
        };
        let signal_line = self.macd_signal;
        let histogram = match (macd_line, signal_line) {
            (Some(m), Some(s)) => Some(m - s),
            _ => None,
        };

        // Bollinger
        let bb = bollinger::calculate(&closes_ordered, self.bb_period, self.bb_std_dev);

        // Stochastic
        let stoch = stochastic::calculate(
            &self.highs.as_slice_ordered(),
            &self.lows.as_slice_ordered(),
            &closes_ordered,
            self.stoch_period,
        );

        // ADX
        let adx_val = self.adx_ema;
        let plus_di = match (self.tr_ema, self.plus_dm_ema) {
            (Some(tr_e), Some(pdm)) if tr_e > 0.0 => Some(100.0 * pdm / tr_e),
            _ => None,
        };
        let minus_di = match (self.tr_ema, self.minus_dm_ema) {
            (Some(tr_e), Some(mdm)) if tr_e > 0.0 => Some(100.0 * mdm / tr_e),
            _ => None,
        };

        // Previous MACD histogram
        let prev_macd_histogram = if self.update_count > 1 {
            Some(self.prev_histogram)
        } else {
            None
        };

        Some(IndicatorSnapshot {
            sma_short: Some(sma_short),
            sma_long: Some(sma_long),
            ema_short: self.ema_short,
            ema_long: self.ema_long,
            ema_200: self.ema_trend,
            prev_ema_short: self.prev_ema_short,
            prev_ema_long: self.prev_ema_long,
            rsi,
            macd: macd_line,
            macd_signal: signal_line,
            macd_histogram: histogram,
            prev_macd: self.prev_macd_line,
            prev_macd_histogram,
            atr: self.atr_value,
            bollinger_upper: Some(bb.upper),
            bollinger_middle: Some(bb.middle),
            bollinger_lower: Some(bb.lower),
            bollinger_bandwidth: Some(bb.bandwidth),
            bollinger_percent_b: Some(bb.percent_b),
            adx: adx_val,
            plus_di,
            minus_di,
            stochastic_k: Some(stoch.k),
            stochastic_d: Some(stoch.d),
        })
    }
}
