#![allow(dead_code)]
/// Average True Range — measures volatility
///
/// TR = max(high-low, |high-prev_close|, |low-prev_close|)
/// ATR = Wilder's smoothed average of TR over `period`
pub fn calculate(highs: &[f64], lows: &[f64], closes: &[f64], period: usize) -> f64 {
    let len = highs.len().min(lows.len()).min(closes.len());
    if len < period + 1 {
        return 0.0;
    }

    // Calculate true ranges
    let mut tr_values = Vec::with_capacity(len - 1);
    for i in 1..len {
        let tr = (highs[i] - lows[i])
            .max((highs[i] - closes[i - 1]).abs())
            .max((lows[i] - closes[i - 1]).abs());
        tr_values.push(tr);
    }

    if tr_values.len() < period {
        return 0.0;
    }

    // Initial ATR = simple average
    let mut atr: f64 = tr_values[..period].iter().sum::<f64>() / period as f64;

    // Wilder's smoothing for remaining
    let period_f = period as f64;
    for &tr in &tr_values[period..] {
        atr = (atr * (period_f - 1.0) + tr) / period_f;
    }

    atr
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_atr_basic() {
        let highs = vec![10.0, 11.0, 12.0, 11.5, 13.0, 12.5, 14.0, 13.5, 15.0, 14.5];
        let lows = vec![9.0, 9.5, 10.0, 10.0, 11.0, 11.0, 12.0, 12.0, 13.0, 13.0];
        let closes = vec![9.5, 10.5, 11.5, 11.0, 12.5, 12.0, 13.5, 13.0, 14.5, 14.0];

        let atr = calculate(&highs, &lows, &closes, 5);
        assert!(atr > 0.0, "ATR should be positive");
        assert!(atr < 5.0, "ATR should be reasonable");
    }
}
