use ig_engine::risk::{RiskManager, RiskConfig, AccountInfo, RiskVerdict};
use chrono::Utc;
use std::fs;

#[test]
fn test_live_calendar_blackout() {
    let mut config = RiskConfig::default();
    config.max_open_positions = 10;
    config.max_risk_per_trade = 0.5;
    config.trading_hours_utc = None; // Disable hours for this test
    
    let mut rm = RiskManager::new(config);
    
    let account = AccountInfo {
        balance: 10000.0,
        equity: 10000.0,
        available_margin: 10000.0,
    };

    let now = Utc::now();
    let now_str = now.to_rfc3339();
    
    // Create a temporary live calendar file with an event happening NOW
    let calendar_json = serde_json::json!({
        "fetched_at": now_str,
        "events": [
            {
                "datetime_utc": now_str,
                "title": "TEST HIGH IMPACT EVENT",
                "country": "USD",
                "impact": "High",
                "blackout_mins": 30
            }
        ]
    });

    // Ensure data directory exists
    let _ = fs::create_dir_all("data");
    fs::write("data/economic_calendar.json", serde_json::to_string(&calendar_json).unwrap())
        .expect("Failed to write test calendar");

    // Attempt a trade during the blackout
    let verdict = rm.check_trade(
        "CS.D.EURUSD.CSD.IP",
        "buy",
        1.1000,
        1.0900,
        1.1100,
        None,
        &account,
        &[],
        "test",
    );

    match verdict {
        RiskVerdict::Rejected(reason) => {
            println!("✅ Correctly rejected by live calendar: {}", reason);
            assert!(reason.contains("Live calendar blackout"));
        },
        RiskVerdict::Approved(_) => {
            panic!("❌ Trade should have been rejected by live calendar blackout!");
        }
    }

    // Cleanup: remove the test calendar file
    let _ = fs::remove_file("data/economic_calendar.json");
}
