use tracing::debug;

use super::{InstrumentSpec, SizingMethod};




/// Calculate position size based on account and risk parameters
///
/// Formula: size = risk_amount / (stop_distance_in_pips × pip_value_per_unit)
///
/// # Arguments
///
/// * `account_balance` - Current account balance in base currency
/// * `risk_pct` - Risk percentage per trade (e.g., 2.0 for 2%)
/// * `entry_price` - Entry price of the trade
/// * `stop_loss_price` - Stop loss price
/// * `epic` - IG Markets epic code for the instrument
///
/// # Returns
///
/// Position size in lots/units
pub fn calculate_position_size(
    account_balance: f64,
    risk_pct: f64,
    entry_price: f64,
    stop_loss_price: f64,
    epic: &str,
    instrument_specs: &std::collections::HashMap<String, InstrumentSpec>,
) -> f64 {
    debug!(
        "Calculating position size: balance={}, risk_pct={}, entry={}, stop={}",
        account_balance, risk_pct, entry_price, stop_loss_price
    );

    // Get instrument spec: Config -> Fallback -> Default
    let instrument = instrument_specs.get(epic).cloned().or_else(|| InstrumentSpec::from_epic_fallback(epic)).unwrap_or_else(|| InstrumentSpec {
        epic: epic.to_string(),
        min_deal_size: 0.1,
        max_deal_size: 100.0,
        pip_value: 10.0,
        pip_scale: 0.0001,
        contract_size: 1.0,
        margin_requirement_pct: 2.0,
        size_decimals: 2,
        min_guaranteed_stop_pips: 10.0,
    });

    // Calculate risk amount in account currency
    let risk_amount = account_balance * risk_pct / 100.0;

    // Calculate stop distance in absolute points
    let stop_distance_in_points = (entry_price - stop_loss_price).abs();

    // Prevent division by zero if stop and entry are identical
    if stop_distance_in_points <= 0.0 {
        return 0.0;
    }

    // Convert points to actual pips using the instrument's scale
    let stop_distance_in_pips = stop_distance_in_points / instrument.pip_scale;

    // Calculate position size using formula:
    // size = risk_amount / (stop_distance_in_pips × pip_value_per_unit)
    let raw_size = risk_amount / (stop_distance_in_pips * instrument.pip_value);

    // Apply decimal rounding for the IG API (e.g. 2 decimal places = factor 100.0)
    let factor = 10_f64.powi(instrument.size_decimals as i32);
    let rounded_size = (raw_size * factor).floor() / factor;

    debug!(
        "Position size calculation: risk_amount={:.2}, pip_scale={}, stop_distance_pips={:.2}, raw_size={:.4}, rounded={}",
        risk_amount, instrument.pip_scale, stop_distance_in_pips, raw_size, rounded_size
    );

    rounded_size
}

/// Calculate Kelly fraction for position sizing
///
/// Kelly Criterion: f* = (bp - q) / b
/// where:
/// - b = ratio of win amount to loss amount (avg_win / avg_loss)
/// - p = probability of winning (win_rate)
/// - q = probability of losing (1 - win_rate)
///
/// # Arguments
///
/// * `win_rate` - Win rate as decimal (e.g., 0.55 for 55%)
/// * `avg_win` - Average winning trade size
/// * `avg_loss` - Average losing trade size (positive value)
///
/// # Returns
///
/// Kelly fraction as decimal (e.g., 0.25 for 25%)
#[allow(dead_code)]
pub fn kelly_fraction(win_rate: f64, avg_win: f64, avg_loss: f64) -> f64 {
    if avg_loss <= 0.0 || win_rate <= 0.0 || win_rate >= 1.0 {
        return 0.0;
    }

    let loss_rate = 1.0 - win_rate;
    let b = avg_win / avg_loss;

    let kelly = (b * win_rate - loss_rate) / b;

    // Ensure kelly is between 0 and 1
    kelly.clamp(0.0, 1.0)
}

/// Apply sizing method to raw position size
///
/// # Arguments
///
/// * `raw_size` - Raw calculated position size
/// * `method` - Sizing method (Fixed, HalfKelly, QuarterKelly)
/// * `kelly_f` - Kelly fraction (used for kelly-based methods)
///
/// # Returns
///
/// Adjusted position size
#[allow(dead_code)]
pub fn apply_sizing_method(raw_size: f64, method: SizingMethod, kelly_f: f64) -> f64 {
    match method {
        SizingMethod::Fixed | SizingMethod::FixedFractional => {
            debug!("Applying Fixed/Fractional sizing method: {:.4}", raw_size);
            raw_size
        }
        SizingMethod::HalfKelly => {
            let adjusted = raw_size * (kelly_f / 2.0);
            debug!(
                "Applying HalfKelly sizing: raw={:.4}, kelly_f={:.4}, adjusted={:.4}",
                raw_size, kelly_f, adjusted
            );
            adjusted
        }
        SizingMethod::QuarterKelly => {
            let adjusted = raw_size * (kelly_f / 4.0);
            debug!(
                "Applying QuarterKelly sizing: raw={:.4}, kelly_f={:.4}, adjusted={:.4}",
                raw_size, kelly_f, adjusted
            );
            adjusted
        }
    }
}

/// Clamp position size to instrument limits
///
/// # Arguments
///
/// * `size` - Position size to clamp
/// * `spec` - Instrument specification with min/max limits
///
/// # Returns
///
/// Clamped position size within instrument limits
#[allow(dead_code)]
pub fn clamp_to_instrument_limits(size: f64, spec: &InstrumentSpec) -> f64 {
    size.max(spec.min_deal_size).min(spec.max_deal_size)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_instrument_spec_gold() {
        let spec = InstrumentSpec::from_epic_fallback("CS.D.CFIGOLD.CFI.IP").expect("Gold spec should exist");
        assert_eq!(spec.pip_value, 1.0);       // SGD$1 per point
        assert_eq!(spec.pip_scale, 1.0);       // 1 point = $1/Troy Ounce
        assert_eq!(spec.min_deal_size, 3.0);  // IG verified minimum
        assert_eq!(spec.max_deal_size, 100.0);
    }

    #[test]
    fn test_instrument_spec_eurusd() {
        let spec = InstrumentSpec::from_epic_fallback("CS.D.EURUSD.CFD").expect("EURUSD spec should exist");
        assert_eq!(spec.pip_value, 1.27);      // USD 1 ≈ SGD$1.27 (IG verified)
        assert_eq!(spec.min_deal_size, 0.02);  // IG verified minimum
    }

    #[test]
    fn test_calculate_position_size() {
        let empty_specs = std::collections::HashMap::new();
        let size = calculate_position_size(
            10000.0,
            2.0, // $200 risk
            1.1000,
            1.0950, // 50 pips stop loss distance
            "CS.D.EURUSD.CSD.IP",
            &empty_specs,
        );
        
        // pip_value = 1.27 (IG verified SGD)
        // 200 risk / (50 pips * 1.27 pip_val) = 3.149...
        // floor to 2 decimals = 3.14
        assert_eq!(size, 3.14);
    }

    #[test]
    fn test_kelly_fraction_valid() {
        let kelly = kelly_fraction(0.6, 2.0, 1.0);
        assert!(kelly > 0.0);
        assert!(kelly <= 1.0);
        // (2 * 0.6 - 0.4) / 2 = 0.8 / 2 = 0.4
        assert!((kelly - 0.4).abs() < 0.001);
    }

    #[test]
    fn test_kelly_fraction_edge_cases() {
        // Invalid win rate
        assert_eq!(kelly_fraction(0.0, 2.0, 1.0), 0.0);
        assert_eq!(kelly_fraction(1.0, 2.0, 1.0), 0.0);
        assert_eq!(kelly_fraction(1.5, 2.0, 1.0), 0.0);

        // Invalid loss amount
        assert_eq!(kelly_fraction(0.5, 2.0, 0.0), 0.0);
        assert_eq!(kelly_fraction(0.5, 2.0, -1.0), 0.0);
    }

    #[test]
    fn test_apply_sizing_method_fixed() {
        let adjusted = apply_sizing_method(1.0, SizingMethod::Fixed, 0.25);
        assert_eq!(adjusted, 1.0);
    }

    #[test]
    fn test_apply_sizing_method_half_kelly() {
        let adjusted = apply_sizing_method(1.0, SizingMethod::HalfKelly, 0.4);
        assert!((adjusted - 0.2).abs() < 0.001);
    }

    #[test]
    fn test_apply_sizing_method_quarter_kelly() {
        let adjusted = apply_sizing_method(1.0, SizingMethod::QuarterKelly, 0.4);
        assert!((adjusted - 0.1).abs() < 0.001);
    }

    #[test]
    fn test_clamp_to_instrument_limits() {
        let spec = InstrumentSpec::from_epic_fallback("CS.D.EURUSD.CFD").expect("EURUSD spec should exist");

        // Clamp high
        let clamped = clamp_to_instrument_limits(150.0, &spec);
        assert_eq!(clamped, 100.0);

        // Clamp low
        let clamped = clamp_to_instrument_limits(0.005, &spec);
        assert_eq!(clamped, 0.02);

        // Within limits
        let clamped = clamp_to_instrument_limits(1.5, &spec);
        assert_eq!(clamped, 1.5);
    }

    #[test]
    fn test_position_sizing_usdjpy() {
        let empty_specs = std::collections::HashMap::new();
        // Test position sizing for JPY pairs (different pip calculation)
        let size = calculate_position_size(
            10000.0,
            1.0, // $100 risk
            150.00,
            149.00, // 100 pips
            "CS.D.USDJPY.CSD.IP",
            &empty_specs,
        );
        
        // pips = 1.00 / 0.01 = 100
        // pip_value = 0.81 (JPY 100 ≈ SGD$0.81, IG verified)
        // risk = 100 -> size = 100 / (100 * 0.81) = 1.234... -> floor to 1.23
        assert_eq!(size, 1.23);
    }
}
