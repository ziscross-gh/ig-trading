#![allow(dead_code)]

/// Relative Strength Index calculation from a series of closes
pub fn calculate(closes: &[f64], period: usize) -> f64 {
    if closes.len() < period + 1 {
        return 50.0; // Neutral when insufficient data
    }

    let mut avg_gain = 0.0;
    let mut avg_loss = 0.0;

    // Initial average (simple average of first `period` changes)
    for i in 1..=period {
        let change = closes[i] - closes[i - 1];
        if change > 0.0 {
            avg_gain += change;
        } else {
            avg_loss += -change;
        }
    }
    avg_gain /= period as f64;
    avg_loss /= period as f64;

    // Smoothed RSI (Wilder's method) for remaining data
    let period_f = period as f64;
    for i in (period + 1)..closes.len() {
        let change = closes[i] - closes[i - 1];
        let gain = if change > 0.0 { change } else { 0.0 };
        let loss = if change < 0.0 { -change } else { 0.0 };

        avg_gain = (avg_gain * (period_f - 1.0) + gain) / period_f;
        avg_loss = (avg_loss * (period_f - 1.0) + loss) / period_f;
    }

    if avg_loss == 0.0 {
        return 100.0;
    }
    let rs = avg_gain / avg_loss;
    100.0 - (100.0 / (1.0 + rs))
}

/// Signal interpretation
pub fn signal(rsi: f64, overbought: f64, oversold: f64) -> RsiSignal {
    if rsi >= overbought {
        RsiSignal::Overbought
    } else if rsi <= oversold {
        RsiSignal::Oversold
    } else {
        RsiSignal::Neutral
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum RsiSignal {
    Overbought,
    Oversold,
    Neutral,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rsi_all_gains() {
        let closes: Vec<f64> = (1..=20).map(|x| x as f64).collect();
        let rsi = calculate(&closes, 14);
        assert!(rsi > 95.0, "All gains should give RSI near 100, got {}", rsi);
    }

    #[test]
    fn test_rsi_all_losses() {
        let closes: Vec<f64> = (1..=20).rev().map(|x| x as f64).collect();
        let rsi = calculate(&closes, 14);
        assert!(rsi < 5.0, "All losses should give RSI near 0, got {}", rsi);
    }

    #[test]
    fn test_rsi_signal() {
        assert_eq!(signal(75.0, 70.0, 30.0), RsiSignal::Overbought);
        assert_eq!(signal(25.0, 70.0, 30.0), RsiSignal::Oversold);
        assert_eq!(signal(50.0, 70.0, 30.0), RsiSignal::Neutral);
    }
}
