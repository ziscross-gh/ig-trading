#![allow(dead_code)]
use reqwest::{Client, Response, StatusCode};
use serde::de::DeserializeOwned;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};
use chrono::{DateTime, Utc};

use crate::api::types::*;
use crate::api::traits::TraderAPI;
use async_trait::async_trait;

const DEMO_BASE_URL: &str = "https://demo-api.ig.com/gateway/deal";
const PROD_BASE_URL: &str = "https://api.ig.com/gateway/deal";
const API_VERSION: &str = "2";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Burstable token-bucket rate limiter.
///
/// Tokens refill continuously at `rate_per_minute / 60` per second up to the burst
/// ceiling (`max_tokens = rate_per_minute`). Each API call consumes one token.
/// If no tokens are available the caller sleeps exactly as long as needed to earn one.
///
/// Holding the `Mutex` across the sleep naturally serialises concurrent callers — IG's
/// session-based API must not receive parallel in-flight requests with the same CST token.
struct TokenBucket {
    tokens: f64,
    max_tokens: f64,
    refill_per_sec: f64,
    last_refill: Instant,
}

impl TokenBucket {
    fn new(rate_per_minute: u32) -> Self {
        let max = rate_per_minute as f64;
        Self {
            tokens: max,
            max_tokens: max,
            refill_per_sec: max / 60.0,
            last_refill: Instant::now(),
        }
    }

    async fn acquire(&mut self) {
        // Refill tokens based on elapsed time
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();
        self.tokens = (self.tokens + elapsed * self.refill_per_sec).min(self.max_tokens);
        self.last_refill = now;

        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
        } else {
            // Calculate precise wait to earn one token, then sleep
            let wait_secs = (1.0 - self.tokens) / self.refill_per_sec;
            tokio::time::sleep(Duration::from_secs_f64(wait_secs)).await;
            self.tokens = 0.0;
            self.last_refill = Instant::now();
        }
    }
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EncryptionKeyResponse {
    pub encryption_key: String,
    pub time_stamp: u64,
}

/// IG REST API client for trading operations
pub struct IGRestClient {
    client: Client,
    base_url: String,
    api_key: String,
    cst: Option<String>,
    security_token: Option<String>,
    lightstreamer_endpoint: Option<String>,
    account_id: Option<String>,
    /// Token-bucket rate limiter — enforces `rate_limit_per_minute` from engine config.
    /// Held as a `Mutex` so concurrent callers are serialised (IG requires sequential requests
    /// per session token).
    rate_limiter: Arc<Mutex<TokenBucket>>,
    last_authenticated: Option<DateTime<Utc>>,
    // Credentials stored for auto-reauthentication
    identifier: String,
    password: String,
}

impl IGRestClient {
    /// Create a new IG REST client and authenticate.
    ///
    /// `rate_limit_per_minute` — taken from `config.ig.rate_limit_per_minute` (default 30).
    /// This is the ceiling enforced by the token bucket; IG will return 429 / FORBIDDEN if
    /// exceeded in practice.
    pub async fn new(
        api_key: String,
        identifier: String,
        password: String,
        is_demo: bool,
        rate_limit_per_minute: u32,
    ) -> Result<Self, anyhow::Error> {
        let base_url = if is_demo {
            DEMO_BASE_URL.to_string()
        } else {
            PROD_BASE_URL.to_string()
        };

        let client = Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .build()?;

        let mut ig_client = Self {
            client,
            base_url,
            api_key,
            cst: None,
            security_token: None,
            lightstreamer_endpoint: None,
            account_id: None,
            rate_limiter: Arc::new(Mutex::new(TokenBucket::new(rate_limit_per_minute))),
            last_authenticated: None,
            identifier: identifier.clone(),
            password: password.clone(),
        };

        ig_client.authenticate(&identifier, &password).await?;
        Ok(ig_client)
    }

    /// Authenticate with IG API using identifier and password
    pub async fn authenticate(&mut self, identifier: &str, password: &str) -> Result<(), anyhow::Error> {
        let (password_to_send, is_encrypted) = match self.get_encryption_key().await {
            Ok(enc_resp) => {
                match Self::encrypt_password(password, enc_resp.time_stamp, &enc_resp.encryption_key) {
                    Ok(enc) => (enc, true),
                    Err(e) => {
                        warn!("Failed to encrypt password: {}", e);
                        (password.to_string(), false)
                    }
                }
            }
            Err(e) => {
                warn!("Failed to get encryption key: {}", e);
                (password.to_string(), false)
            }
        };

        let url = format!("{}/session", self.base_url);

        let body = serde_json::json!({
            "identifier": identifier,
            "password": password_to_send,
            "encryptedPassword": is_encrypted
        });

        info!("Authenticating with IG API");

        let response = self
            .client
            .post(&url)
            .header("X-IG-API-KEY", &self.api_key)
            .header("Version", API_VERSION)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        if response.status() != StatusCode::OK {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            error!("Authentication failed with status: {} ({})", status, error_text);
            return Err(anyhow::anyhow!(
                "Authentication failed: {} ({})",
                status,
                error_text
            ));
        }

        // Extract CST and X-SECURITY-TOKEN from response headers
        let cst_val = response.headers().get("cst")
            .ok_or_else(|| anyhow::anyhow!("CST token not found in response headers"))?
            .to_str()?.to_string();
        let sec_val = response.headers().get("x-security-token")
            .ok_or_else(|| anyhow::anyhow!("X-SECURITY-TOKEN not found in response headers"))?
            .to_str()?.to_string();

        // Parse the response body to get lightstreamerEndpoint and currentAccountId
        let body: serde_json::Value = response.json().await?;
        if let Some(ls_endpoint) = body.get("lightstreamerEndpoint").and_then(|v| v.as_str()) {
            self.lightstreamer_endpoint = Some(ls_endpoint.to_string());
            info!("Lightstreamer endpoint: {}", ls_endpoint);
        }
        if let Some(acct_id) = body.get("currentAccountId").and_then(|v| v.as_str()) {
            self.account_id = Some(acct_id.to_string());
            info!("Account ID: {}", acct_id);
        }

        self.cst = Some(cst_val);
        self.security_token = Some(sec_val);
        self.last_authenticated = Some(Utc::now());
        debug!("CST and Security tokens obtained");

        info!("Successfully authenticated with IG API");
        Ok(())
    }

    /// Get the Lightstreamer endpoint URL (available after authentication)
    pub fn lightstreamer_endpoint(&self) -> Option<&str> {
        self.lightstreamer_endpoint.as_deref()
    }

    /// Get the authenticated account ID
    pub fn account_id(&self) -> Option<&str> {
        self.account_id.as_deref()
    }

    /// Get the CST token
    pub fn cst(&self) -> Option<&str> {
        self.cst.as_deref()
    }

    /// Get the security token
    pub fn security_token(&self) -> Option<&str> {
        self.security_token.as_deref()
    }

    /// Fetch RSA encryption key from IG API for secure password login
    pub async fn get_encryption_key(&mut self) -> Result<EncryptionKeyResponse, anyhow::Error> {
        let url = format!("{}/session/encryptionKey", self.base_url);
        self.apply_rate_limit().await;

        let response = self
            .client
            .get(&url)
            .header("X-IG-API-KEY", &self.api_key)
            .header("Version", "1")
            .send()
            .await?;

        self.handle_response::<EncryptionKeyResponse>(response).await
    }

    /// Helper to encrypt the password using the acquired RSA key
    fn encrypt_password(password: &str, timestamp: u64, key_base64: &str) -> Result<String, anyhow::Error> {
        use base64::{Engine as _, engine::general_purpose::STANDARD as b64};
        use rsa::{RsaPublicKey, Pkcs1v15Encrypt};
        use rsa::pkcs8::DecodePublicKey;

        let der = b64.decode(key_base64)?;
        let pub_key = RsaPublicKey::from_public_key_der(&der)?;

        // IG requires base64 encoding the string before RSA encryption
        let raw_input = format!("{}|{}", password, timestamp);
        let input_bytes = b64.encode(raw_input.as_bytes());
        
        let mut rng = rand::rngs::OsRng;
        let enc_data = pub_key.encrypt(&mut rng, Pkcs1v15Encrypt, input_bytes.as_bytes())?;
        
        // Then base64 encode the RSA output
        Ok(b64.encode(&enc_data))
    }
}

#[async_trait]
impl TraderAPI for IGRestClient {
    /// Get list of accounts (requires API Version 1, not the default Version 2)
    async fn get_accounts(&mut self) -> Result<IGAccountsResponse, anyhow::Error> {
        self.apply_rate_limit().await;
        let url = format!("{}/accounts", self.base_url);
        
        // Build manually to avoid build_request adding Version: 2
        let mut request = self.client.get(&url)
            .header("X-IG-API-KEY", &self.api_key)
            .header("Version", "1")
            .header("Content-Type", "application/json; charset=UTF-8")
            .header("Accept", "application/json; charset=UTF-8");
        
        if let Some(cst) = &self.cst {
            request = request.header("CST", cst);
        }
        if let Some(sec) = &self.security_token {
            request = request.header("X-SECURITY-TOKEN", sec);
        }
        
        let response = request.send().await?;
        self.handle_response::<IGAccountsResponse>(response).await
    }

    /// Get market data for a specific instrument
    async fn get_market(&mut self, epic: &str) -> Result<IGMarketResponse, anyhow::Error> {
        let url = format!("{}/markets/{}", self.base_url, epic);
        self.get_request::<IGMarketResponse>(&url).await
    }

    /// Get historical price data for a specific instrument
    async fn get_price_history(
        &mut self,
        epic: &str,
        resolution: &str,
        max: usize,
    ) -> Result<IGPriceHistoryResponse, anyhow::Error> {
        self.apply_rate_limit().await;
        let url = format!(
            "{}/prices/{}/{}/{}",
            self.base_url, epic, resolution, max
        );
        let builder = self.client.get(&url).header("Version", "1");
        let response = self.build_request(builder).send().await?;
        self.handle_response::<IGPriceHistoryResponse>(response).await
    }

    /// Get list of open positions
    async fn get_positions(&mut self) -> Result<IGPositionsResponse, anyhow::Error> {
        let url = format!("{}/positions", self.base_url);
        self.get_request::<IGPositionsResponse>(&url).await
    }

    /// Open a new position
    async fn open_position(&mut self, request: IGTradeRequest) -> Result<IGTradeResponse, anyhow::Error> {
        let url = format!("{}/positions/otc", self.base_url);

        let body = serde_json::to_value(&request)?;

        info!("open_position payload: {}", serde_json::to_string(&body).unwrap_or_default());

        self.post_request::<IGTradeResponse>(&url, body).await
    }

    /// Close an existing position
    async fn close_position(
        &mut self,
        deal_id: &str,
        direction: &str,
        size: f64,
    ) -> Result<IGTradeResponse, anyhow::Error> {
        let url = format!("{}/positions/otc", self.base_url);

        let body = serde_json::json!({
            "dealId": deal_id,
            "direction": direction,
            "size": size,
            "orderType": "MARKET"
        });

        // Use version 1 for positions/otc tunneled via POST
        let mut retry_count = 0;
        loop {
            self.apply_rate_limit().await;
            
            // Build the request manually to ensure all headers are correct
            let mut request = self.client.post(&url)
                .header("X-IG-API-KEY", &self.api_key)
                .header("Version", "1")
                .header("_method", "DELETE")
                .header("Content-Type", "application/json")
                .json(&body);
                
            if let Some(cst) = &self.cst {
                request = request.header("CST", cst);
            }
            if let Some(sec) = &self.security_token {
                request = request.header("X-SECURITY-TOKEN", sec);
            }

            let response = request.send().await?;
            match self.handle_response::<IGTradeResponse>(response).await {
                Ok(data) => return Ok(data),
                Err(e) if e.to_string().contains("UNAUTHORIZED") && retry_count < 1 => {
                    warn!("Session expired. Re-authenticating and retrying...");
                    let id = self.identifier.clone();
                    let pw = self.password.clone();
                    self.authenticate(&id, &pw).await?;
                    retry_count += 1;
                }
                Err(e) => return Err(e),
            }
        }
    }

    /// Retrieve confirmation for a specific deal reference
    async fn get_deal_confirmation(
        &mut self,
        deal_reference: &str,
    ) -> Result<IGConfirmResponse, anyhow::Error> {
        let url = format!("{}/confirms/{}", self.base_url, deal_reference);
        let mut retry_count = 0;
        loop {
            self.apply_rate_limit().await;
            
            let mut request = self.client.get(&url)
                .header("X-IG-API-KEY", &self.api_key)
                .header("Version", "1")
                .header("Content-Type", "application/json")
                .header("Accept", "application/json");
                
            if let Some(cst) = &self.cst {
                request = request.header("CST", cst);
            }
            if let Some(security_token) = &self.security_token {
                request = request.header("X-SECURITY-TOKEN", security_token);
            }
            
            let response = request.send().await?;
            match self.handle_response::<IGConfirmResponse>(response).await {
                Ok(data) => return Ok(data),
                Err(e) if e.to_string().contains("UNAUTHORIZED") && retry_count < 1 => {
                    warn!("Session expired. Re-authenticating and retrying... (confirms)");
                    let id = self.identifier.clone();
                    let pw = self.password.clone();
                    self.authenticate(&id, &pw).await?;
                    retry_count += 1;
                }
                Err(e) => return Err(e),
            }
        }
    }

    /// Update an open position
    async fn update_position(
        &mut self,
        deal_id: &str,
        request: IGUpdatePositionRequest,
    ) -> Result<IGTradeResponse, anyhow::Error> {
        let url = format!("{}/positions/otc/{}", self.base_url, deal_id);
        let body = serde_json::to_value(&request)?;
        self.put_request::<IGTradeResponse>(&url, body).await
    }

    /// Fetch IG crowd sentiment for a market (GET /clientsentiment/{marketId}, Version 1).
    ///
    /// `market_id` is the short IG identifier (e.g. "GOLD", "EURUSD"), not the full epic.
    /// Built manually to set Version: 1 — build_request defaults to Version: 2.
    async fn get_client_sentiment(
        &mut self,
        market_id: &str,
    ) -> Result<IGSentimentResponse, anyhow::Error> {
        self.apply_rate_limit().await;
        let url = format!("{}/clientsentiment/{}", self.base_url, market_id);

        let mut request = self
            .client
            .get(&url)
            .header("X-IG-API-KEY", &self.api_key)
            .header("Version", "1")
            .header("Content-Type", "application/json; charset=UTF-8")
            .header("Accept", "application/json; charset=UTF-8");

        if let Some(cst) = &self.cst {
            request = request.header("CST", cst);
        }
        if let Some(sec) = &self.security_token {
            request = request.header("X-SECURITY-TOKEN", sec);
        }

        let response = request.send().await?;
        self.handle_response::<IGSentimentResponse>(response).await
    }

    /// Fetch account activity with recursive pagination (10.3).
    async fn get_account_activity(
        &mut self,
        from: &str,
        to: &str,
    ) -> Result<Vec<IGActivity>, anyhow::Error> {
        let mut all_activities: Vec<IGActivity> = Vec::new();
        let initial_url = format!(
            "{}/history/activity?from={}&to={}&detailed=true&pageSize=500",
            self.base_url, from, to
        );
        let mut next_url: Option<String> = Some(initial_url);

        while let Some(url) = next_url.take() {
            self.apply_rate_limit().await;
            let mut request = self
                .client
                .get(&url)
                .header("X-IG-API-KEY", &self.api_key)
                .header("Version", "1")
                .header("Content-Type", "application/json; charset=UTF-8");
            if let Some(cst) = &self.cst {
                request = request.header("CST", cst);
            }
            if let Some(sec) = &self.security_token {
                request = request.header("X-SECURITY-TOKEN", sec);
            }
            let response = request.send().await?;
            let page: IGActivityResponse = self.handle_response(response).await?;

            all_activities.extend(page.activities);

            next_url = page.metadata.paging.next.map(|next| {
                if next.starts_with("http") {
                    next
                } else {
                    format!("{}{}", self.base_url.trim_end_matches('/'), next)
                }
            });
        }

        Ok(all_activities)
    }

    /// Find the watchlist named `name` and return its markets (10.4).
    async fn get_watchlist_by_name(
        &mut self,
        name: &str,
    ) -> Result<IGWatchlistMarketsResponse, anyhow::Error> {
        // Step 1: list all watchlists
        let list_url = format!("{}/watchlists", self.base_url);
        self.apply_rate_limit().await;
        let mut request = self
            .client
            .get(&list_url)
            .header("X-IG-API-KEY", &self.api_key)
            .header("Version", "1")
            .header("Content-Type", "application/json; charset=UTF-8");
        if let Some(cst) = &self.cst {
            request = request.header("CST", cst);
        }
        if let Some(sec) = &self.security_token {
            request = request.header("X-SECURITY-TOKEN", sec);
        }
        let response = request.send().await?;
        let list: IGWatchlistListResponse = self.handle_response(response).await?;

        // Step 2: find by name
        let Some(watchlist) = list.watchlists.into_iter().find(|w| w.name == name) else {
            return Ok(IGWatchlistMarketsResponse { markets: vec![] });
        };

        // Step 3: fetch its markets
        let markets_url = format!("{}/watchlists/{}", self.base_url, watchlist.id);
        self.apply_rate_limit().await;
        let mut request = self
            .client
            .get(&markets_url)
            .header("X-IG-API-KEY", &self.api_key)
            .header("Version", "1")
            .header("Content-Type", "application/json; charset=UTF-8");
        if let Some(cst) = &self.cst {
            request = request.header("CST", cst);
        }
        if let Some(sec) = &self.security_token {
            request = request.header("X-SECURITY-TOKEN", sec);
        }
        let response = request.send().await?;
        self.handle_response::<IGWatchlistMarketsResponse>(response).await
    }
}

impl IGRestClient {
    /// Refresh the session to keep it alive (GET /session, Version 1)
    /// Returns fresh CST and X-SECURITY-TOKEN in response headers
    pub async fn refresh_session(&mut self) -> Result<(), anyhow::Error> {
        let url = format!("{}/session", self.base_url);

        info!("Refreshing session");

        // GET /session with Version 1 refreshes tokens
        let mut request = self.client.get(&url)
            .header("X-IG-API-KEY", &self.api_key)
            .header("Version", "1")
            .header("Content-Type", "application/json; charset=UTF-8")
            .header("Accept", "application/json; charset=UTF-8");
        
        if let Some(cst) = &self.cst {
            request = request.header("CST", cst);
        }
        if let Some(sec) = &self.security_token {
            request = request.header("X-SECURITY-TOKEN", sec);
        }

        let response = request.send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            error!("Session refresh failed: {} ({})", status, error_text);
            return Err(anyhow::anyhow!("Session refresh failed: {}", status));
        }

        // Capture rotated tokens if present
        if let Some(cst) = response.headers().get("cst") {
            if let Ok(cst_str) = cst.to_str() {
                self.cst = Some(cst_str.to_string());
                debug!("CST token rotated/refreshed");
            }
        }
        
        if let Some(sec) = response.headers().get("x-security-token") {
            if let Ok(sec_str) = sec.to_str() {
                self.security_token = Some(sec_str.to_string());
                debug!("X-SECURITY-TOKEN rotated/refreshed");
            }
        }

        self.last_authenticated = Some(Utc::now());
        debug!("Session refreshed successfully");
        Ok(())
    }

    /// Fetch today's financing / interest transactions from IG.
    /// IG applies overnight funding once per day (usually just after midnight).
    /// Returns the net SGD financing P&L for today (positive = credit, negative = charge).
    ///
    /// Endpoint: GET /history/transactions?type=INTEREST&from=...&to=...
    /// The `profitAndLoss` field format is e.g. "SD59.65" or "SD-5.84" where "SD" = SGD.
    pub async fn get_today_financing(&mut self) -> Result<f64, anyhow::Error> {
        let now = Utc::now();
        let from = now.format("%Y-%m-%dT00:00:00").to_string();
        let to   = now.format("%Y-%m-%dT23:59:59").to_string();

        let url = format!(
            "{}/history/transactions?type=INTEREST&from={}&to={}&pageSize=500",
            self.base_url, from, to
        );

        self.apply_rate_limit().await;

        let mut request = self.client.get(&url)
            .header("X-IG-API-KEY", &self.api_key)
            .header("Version", "2")
            .header("Content-Type", "application/json; charset=UTF-8")
            .header("Accept", "application/json; charset=UTF-8");

        if let Some(cst) = &self.cst {
            request = request.header("CST", cst);
        }
        if let Some(token) = &self.security_token {
            request = request.header("X-SECURITY-TOKEN", token);
        }

        let response = request
            .timeout(Duration::from_secs(15))
            .send()
            .await?;

        let status = response.status();
        let body: serde_json::Value = response.json().await.unwrap_or_default();

        if !status.is_success() {
            let code = body["errorCode"].as_str().unwrap_or("unknown");
            return Err(anyhow::anyhow!("Financing fetch failed {}: {}", status, code));
        }

        let mut total_financing = 0.0_f64;
        if let Some(txns) = body["transactions"].as_array() {
            for txn in txns {
                if let Some(pnl_str) = txn["profitAndLoss"].as_str() {
                    // Strip currency prefix e.g. "SD59.65" → 59.65, "SD-5.84" → -5.84
                    let numeric: String = pnl_str.chars()
                        .filter(|c| c.is_ascii_digit() || *c == '-' || *c == '.')
                        .collect();
                    if let Ok(val) = numeric.parse::<f64>() {
                        total_financing += val;
                        debug!("Financing txn: {} → {:.2}", pnl_str, val);
                    }
                }
            }
            info!("Financing fetched: {} transactions, net = {:.2} SGD", txns.len(), total_financing);
        }

        Ok(total_financing)
    }

    /// Logout and close the session
    pub async fn logout(&self) -> Result<(), anyhow::Error> {
        let url = format!("{}/session", self.base_url);

        info!("Logging out from IG API");

        let _response = self
            .build_request(self.client.delete(&url))
            .send()
            .await?;

        debug!("Successfully logged out");
        Ok(())
    }

    /// Perform a GET request with rate limiting
    async fn get_request<T: DeserializeOwned>(&mut self, url: &str) -> Result<T, anyhow::Error> {
        let mut retry_count = 0;
        loop {
            self.apply_rate_limit().await;
            let response = self.build_request(self.client.get(url)).send().await?;
            match self.handle_response::<T>(response).await {
                Ok(data) => return Ok(data),
                Err(e) if e.to_string().contains("UNAUTHORIZED") && retry_count < 1 => {
                    warn!("Session expired. Re-authenticating and retrying...");
                    let id = self.identifier.clone();
                    let pw = self.password.clone();
                    self.authenticate(&id, &pw).await?;
                    retry_count += 1;
                }
                Err(e) => return Err(e),
            }
        }
    }

    /// Perform a POST request with rate limiting
    async fn post_request<T: DeserializeOwned>(
        &mut self,
        url: &str,
        body: serde_json::Value,
    ) -> Result<T, anyhow::Error> {
        let mut retry_count = 0;
        loop {
            self.apply_rate_limit().await;
            let response = self.build_request(self.client.post(url).json(&body)).send().await?;
            match self.handle_response::<T>(response).await {
                Ok(data) => return Ok(data),
                Err(e) if e.to_string().contains("UNAUTHORIZED") && retry_count < 1 => {
                    warn!("Session expired. Re-authenticating and retrying...");
                    let id = self.identifier.clone();
                    let pw = self.password.clone();
                    self.authenticate(&id, &pw).await?;
                    retry_count += 1;
                }
                Err(e) => return Err(e),
            }
        }
    }

    /// Perform a PUT request with rate limiting
    async fn put_request<T: DeserializeOwned>(
        &mut self,
        url: &str,
        body: serde_json::Value,
    ) -> Result<T, anyhow::Error> {
        let mut retry_count = 0;
        loop {
            self.apply_rate_limit().await;
            let response = self.build_request(self.client.put(url).json(&body)).send().await?;
            match self.handle_response::<T>(response).await {
                Ok(data) => return Ok(data),
                Err(e) if e.to_string().contains("UNAUTHORIZED") && retry_count < 1 => {
                    warn!("Session expired. Re-authenticating and retrying...");
                    let id = self.identifier.clone();
                    let pw = self.password.clone();
                    self.authenticate(&id, &pw).await?;
                    retry_count += 1;
                }
                Err(e) => return Err(e),
            }
        }
    }

    /// Perform a DELETE request with rate limiting and special _method header
    async fn delete_request<T: DeserializeOwned>(
        &mut self,
        url: &str,
        body: serde_json::Value,
    ) -> Result<T, anyhow::Error> {
        let mut retry_count = 0;
        loop {
            self.apply_rate_limit().await;
            let response = self.build_request(self.client.delete(url).json(&body).header("_method", "DELETE")).send().await?;
            match self.handle_response::<T>(response).await {
                Ok(data) => return Ok(data),
                Err(e) if e.to_string().contains("UNAUTHORIZED") && retry_count < 1 => {
                    warn!("Session expired. Re-authenticating and retrying...");
                    let id = self.identifier.clone();
                    let pw = self.password.clone();
                    self.authenticate(&id, &pw).await?;
                    retry_count += 1;
                }
                Err(e) => return Err(e),
            }
        }
    }

    /// Build a request with common headers
    fn build_request(&self, builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        let mut request = builder
            .header("X-IG-API-KEY", &self.api_key)
            .header("Version", API_VERSION)
            .header("Content-Type", "application/json");

        if let Some(cst) = &self.cst {
            request = request.header("CST", cst);
        }

        if let Some(security_token) = &self.security_token {
            request = request.header("X-SECURITY-TOKEN", security_token);
        }

        request
    }

    /// Handle HTTP response and deserialize JSON.
    /// Uses `IGError` for structured error classification (rate limits, auth, market closed, etc.).
    /// Auth errors always surface as "UNAUTHORIZED" so that retry loops in `get_request` /
    /// `post_request` / etc. can trigger re-authentication without downcasting.
    async fn handle_response<T: DeserializeOwned>(
        &mut self,
        response: Response,
    ) -> Result<T, anyhow::Error> {
        let status = response.status();

        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();

            // Extract IG's machine-readable errorCode from the JSON body if present.
            let ig_code = serde_json::from_str::<serde_json::Value>(&error_text)
                .ok()
                .and_then(|v| v.get("errorCode").and_then(|c| c.as_str()).map(|s| s.to_string()))
                .unwrap_or_default();

            let ig_error = crate::api::errors::IGError::from_ig_code(status, &ig_code, &error_text);
            error!("API request failed with status {}: {} (Code: {})", status, error_text, ig_code);

            // Surface auth failures as the "UNAUTHORIZED" sentinel string so that
            // the retry wrappers (get_request, post_request, etc.) can re-authenticate.
            if ig_error.requires_reauth() {
                return Err(anyhow::anyhow!("UNAUTHORIZED"));
            }

            return Err(anyhow::Error::from(ig_error));
        }

        let body_text = response.text().await?;
        match serde_json::from_str::<T>(&body_text) {
            Ok(data) => Ok(data),
            Err(e) => {
                error!("Failed to decode response body as {}: {}. Body: {}", std::any::type_name::<T>(), e, body_text);
                Err(anyhow::anyhow!("error decoding response body"))
            }
        }
    }

    /// Block until a token is available from the token bucket, then consume it.
    ///
    /// Holding the Mutex across any wait naturally serialises concurrent API calls —
    /// only one request can be in-flight at a time, which is correct for IG's
    /// session-based REST API.
    async fn apply_rate_limit(&self) {
        let mut bucket = self.rate_limiter.lock().await;
        bucket.acquire().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base_urls() {
        assert_eq!(DEMO_BASE_URL, "https://demo-api.ig.com/gateway/deal");
        assert_eq!(PROD_BASE_URL, "https://api.ig.com/gateway/deal");
    }
}
