#![allow(dead_code)]
use super::{sma, BollingerValues};

/// Calculate Bollinger Bands
pub fn calculate(closes: &[f64], period: usize, std_dev_mult: f64) -> BollingerValues {
    if closes.len() < period {
        return BollingerValues {
            upper: 0.0,
            middle: 0.0,
            lower: 0.0,
            bandwidth: 0.0,
            percent_b: 0.5,
        };
    }

    let middle = sma::calculate(closes, period);
    let slice = &closes[closes.len() - period..];

    // Standard deviation
    let variance = slice.iter().map(|x| (x - middle).powi(2)).sum::<f64>() / period as f64;
    let std_dev = variance.sqrt();

    let upper = middle + std_dev * std_dev_mult;
    let lower = middle - std_dev * std_dev_mult;
    let bandwidth = if middle > 0.0 {
        (upper - lower) / middle
    } else {
        0.0
    };

    let current_price = *closes.last().unwrap_or(&0.0);
    let percent_b = if upper != lower {
        (current_price - lower) / (upper - lower)
    } else {
        0.5
    };

    BollingerValues {
        upper,
        middle,
        lower,
        bandwidth,
        percent_b,
    }
}

/// Detect Bollinger squeeze (bandwidth contraction below threshold)
pub fn is_squeeze(bandwidth_history: &[f64], threshold_percentile: f64) -> bool {
    if bandwidth_history.len() < 20 {
        return false;
    }
    let mut sorted = bandwidth_history.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let idx = ((threshold_percentile / 100.0) * sorted.len() as f64) as usize;
    let threshold = sorted[idx.min(sorted.len() - 1)];
    *bandwidth_history.last().unwrap_or(&0.0) <= threshold
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bollinger_basic() {
        let closes: Vec<f64> = (0..30).map(|i| 100.0 + (i as f64 * 0.1).sin()).collect();
        let bb = calculate(&closes, 20, 2.0);
        assert!(bb.upper > bb.middle);
        assert!(bb.middle > bb.lower);
        assert!(bb.bandwidth > 0.0);
        assert!(bb.percent_b.is_finite()); // percent_b is unbounded; can go outside [0,1] when price is outside bands
    }
}
