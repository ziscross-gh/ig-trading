#![allow(dead_code)]
use reqwest::{Client, Response, StatusCode};
use serde::de::DeserializeOwned;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;
use tracing::{debug, error, info, warn};
use chrono::{DateTime, Utc};

use crate::api::types::*;
use crate::api::traits::TraderAPI;
use async_trait::async_trait;

const DEMO_BASE_URL: &str = "https://demo-api.ig.com/gateway/deal";
const PROD_BASE_URL: &str = "https://api.ig.com/gateway/deal";
const API_VERSION: &str = "2";
const RATE_LIMIT_PERMITS: usize = 100;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

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
    rate_limiter: Arc<Semaphore>,
    last_authenticated: Option<DateTime<Utc>>,
    // Credentials stored for auto-reauthentication
    identifier: String,
    password: String,
}

impl IGRestClient {
    /// Create a new IG REST client and authenticate
    pub async fn new(
        api_key: String,
        identifier: String,
        password: String,
        is_demo: bool,
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
            rate_limiter: Arc::new(Semaphore::new(RATE_LIMIT_PERMITS)),
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

        let body = serde_json::json!({
            "epic": request.epic,
            "direction": request.direction,
            "size": request.size,
            "orderType": request.order_type,
            "level": request.level,
            "stopLevel": request.stop_level,
            "limitLevel": request.limit_level,
            "currencyCode": request.currency_code,
            "guaranteedStop": request.guaranteed_stop.unwrap_or(false),
            "forceOpen": request.force_open.unwrap_or(false),
            "expiry": request.expiry
        });

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
        let url = format!("{}/positions/otc/{}", self.base_url, deal_id);

        let body = serde_json::json!({
            "direction": direction,
            "size": size
        });

        self.delete_request::<IGTradeResponse>(&url, body).await
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

    /// Handle HTTP response and deserialize JSON
    async fn handle_response<T: DeserializeOwned>(
        &mut self,
        response: Response,
    ) -> Result<T, anyhow::Error> {
        let status = response.status();

        if status == StatusCode::UNAUTHORIZED {
            warn!("Unauthorized (401) received.");
            return Err(anyhow::anyhow!("UNAUTHORIZED"));
        }

        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            error!("API request failed with status {}: {}", status, error_text);
            return Err(anyhow::anyhow!(
                "API request failed with status {}: {}",
                status,
                error_text
            ));
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

    /// Apply rate limiting using semaphore
    async fn apply_rate_limit(&self) {
        let _permit = self.rate_limiter.acquire().await;
        // Permit is automatically released when dropped
        // In a production system, you might want to add a delay here
        tokio::time::sleep(Duration::from_millis(10)).await;
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
