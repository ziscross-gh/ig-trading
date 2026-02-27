#![allow(dead_code)]

/// Simple Moving Average
pub fn calculate(data: &[f64], period: usize) -> f64 {
    if data.len() < period || period == 0 {
        return 0.0;
    }
    let slice = &data[data.len() - period..];
    slice.iter().sum::<f64>() / period as f64
}

/// SMA series from data
pub fn series(data: &[f64], period: usize) -> Vec<f64> {
    if data.len() < period {
        return vec![];
    }
    let mut result = Vec::with_capacity(data.len() - period + 1);
    let mut sum: f64 = data[..period].iter().sum();
    result.push(sum / period as f64);

    for i in period..data.len() {
        sum += data[i] - data[i - period];
        result.push(sum / period as f64);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sma_basic() {
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        assert!((calculate(&data, 3) - 4.0).abs() < 1e-10);
        assert!((calculate(&data, 5) - 3.0).abs() < 1e-10);
    }

    #[test]
    fn test_sma_series() {
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let result = series(&data, 3);
        assert_eq!(result.len(), 3);
        assert!((result[0] - 2.0).abs() < 1e-10);
        assert!((result[1] - 3.0).abs() < 1e-10);
        assert!((result[2] - 4.0).abs() < 1e-10);
    }
}
