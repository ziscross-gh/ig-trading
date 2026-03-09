use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use axum::{
    body::Body,
    http::{Request, StatusCode},
    response::Response,
};
use tower::util::ServiceExt; 
use serde_json::json;
use chrono::Utc;

use ig_engine::engine::state::{EngineState};
use ig_engine::engine::config::EngineConfig;
use ig_engine::ipc::http_server::AppState;
use ig_engine::indicators::Candle;

#[tokio::test]
async fn test_backtest_endpoint() {
    // 1. Setup EngineState with mock data
    let mut config = EngineConfig::default();
    config.markets.epics = vec!["CS.D.EURUSD.CSD.IP".to_string()];
    
    let engine_state = Arc::new(RwLock::new(EngineState::new(config)));
    let (event_tx, _) = broadcast::channel(100);
    
    // Populate with candles
    {
        let mut s = engine_state.write().await;
        let epic = "CS.D.EURUSD.CSD.IP";
        let mut candles = Vec::new();
        let mut price = 1.1000;
        for i in 0..100 {
            price += 0.0001;
            candles.push(Candle {
                timestamp: Utc::now().timestamp() - (100 - i) * 3600,
                open: price,
                high: price + 0.0005,
                low: price - 0.0005,
                close: price + 0.0001,
                volume: 1000,
            });
        }
        s.markets.history.warm_up(epic, "HOUR", candles);
    }

    let app_state = AppState {
        engine_state,
        event_tx,
        last_optimization_result: Arc::new(RwLock::new(None)),
    };

    // 2. Setup Router
    let app = axum::Router::new()
        .route("/api/backtest", axum::routing::post(ig_engine::ipc::http_server::post_backtest))
        .with_state(app_state);

    // 3. Make Request
    let response: Response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/backtest")
                .header("Content-Type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "epic": "CS.D.EURUSD.CSD.IP",
                        "strategy_name": "ma_crossover",
                        "initial_balance": 10000.0,
                        "risk_pct": 1.0
                    }))
                    .expect("JSON serialization failed"),
                ))
                .expect("Request builder failed"),
        )
        .await
        .expect("Oneshot call failed");

    // 4. Assert
    assert_eq!(response.status(), StatusCode::OK);
    
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.expect("Failed to read body");
    let body: serde_json::Value = serde_json::from_slice(&body).expect("Failed to parse body JSON");
    
    assert_eq!(body["success"], true);
    assert_eq!(body["epic"], "CS.D.EURUSD.CSD.IP");
    assert_eq!(body["strategy"], "ma_crossover");
    assert!(body["result"]["total_trades"].as_u64().is_some());
}

#[tokio::test]
async fn test_backtest_endpoint_filtering() {
    // 1. Setup EngineState with mock data
    let mut config = EngineConfig::default();
    config.markets.epics = vec!["CS.D.EURUSD.CSD.IP".to_string()];
    
    let engine_state = Arc::new(RwLock::new(EngineState::new(config)));
    let (event_tx, _) = broadcast::channel(100);
    
    let base_ts = 1700000000;
    
    // Populate with 100 candles, one per hour
    {
        let mut s = engine_state.write().await;
        let epic = "CS.D.EURUSD.CSD.IP";
        let mut candles = Vec::new();
        for i in 0..100 {
            candles.push(Candle {
                timestamp: base_ts + (i * 3600),
                open: 1.1000,
                high: 1.1005,
                low: 1.0995,
                close: 1.1001,
                volume: 1000,
            });
        }
        s.markets.history.warm_up(epic, "HOUR", candles);
    }

    let app_state = AppState {
        engine_state,
        event_tx,
        last_optimization_result: Arc::new(RwLock::new(None)),
    };

    let app = axum::Router::new()
        .route("/api/backtest", axum::routing::post(ig_engine::ipc::http_server::post_backtest))
        .with_state(app_state);

    // Request with range that should only include 60 candles (from 20 to 79)
    let response: Response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/backtest")
                .header("Content-Type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "epic": "CS.D.EURUSD.CSD.IP",
                        "strategy_name": "ma_crossover",
                        "initial_balance": 10000.0,
                        "risk_pct": 1.0,
                        "from": base_ts + (20 * 3600),
                        "to": base_ts + (79 * 3600)
                    }))
                    .expect("JSON serialization failed"),
                ))
                .expect("Request builder failed"),
        )
        .await
        .expect("Oneshot call failed");

    assert_eq!(response.status(), StatusCode::OK);
    
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.expect("Failed to read body");
    let body: serde_json::Value = serde_json::from_slice(&body).expect("Failed to parse body JSON");
    
    assert_eq!(body["success"], true);
    assert_eq!(body["candle_count"], 60);
}
