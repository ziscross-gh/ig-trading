#![allow(dead_code)]
use anyhow::Result;
use tracing::{info, warn, error, debug};
use serde::{Deserialize, Serialize};
use tokio::time::{sleep, Duration};

use crate::api::rest_client::IGRestClient;
use crate::api::types::{IGTradeRequest, IGConfirmResponse};
use crate::api::traits::TraderAPI;
use crate::risk::AdjustedTrade;
use crate::engine::state::Position;

/// Configuration for order execution behavior
#[derive(Debug, Clone)]
pub struct OrderManagerConfig {
    pub confirm_timeout_ms: u64,
    pub confirm_max_retries: u32,
    /// Limited risk accounts REQUIRE guaranteed stops on every trade.
    /// This must be true for IG limited risk accounts.
    pub guaranteed_stop: bool,
}

/// Execution result from order submission
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    pub deal_id: String,
    pub deal_reference: String,
    pub fill_price: f64,
    pub status: String,
    pub epic: String,
    pub direction: String,
    pub size: f64,
}

/// Result from closing a position
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloseResult {
    pub deal_id: String,
    pub close_price: f64,
    pub pnl: f64,
}

/// Order manager for executing trades and managing position lifecycle
pub struct OrderManager {
    config: OrderManagerConfig,
}

impl OrderManager {
    pub fn new(config: OrderManagerConfig) -> Self {
        Self { config }
    }

    /// Execute a trade by opening a position
    pub async fn execute_trade(
        &self,
        client: &mut IGRestClient,
        trade: &AdjustedTrade,
    ) -> Result<ExecutionResult> {
        info!(
            "Executing trade: {} {} {} @ SL={} TP={}",
            trade.direction, trade.size, trade.epic, trade.stop_loss, trade.take_profit
        );

        // Build the trade request
        // NOTE: Limited risk accounts REQUIRE guaranteed_stop = true.
        // Guaranteed stops have a premium but cap your maximum loss absolutely.
        // Trailing stops are NOT available on limited risk accounts.
        let request = IGTradeRequest {
            epic: trade.epic.clone(),
            direction: trade.direction.clone(),
            size: 3.0, // HARDCODED FOR TESTING
            order_type: "MARKET".to_string(),
            level: None,
            stop_level: Some(trade.stop_loss),
            stop_distance: None,
            limit_level: Some(trade.take_profit),
            currency_code: Some("SGD".to_string()),
            guaranteed_stop: Some(self.config.guaranteed_stop),
            trailing_stop: None, // NOT available on limited risk accounts
            force_open: Some(true),
            expiry: "-".to_string(),
        };

        // Submit the order
        let trade_response = client.open_position(request).await?;

        info!(
            "Trade submitted: deal_reference={}, status={:?}",
            trade_response.deal_reference, trade_response.deal_status
        );

        // Poll for deal confirmation with retries
        let confirm_response = self
            .poll_deal_confirmation(client, &trade_response.deal_reference)
            .await?;

        let execution_result = ExecutionResult {
            deal_id: confirm_response.deal_id.clone(),
            deal_reference: confirm_response.deal_reference.clone(),
            fill_price: confirm_response.level.unwrap_or(0.0),
            status: confirm_response.deal_status.clone(),
            epic: confirm_response.epic.clone(),
            direction: confirm_response.direction.clone(),
            size: confirm_response.size.unwrap_or(0.0),
        };

        info!(
            "Trade execution confirmed: deal_id={}, fill_price={}, status={}",
            execution_result.deal_id, execution_result.fill_price, execution_result.status
        );

        Ok(execution_result)
    }

    /// Update the stop loss for an open position
    pub async fn update_stop_loss(
        &self,
        client: &mut IGRestClient,
        position: &Position,
        new_stop: f64,
    ) -> Result<crate::api::types::IGTradeResponse> {
        use crate::api::types::IGUpdatePositionRequest;

        debug!(
            "Updating stop loss for {}: deal_id={}, old={:?}, new={}",
            position.epic, position.deal_id, position.stop_loss, new_stop
        );

        let request = IGUpdatePositionRequest {
            stop_level: Some(new_stop),
            limit_level: position.take_profit,
            stop_distance: None,
            limit_distance: None,
            guaranteed_stop: Some(self.config.guaranteed_stop),
            trailing_stop: Some(false),
            trailing_stop_distance: None,
            trailing_stop_increment: None,
        };

        client.update_position(&position.deal_id, request).await
    }

    /// Close an open position
    pub async fn close_position(
        &self,
        client: &mut IGRestClient,
        position: &Position,
    ) -> Result<CloseResult> {
        info!(
            "Closing position: deal_id={}, epic={}, size={}, current_price={}",
            position.deal_id, position.epic, position.size, position.current_price
        );

        // Determine direction for close (opposite of open direction)
        let close_direction = match position.direction {
            crate::engine::state::Direction::Buy => "SELL",
            crate::engine::state::Direction::Sell => "BUY",
        };

        // Call close position API with retries for POSITION_NOT_FOUND
        let mut close_response = None;
        let mut retry_count = 0;
        let max_retries = 3;

        while retry_count < max_retries {
            match client.close_position(&position.deal_id, close_direction, position.size).await {
                Ok(resp) => {
                    close_response = Some(resp);
                    break;
                }
                Err(e) if e.to_string().contains("POSITION_NOT_FOUND") && retry_count < max_retries - 1 => {
                    warn!("Position not found yet (replication delay). Retrying close... ({}/{})", retry_count + 1, max_retries);
                    sleep(Duration::from_millis(500)).await;
                    retry_count += 1;
                }
                Err(e) => return Err(e),
            }
        }

        let close_response = close_response.ok_or_else(|| anyhow::anyhow!("Failed to get close response after retries"))?;

        info!(
            "Close submitted: deal_reference={}, status={:?}",
            close_response.deal_reference, close_response.deal_status
        );

        // Poll for confirmation
        let confirm_response = self
            .poll_deal_confirmation(client, &close_response.deal_reference)
            .await?;

        let close_price = confirm_response.level.unwrap_or(0.0);
        let pnl = if position.direction.to_string() == "BUY" {
            (close_price - position.open_price) * position.size
        } else {
            (position.open_price - close_price) * position.size
        };

        let close_result = CloseResult {
            deal_id: confirm_response.deal_id.clone(),
            close_price,
            pnl,
        };

        info!(
            "Position closed: deal_id={}, close_price={}, pnl={}",
            close_result.deal_id, close_result.close_price, close_result.pnl
        );

        Ok(close_result)
    }

    /// Poll for deal confirmation with retries
    async fn poll_deal_confirmation(
        &self,
        client: &mut IGRestClient,
        deal_reference: &str,
    ) -> Result<IGConfirmResponse> {
        let sleep_ms = 500;
        let max_retries = self.config.confirm_max_retries;

        for attempt in 0..max_retries {
            sleep(Duration::from_millis(sleep_ms)).await;

            match client.get_deal_confirmation(deal_reference).await {
                Ok(confirm) => {
                    // Check if confirmation is final
                    if confirm.deal_status == "ACCEPTED" || confirm.deal_status == "REJECTED" {
                        if confirm.deal_status == "ACCEPTED" {
                            return Ok(confirm);
                        } else {
                            let reason = confirm.reason.unwrap_or_else(|| "Unknown".to_string());
                            error!("Deal rejected: {}", reason);
                            return Err(anyhow::anyhow!("Deal rejected: {}", reason));
                        }
                    }
                    // Still PENDING, continue polling
                    info!(
                        "Deal confirmation pending (attempt {}/{}): {}",
                        attempt + 1,
                        max_retries,
                        deal_reference
                    );
                }
                Err(e) => {
                    // Retry on error (could be temporary network issue)
                    warn!(
                        "Error fetching deal confirmation (attempt {}/{}): {}",
                        attempt + 1,
                        max_retries,
                        e
                    );
                }
            }
        }

        error!(
            "Deal confirmation timeout after {} retries: {}",
            max_retries, deal_reference
        );
        Err(anyhow::anyhow!(
            "Deal confirmation timeout after {} retries",
            max_retries
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_order_manager_creation() {
        let config = OrderManagerConfig {
            confirm_timeout_ms: 5000,
            confirm_max_retries: 10,
            guaranteed_stop: true,
        };
        let manager = OrderManager::new(config);
        assert_eq!(manager.config.confirm_max_retries, 10);
        assert!(manager.config.guaranteed_stop);
    }

    #[test]
    fn test_execution_result_serialization() {
        let result = ExecutionResult {
            deal_id: "123".to_string(),
            deal_reference: "ref123".to_string(),
            fill_price: 1234.56,
            status: "ACCEPTED".to_string(),
            epic: "CS.D.EURUSD.CFD".to_string(),
            direction: "BUY".to_string(),
            size: 0.5,
        };

        let json = serde_json::to_string(&result).expect("Serialization failed");
        assert!(json.contains("deal_id"));
        assert!(json.contains("1234.56"));
    }
}
