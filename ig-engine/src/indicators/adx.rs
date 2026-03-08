#![allow(dead_code)]
/// Average Directional Index — measures trend strength
///
/// ADX > 25 = trending market
/// ADX < 20 = ranging/choppy market
/// +DI > -DI = bullish trend
/// -DI > +DI = bearish trend
use super::ADXValues;

pub fn calculate(
    highs: &[f64],
    lows: &[f64],
    closes: &[f64],
    period: usize,
) -> ADXValues {
    let len = highs.len().min(lows.len()).min(closes.len());
    if len < period * 2 + 1 {
        return ADXValues { adx: 0.0, plus_di: 0.0, minus_di: 0.0 };
    }

    let period_f = period as f64;
    let mut plus_dm_sum = 0.0;
    let mut minus_dm_sum = 0.0;
    let mut tr_sum = 0.0;

    // Initial sums
    for i in 1..=period {
        let plus_dm = if highs[i] - highs[i - 1] > lows[i - 1] - lows[i]
            && highs[i] - highs[i - 1] > 0.0
        {
            highs[i] - highs[i - 1]
        } else {
            0.0
        };
        let minus_dm = if lows[i - 1] - lows[i] > highs[i] - highs[i - 1]
            && lows[i - 1] - lows[i] > 0.0
        {
            lows[i - 1] - lows[i]
        } else {
            0.0
        };
        let tr = (highs[i] - lows[i])
            .max((highs[i] - closes[i - 1]).abs())
            .max((lows[i] - closes[i - 1]).abs());

        plus_dm_sum += plus_dm;
        minus_dm_sum += minus_dm;
        tr_sum += tr;
    }

    // Wilder's smoothing
    let mut dx_values = Vec::new();
    for i in (period + 1)..len {
        let plus_dm = if highs[i] - highs[i - 1] > lows[i - 1] - lows[i]
            && highs[i] - highs[i - 1] > 0.0
        {
            highs[i] - highs[i - 1]
        } else {
            0.0
        };
        let minus_dm = if lows[i - 1] - lows[i] > highs[i] - highs[i - 1]
            && lows[i - 1] - lows[i] > 0.0
        {
            lows[i - 1] - lows[i]
        } else {
            0.0
        };
        let tr = (highs[i] - lows[i])
            .max((highs[i] - closes[i - 1]).abs())
            .max((lows[i] - closes[i - 1]).abs());

        plus_dm_sum = plus_dm_sum - (plus_dm_sum / period_f) + plus_dm;
        minus_dm_sum = minus_dm_sum - (minus_dm_sum / period_f) + minus_dm;
        tr_sum = tr_sum - (tr_sum / period_f) + tr;

        if tr_sum > 0.0 {
            let plus_di = 100.0 * plus_dm_sum / tr_sum;
            let minus_di = 100.0 * minus_dm_sum / tr_sum;
            let di_sum = plus_di + minus_di;
            if di_sum > 0.0 {
                let dx = 100.0 * (plus_di - minus_di).abs() / di_sum;
                dx_values.push((dx, plus_di, minus_di));
            }
        }
    }

    if dx_values.len() < period {
        return ADXValues { adx: 0.0, plus_di: 0.0, minus_di: 0.0 };
    }

    // ADX = smoothed average of DX
    let mut adx: f64 = dx_values[..period].iter().map(|(dx, _, _)| dx).sum::<f64>() / period_f;
    for &(dx, _, _) in &dx_values[period..] {
        adx = (adx * (period_f - 1.0) + dx) / period_f;
    }

    let (_, plus_di, minus_di) = dx_values.last().unwrap_or(&(0.0, 0.0, 0.0));

    ADXValues {
        adx,
        plus_di: *plus_di,
        minus_di: *minus_di,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adx_trending() {
        // Strong uptrend
        let n = 60;
        let highs: Vec<f64> = (0..n).map(|i| 100.0 + i as f64 * 1.0 + 0.5).collect();
        let lows: Vec<f64> = (0..n).map(|i| 100.0 + i as f64 * 1.0 - 0.5).collect();
        let closes: Vec<f64> = (0..n).map(|i| 100.0 + i as f64 * 1.0).collect();

        let result = calculate(&highs, &lows, &closes, 14);
        assert!(result.adx > 20.0, "ADX should indicate trend, got {}", result.adx);
    }
}
