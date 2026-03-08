#![allow(dead_code)]
use serde::{Deserialize, Serialize};

/// Authentication response from IG API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IGAuthResponse {
    pub cst: String,
    pub security_token: String,
    pub lightstreamer_endpoint: String,
    pub account_id: String,
}

/// Account information (matches IG API /accounts response)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub account_id: String,
    pub account_name: String,
    pub account_type: String,
    pub status: String,
    pub preferred: bool,
    pub balance: AccountBalance,
    pub currency: String,
}

/// Nested balance object within Account (IG API structure)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountBalance {
    pub balance: f64,
    pub deposit: f64,
    pub profit_loss: f64,
    pub available: f64,
}

/// Accounts response containing list of accounts
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IGAccountsResponse {
    pub accounts: Vec<Account>,
}

/// Market data for a specific instrument
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IGMarketResponse {
    pub instrument: IGInstrument,
    pub snapshot: IGSnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IGInstrument {
    pub epic: String,
    pub name: String,
    #[serde(rename = "type")]
    pub instrument_type: String,
    pub unit: String,
    pub streaming_prices_available: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IGSnapshot {
    pub market_status: String,
    pub bid: Option<f64>,
    pub offer: Option<f64>,
    pub high: Option<f64>,
    pub low: Option<f64>,
    pub percentage_change: Option<f64>,
    pub update_time: String,
}

/// Price history response with nested bid/ask values
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IGPriceHistoryResponse {
    pub prices: Vec<IGPriceSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IGPriceSnapshot {
    pub snapshot_time: String,
    pub open_price: IGPriceValue,
    pub close_price: IGPriceValue,
    pub high_price: IGPriceValue,
    pub low_price: IGPriceValue,
    pub last_traded_volume: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IGPriceValue {
    pub bid: f64,
    pub ask: f64,
}

/// Individual position information
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Position {
    pub deal_id: String,
    pub deal_reference: String,
    pub epic: String,
    pub direction: String,
    pub size: f64,
    pub level: f64,
    pub stop_level: Option<f64>,
    pub limit_level: Option<f64>,
    pub pnl: f64,
    pub currency: String,
    pub created_date: String,
}

/// Positions response containing list of positions
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IGPositionsResponse {
    pub positions: Vec<Position>,
}

/// Trade request for opening a position
///
/// # Limited Risk Account Notes
/// - `guaranteed_stop` MUST be `Some(true)` — the account requires it
/// - `trailing_stop` must be `None` — not available on limited risk
/// - Guaranteed stops have a knock-on premium charged by IG
/// - Stop distance must meet IG's minimum guaranteed stop distance for the instrument
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IGTradeRequest {
    pub epic: String,
    pub direction: String,
    pub size: f64,
    pub order_type: String,
    pub level: Option<f64>,
    pub stop_level: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_distance: Option<f64>,
    pub limit_level: Option<f64>,
    pub currency_code: Option<String>,
    /// REQUIRED for limited risk accounts — must be true
    pub guaranteed_stop: Option<bool>,
    /// NOT available on limited risk accounts — always None
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trailing_stop: Option<bool>,
    pub force_open: Option<bool>,
    pub expiry: String,
}

/// Deal status enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DealStatus {
    #[serde(rename = "ACCEPTED")]
    Accepted,
    #[serde(rename = "REJECTED")]
    Rejected,
    #[serde(rename = "PENDING")]
    Pending,
}

/// Trade response after position is opened
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IGTradeResponse {
    pub deal_reference: String,
    pub deal_status: Option<String>,
    pub reason: Option<String>,
}

/// Affected deal in confirmation
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AffectedDeal {
    pub deal_id: String,
    pub status: String,
}

/// Deal confirmation response
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IGConfirmResponse {
    pub deal_id: String,
    pub deal_reference: String,
    pub deal_status: String,
    pub epic: String,
    pub direction: String,
    pub size: Option<f64>,
    pub level: Option<f64>,
    pub stop_level: Option<f64>,
    pub stop_distance: Option<f64>,
    pub limit_level: Option<f64>,
    pub limit_distance: Option<f64>,
    pub guaranteed_stop: Option<bool>,
    pub reason: Option<String>,
    pub affected_deals: Option<Vec<AffectedDeal>>,
}

/// Request for updating an open position (stops/limits)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IGUpdatePositionRequest {
    pub stop_level: Option<f64>,
    pub limit_level: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_distance: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit_distance: Option<f64>,
    pub guaranteed_stop: Option<bool>,
    pub trailing_stop: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trailing_stop_distance: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trailing_stop_increment: Option<f64>,
}
