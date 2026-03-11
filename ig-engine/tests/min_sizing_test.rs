use ig_engine::risk::{RiskManager, RiskConfig, AccountInfo, RiskVerdict};

#[test]
fn test_minimum_sizing_logic() {
    let mut config = RiskConfig::default();
    config.max_open_positions = 10;
    config.max_risk_per_trade = 0.5; // 0.5% risk
    config.trading_hours_utc = None; // Disable for test
    config.macro_events = vec![];    // Disable for test
    
    let mut rm = RiskManager::new(config);
    
    // Simulate a SMALL account: $1,000 SGD
    let account = AccountInfo {
        balance: 1000.0,
        equity: 1000.0,
        available_margin: 1000.0,
    };

    println!("\n--- Minimum Sizing Test ($1,000 Balance, 0.5% Risk = $5 Max Risk) ---");

    // 1. Test EUR/USD
    // Entry 1.1000, SL 1.0950 (50 pips risk)
    // Risk $5 / (50 pips * 1.25 pip_value) = 0.08 units
    let verdict_eur = rm.check_trade(
        "CS.D.EURUSD.CSD.IP",
        "buy",
        1.1000,
        1.0950,
        1.1150,
        None,
        &account,
        &[],
        "test",
    );

    if let RiskVerdict::Approved(trade) = verdict_eur {
        println!("✅ EUR/USD Approved: Size={:.2} (Min=0.02)", trade.size);
        assert!(trade.size >= 0.02);
    } else {
        println!("❌ EUR/USD Rejected: {:?}", verdict_eur);
    }

    // 2. Test GOLD
    // Entry 2000.0, SL 1990.0 (10 points risk)
    // Risk $5 / (10 points * 1.0 pip_value) = 0.5 units
    // Min size is 3.0. Should clamp to 3.0 or reject for too much risk.
    let verdict_gold = rm.check_trade(
        "CS.D.CFIGOLD.CFI.IP",
        "buy",
        2000.0,
        1990.0,
        2020.0,
        None,
        &account,
        &[],
        "test",
    );

    match verdict_gold {
        RiskVerdict::Approved(trade) => {
            println!("⚠️  GOLD Approved: Size={:.2} (Min=3.0) - NOTE: This exceeds 0.5% risk due to min sizing!", trade.size);
        },
        RiskVerdict::Rejected(reason) => {
            println!("✅ GOLD Correctly Rejected for small account: {}", reason);
        }
    }
}
