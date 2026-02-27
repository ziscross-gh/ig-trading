use async_trait::async_trait;
use crate::api::types::*;

/// Unified error type for the trading engine
#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("Authentication failed: {0}")]
    AuthError(String),
    #[error("API Request failed: {status} - {message}")]
    ApiError { status: u16, message: String },
    #[error("Network error: {0}")]
    NetworkError(#[from] reqwest::Error),
    #[error("Data parsing error: {0}")]
    ParseError(#[from] serde_json::Error),
    #[error("Engine state error: {0}")]
    StateError(String),
    #[error("Internal error: {0}")]
    Internal(#[from] anyhow::Error),
}

/// Abstract interface for trading operations, enabling swappable backends (IG, Backtester, etc.)
#[async_trait]
pub trait TraderAPI: Send + Sync {
    /// Fetch all accounts and their current balances
    async fn get_accounts(&mut self) -> Result<IGAccountsResponse, anyhow::Error>;
    
    /// Get static instrument information (min sizes, pip values)
    async fn get_market(&mut self, epic: &str) -> Result<IGMarketResponse, anyhow::Error>;
    
    /// Retrieve historical OHLCV data
    async fn get_price_history(
        &mut self,
        epic: &str,
        resolution: &str,
        max: usize,
    ) -> Result<IGPriceHistoryResponse, anyhow::Error>;
    
    /// Fetch current open positions from the broker
    async fn get_positions(&mut self) -> Result<IGPositionsResponse, anyhow::Error>;
    
    /// Submit a new trade request (market or limit)
    async fn open_position(&mut self, request: IGTradeRequest) -> Result<IGTradeResponse, anyhow::Error>;
    
    /// Close an existing position via deal ID
    async fn close_position(
        &mut self,
        deal_id: &str,
        direction: &str,
        size: f64,
    ) -> Result<IGTradeResponse, anyhow::Error>;
    
    /// Retrieve confirmation for a pending deal reference
    async fn get_deal_confirmation(
        &mut self,
        deal_reference: &str,
    ) -> Result<IGConfirmResponse, anyhow::Error>;

    /// Update an open position's stop or limit levels
    async fn update_position(
        &mut self,
        deal_id: &str,
        request: IGUpdatePositionRequest,
    ) -> Result<IGTradeResponse, anyhow::Error>;
}
