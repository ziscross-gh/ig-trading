#![allow(dead_code)]

use super::ema;

/// MACD result
#[derive(Debug, Clone)]
pub struct MACDResult {
    pub macd_line: f64,
    pub signal_line: f64,
    pub histogram: f64,
    pub crossover: Crossover,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Crossover {
    Bullish,  // MACD crossed above signal
    Bearish,  // MACD crossed below signal
    None,
}

/// Calculate MACD from close prices
pub fn calculate(
    closes: &[f64],
    fast_period: usize,
    slow_period: usize,
    signal_period: usize,
) -> MACDResult {
    if closes.len() < slow_period + signal_period {
        return MACDResult {
            macd_line: 0.0,
            signal_line: 0.0,
            histogram: 0.0,
            crossover: Crossover::None,
        };
    }

    let fast_ema = ema::series(closes, fast_period);
    let slow_ema = ema::series(closes, slow_period);

    // MACD line = fast EMA - slow EMA
    let macd_line: Vec<f64> = fast_ema
        .iter()
        .zip(slow_ema.iter())
        .map(|(f, s)| f - s)
        .collect();

    // Signal line = EMA of MACD line
    let signal_ema = ema::series(&macd_line, signal_period);

    let current_macd = *macd_line.last().unwrap_or(&0.0);
    let current_signal = *signal_ema.last().unwrap_or(&0.0);
    let histogram = current_macd - current_signal;

    // Detect crossover
    let crossover = if macd_line.len() >= 2 && signal_ema.len() >= 2 {
        let prev_macd = macd_line[macd_line.len() - 2];
        let prev_signal = signal_ema[signal_ema.len() - 2];

        if prev_macd <= prev_signal && current_macd > current_signal {
            Crossover::Bullish
        } else if prev_macd >= prev_signal && current_macd < current_signal {
            Crossover::Bearish
        } else {
            Crossover::None
        }
    } else {
        Crossover::None
    };

    MACDResult {
        macd_line: current_macd,
        signal_line: current_signal,
        histogram,
        crossover,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_macd_basic() {
        // Generate simple trending data
        let closes: Vec<f64> = (0..50).map(|i| 100.0 + i as f64 * 0.5).collect();
        let result = calculate(&closes, 12, 26, 9);
        // In an uptrend, MACD should be positive
        assert!(result.macd_line > 0.0, "MACD line should be positive in uptrend");
    }
}
