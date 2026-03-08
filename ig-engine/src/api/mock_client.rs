use async_trait::async_trait;
use crate::api::types::*;
use crate::api::traits::TraderAPI;
use std::collections::HashMap;
use chrono::Utc;
use uuid::Uuid;

pub struct MockTraderClient {
    pub positions: HashMap<String, Position>,
    pub account_balance: f64,
    pub next_prices: HashMap<String, f64>,
}

impl MockTraderClient {
    pub fn new(initial_balance: f64) -> Self {
        Self {
            positions: HashMap::new(),
            account_balance: initial_balance,
            next_prices: HashMap::new(),
        }
    }

    pub fn set_price(&mut self, epic: &str, price: f64) {
        self.next_prices.insert(epic.to_string(), price);
    }
}

#[async_trait]
impl TraderAPI for MockTraderClient {
    async fn get_accounts(&mut self) -> Result<IGAccountsResponse, anyhow::Error> {
        Ok(IGAccountsResponse {
            accounts: vec![Account {
                account_id: "MOCK_ACCOUNT".to_string(),
                account_name: "Mock Account".to_string(),
                account_type: "CFD".to_string(),
                status: "ENABLED".to_string(),
                preferred: true,
                balance: AccountBalance {
                    balance: self.account_balance,
                    deposit: self.account_balance,
                    profit_loss: 0.0,
                    available: self.account_balance,
                },
                currency: "USD".to_string(),
            }],
        })
    }

    async fn get_market(&mut self, epic: &str) -> Result<IGMarketResponse, anyhow::Error> {
        let price = self.next_prices.get(epic).cloned().unwrap_or(1.18);
        Ok(IGMarketResponse {
            instrument: IGInstrument {
                epic: epic.to_string(),
                name: format!("Mock {}", epic),
                instrument_type: "CURRENCIES".to_string(),
                unit: "SHARE".to_string(),
                streaming_prices_available: true,
            },
            snapshot: IGSnapshot {
                market_status: "TRADEABLE".to_string(),
                bid: Some(price - 0.0001),
                offer: Some(price + 0.0001),
                high: Some(price + 0.01),
                low: Some(price - 0.01),
                percentage_change: Some(0.0),
                update_time: Utc::now().to_rfc3339(),
            },
        })
    }

    async fn get_price_history(
        &mut self,
        _epic: &str,
        _resolution: &str,
        max: usize,
    ) -> Result<IGPriceHistoryResponse, anyhow::Error> {
        let mut prices = Vec::new();
        let now = Utc::now();
        for i in 0..max {
            let time = now - chrono::Duration::hours(i as i64);
            prices.push(IGPriceSnapshot {
                snapshot_time: time.to_rfc3339(),
                open_price: IGPriceValue { bid: 1.18, ask: 1.1802 },
                close_price: IGPriceValue { bid: 1.181, ask: 1.1812 },
                high_price: IGPriceValue { bid: 1.182, ask: 1.1822 },
                low_price: IGPriceValue { bid: 1.179, ask: 1.1792 },
                last_traded_volume: Some(100.0),
            });
        }
        Ok(IGPriceHistoryResponse { prices })
    }

    async fn get_positions(&mut self) -> Result<IGPositionsResponse, anyhow::Error> {
        let positions: Vec<Position> = self.positions.values().cloned().collect();
        Ok(IGPositionsResponse { positions })
    }

    async fn open_position(&mut self, request: IGTradeRequest) -> Result<IGTradeResponse, anyhow::Error> {
        let deal_reference = Uuid::new_v4().to_string();
        let deal_id = format!("DIA_{}", deal_reference);
        
        let position = Position {
            deal_id: deal_id.clone(),
            deal_reference: deal_reference.clone(),
            epic: request.epic.clone(),
            direction: request.direction.clone(),
            size: request.size,
            level: request.level.unwrap_or(1.18),
            limit_level: request.limit_level,
            stop_level: request.stop_level,
            created_date: Utc::now().to_rfc3339(),
            currency: request.currency_code.clone().unwrap_or("USD".to_string()),
            pnl: 0.0,
        };

        self.positions.insert(deal_id.clone(), position);

        Ok(IGTradeResponse {
            deal_reference,
            deal_status: Some("ACCEPTED".to_string()),
            reason: None,
        })
    }

    async fn close_position(
        &mut self,
        deal_id: &str,
        _direction: &str,
        _size: f64,
    ) -> Result<IGTradeResponse, anyhow::Error> {
        if self.positions.remove(deal_id).is_some() {
            Ok(IGTradeResponse {
                deal_reference: Uuid::new_v4().to_string(),
                deal_status: Some("ACCEPTED".to_string()),
                reason: None,
            })
        } else {
            Err(anyhow::anyhow!("POSITION_NOT_FOUND"))
        }
    }

    async fn update_position(
        &mut self,
        _deal_id: &str,
        _request: IGUpdatePositionRequest,
    ) -> Result<IGTradeResponse, anyhow::Error> {
        Ok(IGTradeResponse {
            deal_reference: Uuid::new_v4().to_string(),
            deal_status: Some("ACCEPTED".to_string()),
            reason: None,
        })
    }
    
    async fn get_deal_confirmation(
        &mut self,
        deal_reference: &str,
    ) -> Result<IGConfirmResponse, anyhow::Error> {
        Ok(IGConfirmResponse {
            deal_id: format!("DIA_{}", deal_reference),
            deal_reference: deal_reference.to_string(),
            deal_status: "ACCEPTED".to_string(),
            epic: "MOCK_EPIC".to_string(),
            direction: "BUY".to_string(),
            size: Some(1.0),
            level: Some(1.18),
            stop_level: None,
            stop_distance: None,
            limit_level: None,
            limit_distance: None,
            guaranteed_stop: Some(false),
            reason: Some("SUCCESS".to_string()),
            affected_deals: None,
        })
    }
}
