use super::StochasticValues;

/// Stochastic Oscillator (%K and %D)
///
/// %K = (Close - Lowest Low) / (Highest High - Lowest Low) × 100
/// %D = 3-period SMA of %K
pub fn calculate(
    highs: &[f64],
    lows: &[f64],
    closes: &[f64],
    period: usize,
) -> StochasticValues {
    let len = highs.len().min(lows.len()).min(closes.len());
    if len < period + 3 {
        return StochasticValues { k: 50.0, d: 50.0 };
    }

    // Calculate %K values for last 3 periods (for %D averaging)
    let mut k_values = Vec::with_capacity(3);
    for offset in 0..3 {
        let end = len - offset;
        let start = end.saturating_sub(period);
        let highest = highs[start..end].iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let lowest = lows[start..end].iter().cloned().fold(f64::INFINITY, f64::min);
        let close = closes[end - 1];

        let k = if highest != lowest {
            ((close - lowest) / (highest - lowest)) * 100.0
        } else {
            50.0
        };
        k_values.push(k);
    }

    let current_k = k_values[0];
    let d = k_values.iter().sum::<f64>() / k_values.len() as f64;

    StochasticValues { k: current_k, d }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stochastic_extreme() {
        // Price at highest high → %K should be near 100
        let highs = vec![10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 16.0, 17.0, 18.0, 19.0, 20.0, 21.0, 22.0, 23.0, 24.0, 25.0, 26.0, 27.0];
        let lows = vec![9.0, 10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 16.0, 17.0, 18.0, 19.0, 20.0, 21.0, 22.0, 23.0, 24.0, 25.0, 26.0];
        let closes = vec![9.5, 10.5, 11.5, 12.5, 13.5, 14.5, 15.5, 16.5, 17.5, 18.5, 19.5, 20.5, 21.5, 22.5, 23.5, 24.5, 25.5, 26.5];

        let result = calculate(&highs, &lows, &closes, 14);
        assert!(result.k > 80.0, "K should be high in uptrend, got {}", result.k);
    }
}
