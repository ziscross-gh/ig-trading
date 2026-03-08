use std::collections::HashMap;
use crate::indicators::Candle;

/// An in-progress OHLCV bar for one epic.
#[derive(Debug, Clone)]
struct LiveBar {
    bar_ts: i64,
    open: f64,
    high: f64,
    low: f64,
    close: f64,
}

impl LiveBar {
    fn new(bar_ts: i64, price: f64) -> Self {
        Self { bar_ts, open: price, high: price, low: price, close: price }
    }

    fn update(&mut self, price: f64) {
        if price > self.high { self.high = price; }
        if price < self.low  { self.low  = price; }
        self.close = price;
    }

    fn to_candle(&self) -> Candle {
        Candle {
            timestamp: self.bar_ts,
            open:      self.open,
            high:      self.high,
            low:       self.low,
            close:     self.close,
            volume:    0,
        }
    }
}

/// Accumulates real-time price ticks into OHLCV bars.
///
/// Call [`update`] on every incoming mid-price tick.  When the bar boundary
/// is crossed, the *completed* bar is returned so the caller can push it to
/// [`CandleStore`] and update [`IndicatorSet`].
pub struct BarAccumulator {
    resolution_secs: i64,
    bars: HashMap<String, LiveBar>,
}

impl BarAccumulator {
    /// `resolution_secs`: bar length in seconds (e.g. 3600 for 1-hour bars).
    pub fn new(resolution_secs: i64) -> Self {
        Self { resolution_secs, bars: HashMap::new() }
    }

    fn bar_ts(&self, ts: i64) -> i64 {
        ts - (ts % self.resolution_secs)
    }

    /// Returns the bar-start timestamp of the currently open bar for `epic`,
    /// or `None` if no tick has been received yet.
    pub fn current_bar_ts(&self, epic: &str) -> Option<i64> {
        self.bars.get(epic).map(|b| b.bar_ts)
    }

    /// Feed a new mid-price tick for `epic` at unix time `now_ts`.
    ///
    /// Returns `Some(Candle)` containing the *just-completed* bar if the tick
    /// crossed a bar boundary, otherwise `None`.
    pub fn update(&mut self, epic: &str, price: f64, now_ts: i64) -> Option<Candle> {
        let bar_ts = self.bar_ts(now_ts);

        match self.bars.get_mut(epic) {
            Some(bar) if bar.bar_ts == bar_ts => {
                bar.update(price);
                None
            }
            Some(bar) => {
                let completed = bar.to_candle();
                *bar = LiveBar::new(bar_ts, price);
                Some(completed)
            }
            None => {
                self.bars.insert(epic.to_string(), LiveBar::new(bar_ts, price));
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_bar_accumulates_high_low() {
        let mut acc = BarAccumulator::new(3600);
        let base = 1_700_000_000_i64;
        assert!(acc.update("EPIC", 1.10, base).is_none());
        assert!(acc.update("EPIC", 1.15, base + 60).is_none());
        assert!(acc.update("EPIC", 1.05, base + 120).is_none());
        // Still in the same bar — no completed candle yet
        let bars = &acc.bars;
        let b = bars.get("EPIC").unwrap();
        assert_eq!(b.open, 1.10);
        assert!((b.high - 1.15).abs() < 1e-10);
        assert!((b.low  - 1.05).abs() < 1e-10);
        assert!((b.close - 1.05).abs() < 1e-10);
    }

    #[test]
    fn new_bar_returns_completed_candle() {
        let mut acc = BarAccumulator::new(3600);
        let base = 1_700_000_000_i64;
        acc.update("EPIC", 1.10, base);
        acc.update("EPIC", 1.20, base + 60);
        // Tick in the next hour — should complete the previous bar
        let completed = acc.update("EPIC", 1.30, base + 3600);
        assert!(completed.is_some());
        let c = completed.unwrap();
        assert_eq!(c.timestamp, acc.bars.get("EPIC").unwrap().bar_ts - 3600);
        assert!((c.open  - 1.10).abs() < 1e-10);
        assert!((c.high  - 1.20).abs() < 1e-10);
        assert!((c.close - 1.20).abs() < 1e-10);
    }
}
