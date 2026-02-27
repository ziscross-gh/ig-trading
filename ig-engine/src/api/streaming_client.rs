#![allow(dead_code)]
use std::sync::Arc;
use tokio::sync::{Notify, RwLock, broadcast, mpsc};
use tracing::{info, error};

use lightstreamer_client::ls_client::{LightstreamerClient, LogType, Transport};
use lightstreamer_client::subscription::{Subscription, SubscriptionMode, Snapshot};
use lightstreamer_client::subscription_listener::SubscriptionListener;
use lightstreamer_client::item_update::ItemUpdate;

use crate::engine::state::{EngineState, MarketState};
use crate::ipc::events::EngineEvent;

/// Internal event for the state worker
pub enum StateUpdate {
    Market(MarketState),
    Account(serde_json::Value),
    Trade(serde_json::Value),
}

/// IG Lightstreamer streaming price data fields
const PRICE_FIELDS: &[&str] = &[
    "BID",
    "OFFER",
    "HIGH",
    "LOW",
    "CHANGE",
    "CHANGE_PCT",
    "MARKET_STATE",
    "UPDATE_TIME",
];

/// A listener that updates the engine state and broadcasts events
pub struct StreamingListener {
    pub name: String,
    pub tx: mpsc::Sender<StateUpdate>,
}

impl SubscriptionListener for StreamingListener {
    fn on_item_update(&self, update: &ItemUpdate) {
        let tx = self.tx.clone();
        let name = self.name.clone();
        let update_clone = update.clone();
        tracing::info!("Raw LS update [{}]: {:?}", name, update_clone);

        tokio::spawn(async move {
            match name.as_str() {
                "PRICES" => {
                    if let Some(market_state) = parse_market_state_from_update(&update_clone) {
                        let _ = tx.send(StateUpdate::Market(market_state)).await;
                    }
                }
                "ACCOUNT" => {
                    let fields = update_clone.get_fields_as_json();
                    let _ = tx.send(StateUpdate::Account(fields)).await;
                }
                "TRADES" => {
                    let fields = update_clone.get_fields_as_json();
                    let _ = tx.send(StateUpdate::Trade(fields)).await;
                }
                _ => {}
            }
        });
    }

    fn on_subscription(&mut self) {
        info!("[{}] Subscription confirmed by server", self.name);
    }

    fn on_unsubscription(&mut self) {
        info!("[{}] Unsubscribed", self.name);
    }
}

/// Helper to convert ItemUpdate fields to JSON Value
trait ItemUpdateExt {
    fn get_fields_as_json(&self) -> serde_json::Value;
}

impl ItemUpdateExt for ItemUpdate {
    fn get_fields_as_json(&self) -> serde_json::Value {
        let mut map = serde_json::Map::new();
        for (k, v) in &self.fields {
            if let Some(val) = v {
                map.insert(k.clone(), serde_json::json!(val));
            }
        }
        serde_json::Value::Object(map)
    }
}

/// Streaming client for IG Lightstreamer market data
pub struct IGStreamingClient {
    ls_client: LightstreamerClient,
    shutdown: Arc<Notify>,
    state: Arc<RwLock<EngineState>>,
    event_tx: broadcast::Sender<EngineEvent>,
    update_tx: mpsc::Sender<StateUpdate>,
}

impl IGStreamingClient {
    /// Create a new streaming client using credentials from the REST client
    pub fn new(
        lightstreamer_endpoint: &str,
        account_id: &str,
        cst: &str,
        security_token: &str,
        state: Arc<RwLock<EngineState>>,
        event_tx: broadcast::Sender<EngineEvent>,
    ) -> Result<Self, anyhow::Error> {
        // IG wants the password as "CST-{cst}|XST-{security_token}"
        let ls_password = format!("CST-{}|XST-{}", cst, security_token);

        // IG returns endpoint like "https://demo-apd.marketdatasystems.com"
        // Lightstreamer server lives at the /lightstreamer path
        let full_endpoint = format!("{}/lightstreamer", lightstreamer_endpoint.trim_end_matches('/'));

        let mut ls_client = LightstreamerClient::new(
            Some(&full_endpoint),
            Some("DEFAULT"), // IG uses the DEFAULT adapter set
            Some(account_id),
            Some(&ls_password),
        ).map_err(|e| anyhow::anyhow!("Failed to create Lightstreamer client: {}", e))?;

        // Only WS streaming is supported by this crate
        ls_client.connection_options.set_forced_transport(Some(Transport::WsStreaming));
        ls_client.set_logging_type(LogType::TracingLogs);

        let shutdown = Arc::new(Notify::new());

        // Create update worker
        let (update_tx, mut update_rx) = mpsc::channel(100);
        let state_worker = state.clone();
        let event_tx_worker = event_tx.clone();

        tokio::spawn(async move {
            info!("State update worker started");
            while let Some(update) = update_rx.recv().await {
                match update {
                    StateUpdate::Market(market_state) => {
                        let mut s = state_worker.write().await;
                        s.markets.live.insert(market_state.epic.clone(), market_state.clone());
                        let _ = event_tx_worker.send(EngineEvent::market_update(market_state));
                    }
                    StateUpdate::Account(fields) => {
                        let mut s = state_worker.write().await;
                        // Map LS fields to AccountState
                        if let Some(val) = fields.get("FUNDS").and_then(|v| v.as_str()).and_then(|v| v.parse::<f64>().ok()) {
                            s.account.balance = val;
                        }
                        if let Some(val) = fields.get("AVAILABLE_TO_DEAL").and_then(|v| v.as_str()).and_then(|v| v.parse::<f64>().ok()) {
                            s.account.available = val;
                        }
                        if let Some(val) = fields.get("EQUITY_USED").and_then(|v| v.as_str()).and_then(|v| v.parse::<f64>().ok()) {
                            s.account.margin = val;
                        }
                        if let Some(val) = fields.get("PNL").and_then(|v| v.as_str()).and_then(|v| v.parse::<f64>().ok()) {
                            s.account.pnl = val;
                        }
                        // Optionally broadcast account heartbeat
                    }
                    StateUpdate::Trade(fields) => {
                        info!("Trade update received: {:?}", fields);
                        // Confirms, OPU (Open Position Update), WOU (Working Order Update)
                        // Trigger a positions refresh if something changed
                    }
                }
            }
            info!("State update worker stopped");
        });

        info!(
            "Lightstreamer client created for endpoint: {}",
            full_endpoint
        );

        Ok(Self { 
            ls_client, 
            shutdown,
            state,
            event_tx,
            update_tx,
        })
    }

    /// Subscribe to real-time price updates for the given epics
    pub fn subscribe_prices(&mut self, epics: &[String]) {
        let items: Vec<String> = epics
            .iter()
            .map(|epic| format!("MARKET:{}", epic))
            .collect();

        let fields: Vec<String> = PRICE_FIELDS.iter().map(|f| f.to_string()).collect();

        match Subscription::new(SubscriptionMode::Merge, Some(items.clone()), Some(fields)) {
            Ok(mut subscription) => {
                let _ = subscription.set_requested_snapshot(Some(Snapshot::Yes));
                subscription.add_listener(Box::new(StreamingListener {
                    name: "PRICES".to_string(),
                    tx: self.update_tx.clone(),
                }));

                let sender = self.ls_client.subscription_sender.clone();
                LightstreamerClient::subscribe(sender, subscription);
                info!(
                    "Subscribed to price updates for {} markets: {:?}",
                    epics.len(),
                    epics
                );
            }
            Err(e) => {
                error!("Failed to create price subscription: {:?}", e);
            }
        }
    }

    /// Subscribe to account balance/margin updates
    pub fn subscribe_account(&mut self, account_id: &str) {
        let items = vec![format!("ACCOUNT:{}", account_id)];
        let fields = vec![
            "PNL".to_string(),
            "DEPOSIT".to_string(),
            "AVAILABLE_CASH".to_string(),
            "FUNDS".to_string(),
            "MARGIN".to_string(),
            "AVAILABLE_TO_DEAL".to_string(),
            "EQUITY".to_string(),
            "EQUITY_USED".to_string(),
        ];

        match Subscription::new(SubscriptionMode::Merge, Some(items), Some(fields)) {
            Ok(mut subscription) => {
                let _ = subscription.set_requested_snapshot(Some(Snapshot::Yes));
                subscription.add_listener(Box::new(StreamingListener {
                    name: "ACCOUNT".to_string(),
                    tx: self.update_tx.clone(),
                }));

                let sender = self.ls_client.subscription_sender.clone();
                LightstreamerClient::subscribe(sender, subscription);
                info!("Subscribed to account updates for {}", account_id);
            }
            Err(e) => {
                error!("Failed to create account subscription: {:?}", e);
            }
        }
    }

    /// Subscribe to trade confirmations
    pub fn subscribe_trades(&mut self, account_id: &str) {
        let items = vec![format!("TRADE:{}", account_id)];
        let fields = vec![
            "CONFIRMS".to_string(),
            "OPU".to_string(),
            "WOU".to_string(),
        ];

        match Subscription::new(SubscriptionMode::Distinct, Some(items), Some(fields)) {
            Ok(mut subscription) => {
                // For Distinct mode, Snapshot::Yes or a number is expected
                let _ = subscription.set_requested_snapshot(Some(Snapshot::Yes));
                subscription.add_listener(Box::new(StreamingListener {
                    name: "TRADES".to_string(),
                    tx: self.update_tx.clone(),
                }));

                let sender = self.ls_client.subscription_sender.clone();
                LightstreamerClient::subscribe(sender, subscription);
                info!("Subscribed to trade confirmations for {}", account_id);
            }
            Err(e) => {
                error!("Failed to create trade subscription: {:?}", e);
            }
        }
    }

    /// Connect to the Lightstreamer server and begin streaming.
    /// This call blocks until the shutdown signal is triggered or the connection drops.
    pub async fn connect(&mut self) -> Result<(), anyhow::Error> {
        info!("Connecting to IG Lightstreamer...");
        self.ls_client
            .connect(self.shutdown.clone())
            .await
            .map_err(|e| anyhow::anyhow!("Lightstreamer connection error: {}", e))?;
        info!("Lightstreamer session ended");
        Ok(())
    }

    /// Signal the streaming client to disconnect
    pub fn shutdown(&self) {
        info!("Sending Lightstreamer shutdown signal");
        self.shutdown.notify_one();
    }

    /// Set the shutdown notify object (used for external cancellation)
    pub fn set_shutdown_notify(&mut self, shutdown: Arc<Notify>) {
        self.shutdown = shutdown;
    }
}

/// Parse a Lightstreamer price update into a MarketState
pub fn parse_market_state_from_update(update: &ItemUpdate) -> Option<MarketState> {
    let item_name = update.item_name.as_ref()?;
    let epic = item_name.strip_prefix("MARKET:")?;

    // Use changed_fields for values that just arrived, fall back to full fields
    let get_field = |name: &str| -> Option<f64> {
        update
            .changed_fields
            .get(name)
            .or_else(|| update.fields.get(name).and_then(|v| v.as_ref()))
            .and_then(|v| v.parse::<f64>().ok())
    };

    let bid = get_field("BID");
    let offer = get_field("OFFER");
    
    // If we have neither bid nor offer, skip this update as it might be high/low only
    if bid.is_none() && offer.is_none() {
        return None;
    }

    Some(MarketState {
        epic: epic.to_string(),
        bid: bid.unwrap_or(0.0),
        ask: offer.unwrap_or(0.0),
        spread: if let (Some(b), Some(o)) = (bid, offer) { o - b } else { 0.0 },
        high: get_field("HIGH").unwrap_or(0.0),
        low: get_field("LOW").unwrap_or(0.0),
        change_pct: get_field("CHANGE_PCT").unwrap_or(0.0),
        last_update: chrono::Utc::now(),
    })
}
