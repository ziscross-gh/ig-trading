#![allow(dead_code)]

/// Exponential Moving Average — incremental update
///
/// If `prev` is None, returns the raw value (seed).
/// Otherwise applies the EMA formula: EMA = value * k + prev * (1 - k)
pub fn update_ema(prev: Option<f64>, value: f64, period: usize) -> f64 {
    let k = 2.0 / (period as f64 + 1.0);
    match prev {
        Some(prev_ema) => value * k + prev_ema * (1.0 - k),
        None => value,
    }
}

/// Compute full EMA series from data
pub fn series(data: &[f64], period: usize) -> Vec<f64> {
    if data.is_empty() || period == 0 {
        return vec![];
    }
    let k = 2.0 / (period as f64 + 1.0);
    let mut result = Vec::with_capacity(data.len());
    result.push(data[0]);

    for i in 1..data.len() {
        let prev = result[i - 1];
        result.push(data[i] * k + prev * (1.0 - k));
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ema_update() {
        let mut ema = None;
        // First value seeds the EMA
        let v = update_ema(ema, 10.0, 3);
        assert!((v - 10.0).abs() < 1e-10);

        ema = Some(v);
        let v = update_ema(ema, 12.0, 3);
        // k = 2/(3+1) = 0.5; EMA = 12*0.5 + 10*0.5 = 11
        assert!((v - 11.0).abs() < 1e-10);
    }

    #[test]
    fn test_ema_series() {
        let data = vec![10.0, 12.0, 11.0, 13.0];
        let result = series(&data, 3);
        assert_eq!(result.len(), 4);
        assert!((result[0] - 10.0).abs() < 1e-10);
    }
}
