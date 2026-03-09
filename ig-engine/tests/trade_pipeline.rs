use ig_engine::api::mock_client::MockTraderClient;
use ig_engine::api::traits::TraderAPI;
use ig_engine::api::types::IGTradeRequest;

#[tokio::test]
async fn test_mock_trade_execution() {
    let mut mock = MockTraderClient::new(10000.0);
    
    // 1. Check initial state
    let accounts = mock.get_accounts().await.expect("Failed to get accounts");
    assert_eq!(accounts.accounts[0].balance.balance, 10000.0);
    
    // 2. Open a "Buy" position
    let request = IGTradeRequest {
        epic: "CS.D.EURUSD.CFD.IP".to_string(),
        direction: "BUY".to_string(),
        size: 1.0,
        order_type: "MARKET".to_string(),
        level: Some(1.18),
        stop_level: Some(1.17),
        stop_distance: None,
        limit_level: Some(1.20),
        currency_code: Some("USD".to_string()),
        expiry: "DFB".to_string(),
        guaranteed_stop: Some(false),
        trailing_stop: Some(false),
        force_open: Some(false),
    };
    
    let resp = mock.open_position(request).await.expect("Failed to open mock position");
    assert!(resp.deal_reference.len() > 0);
    
    // 3. Verify position exists in mock state
    let positions = mock.get_positions().await.expect("Failed to get positions");
    assert_eq!(positions.positions.len(), 1);
    assert_eq!(positions.positions[0].market.epic, "CS.D.EURUSD.CFD.IP");

    // 4. Close the position
    let deal_id = &positions.positions[0].position.deal_id;
    mock.close_position(deal_id, "SELL", 1.0).await.expect("Failed to close mock position");
    
    // 5. Verify position is gone
    let final_positions = mock.get_positions().await.expect("Failed to get final positions");
    assert_eq!(final_positions.positions.len(), 0);
}
