#![allow(dead_code)]
use std::sync::Arc;
use tokio::sync::{Notify, RwLock, broadcast, mpsc};
use tracing::{info, warn, error};
use chrono::Utc;

use lightstreamer_client::ls_client::{LightstreamerClient, LogType, Transport};
use lightstreamer_client::subscription::{Subscription, SubscriptionMode, Snapshot};
use lightstreamer_client::subscription_listener::SubscriptionListener;
use lightstreamer_client::item_update::ItemUpdate;

use crate::engine::state::{EngineState, MarketState, ClosedTrade, Direction};
use crate::ipc::events::EngineEvent;
use crate::notifications::telegram::{TelegramNotifier, get_instrument_name};

/// Internal event for the state worker
pub enum StateUpdate {
    Market(MarketState),
    Account(serde_json::Value),
    Trade(serde_json::Value),
}

/// IG OPU (Open Position Update) — subset of fields we care about
/// Arrives as JSON string inside the TRADE subscription's OPU field.
struct Opu {
    deal_id: String,
    epic: String,
    direction: Direction,
    level: f64,      // close price
    size: f64,
    status: String,  // "DELETED" = closed
    pnl: f64,
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
        tracing::trace!("Raw LS update [{}]: {:?}", name, update_clone);

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

/// Spawn the state-update worker task **once**, shared across all reconnect attempts.
///
/// Previously the worker was spawned inside `IGStreamingClient::new()`, which caused
/// a new task (and a new mpsc channel) to be created on every reconnect. During
/// market closure the reconnect loop cycles every ~5 s, leaking task handles.
///
/// Callers should invoke this function once before the reconnect loop, then pass
/// clones of the returned `Sender` into each `IGStreamingClient::new()` call.
pub fn spawn_state_worker(
    state: Arc<RwLock<EngineState>>,
    event_tx: broadcast::Sender<EngineEvent>,
) -> mpsc::Sender<StateUpdate> {
    let (update_tx, mut update_rx) = mpsc::channel::<StateUpdate>(100);
    let state_worker = state;
    let event_tx_worker = event_tx;

    tokio::spawn(async move {
        info!("State update worker started");
        while let Some(update) = update_rx.recv().await {
            match update {
                StateUpdate::Market(mut market_state) => {
                    let mid = (market_state.bid + market_state.ask) / 2.0;
                    let now_ts = Utc::now().timestamp();

                    let mut s = state_worker.write().await;

                    // Preserve market_state from previous update when not included in this tick.
                    // Lightstreamer Merge mode only sends changed fields, so MARKET_STATE may
                    // arrive on the snapshot and then not repeat on subsequent price ticks.
                    if market_state.market_state.is_none() {
                        if let Some(prev) = s.markets.live.get(&market_state.epic) {
                            market_state.market_state = prev.market_state.clone();
                        }
                    }

                    // Accumulate tick into the current OHLCV bar.
                    // When the bar boundary flips, push the completed candle to history
                    // and advance each indicator set with a proper OHLCV bar.
                    if let Some(completed) = s.markets.bar_accumulator.update(&market_state.epic, mid, now_ts) {
                        s.markets.history.push(&market_state.epic, "HOUR", completed.clone());

                        if let Some(tf_map) = s.markets.indicators.get_mut(&market_state.epic) {
                            for indicator_set in tf_map.values_mut() {
                                indicator_set.update(&completed);
                            }
                        }

                        info!(
                            "Bar closed for {} @ {}: O={:.5} H={:.5} L={:.5} C={:.5}",
                            market_state.epic,
                            completed.timestamp,
                            completed.open,
                            completed.high,
                            completed.low,
                            completed.close
                        );
                    }

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
                }
                StateUpdate::Trade(fields) => {
                    // Parse OPU (Open Position Update) — sent when IG closes a position server-side
                    // e.g. stop loss hit, take profit hit, manual close in app
                    if let Some(opu_str) = fields.get("OPU").and_then(|v| v.as_str()) {
                        match parse_opu(opu_str) {
                            Some(opu) if opu.status == "DELETED" => {
                                info!("OPU: position closed server-side: deal_id={}, epic={}, pnl={:.2}", opu.deal_id, opu.epic, opu.pnl);

                                let (closed_position, close_reason) = {
                                    let mut s = state_worker.write().await;

                                    // Find and remove the position from active list
                                    let pos_idx = s.trades.active.iter().position(|p| p.deal_id == opu.deal_id);
                                    if let Some(idx) = pos_idx {
                                        let pos = s.trades.active.remove(idx);

                                        // Record in closed trade history
                                        s.add_closed_trade(ClosedTrade {
                                            deal_id: opu.deal_id.clone(),
                                            epic: opu.epic.clone(),
                                            direction: opu.direction.clone(),
                                            size: pos.size,
                                            entry_price: pos.open_price,
                                            exit_price: opu.level,
                                            stop_loss: pos.stop_loss.unwrap_or(0.0),
                                            take_profit: pos.take_profit,
                                            pnl: opu.pnl,
                                            strategy: pos.strategy.clone(),
                                            status: "closed_server_side".to_string(),
                                            opened_at: pos.opened_at,
                                            closed_at: Utc::now(),
                                            is_virtual: pos.is_virtual,
                                        });

                                        s.record_trade_result(opu.pnl);
                                        (Some(pos), "Server-side close (SL/TP/manual)")
                                    } else {
                                        warn!("OPU DELETED for unknown deal_id={} — may have been closed by engine already", opu.deal_id);
                                        (None, "")
                                    }
                                };

                                if let Some(pos) = closed_position {
                                    let _ = event_tx_worker.send(EngineEvent::position_closed(
                                        opu.deal_id.clone(),
                                        opu.pnl,
                                    ));

                                    // Telegram notification
                                    let tg = TelegramNotifier::new(&None);
                                    let name = get_instrument_name(&opu.epic);
                                    let dir = format!("{}", pos.direction);
                                    let pnl = opu.pnl;
                                    let reason = close_reason.to_string();
                                    tokio::spawn(async move {
                                        let msg = format!(
                                            "{} <b>POSITION CLOSED (stream)</b>\n\n<b>Instrument:</b> {}\n<b>Direction:</b> {}\n<b>Reason:</b> {}\n<b>P&amp;L:</b> {:.2}",
                                            if pnl >= 0.0 { "✅" } else { "❌" },
                                            name, dir, reason, pnl
                                        );
                                        let _ = tg.send_message(&msg).await;
                                    });
                                }
                            }
                            Some(opu) => {
                                info!("OPU: status={} for deal_id={} (not a close, ignoring)", opu.status, opu.deal_id);
                            }
                            None => {
                                warn!("OPU: failed to parse OPU payload: {}", opu_str);
                            }
                        }
                    } else if fields.get("CONFIRMS").and_then(|v| v.as_str()).is_some() {
                        // CONFIRMS are already handled by order_manager polling — safe to ignore here
                        info!("Trade CONFIRM received via stream (order_manager handles this)");
                    }
                }
            }
        }
        info!("State update worker stopped");
    });

    update_tx
}

/// Streaming client for IG Lightstreamer market data
pub struct IGStreamingClient {
    ls_client: LightstreamerClient,
    shutdown: Arc<Notify>,
    update_tx: mpsc::Sender<StateUpdate>,
}

impl IGStreamingClient {
    /// Create a new streaming client.
    ///
    /// `update_tx` is the sender half of the state-update channel created by
    /// [`spawn_state_worker`].  The same channel is reused across reconnects so
    /// no new worker task is spawned here.
    pub fn new(
        lightstreamer_endpoint: &str,
        account_id: &str,
        cst: &str,
        security_token: &str,
        update_tx: mpsc::Sender<StateUpdate>,
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

        info!(
            "Lightstreamer client created for endpoint: {}",
            full_endpoint
        );

        Ok(Self {
            ls_client,
            shutdown,
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

    // String-valued field extractor (e.g. MARKET_STATE = "TRADEABLE" / "EDIT" / "OFFLINE")
    let get_string_field = |name: &str| -> Option<String> {
        update
            .changed_fields
            .get(name)
            .cloned()
            .or_else(|| update.fields.get(name).and_then(|v| v.clone()))
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
        market_state: get_string_field("MARKET_STATE"),
        last_update: chrono::Utc::now(),
    })
}

/// Helper to parse the JSON string payload of an OPU update
fn parse_opu(payload: &str) -> Option<Opu> {
    let json: serde_json::Value = serde_json::from_str(payload).ok()?;
    
    let deal_id = json.get("dealId")?.as_str()?.to_string();
    let epic = json.get("epic")?.as_str()?.to_string();
    
    let dir_str = json.get("direction")?.as_str()?;
    let direction = match dir_str {
        "BUY" => Direction::Buy,
        "SELL" => Direction::Sell,
        _ => return None,
    };
    
    let level = json.get("level")?.as_f64()?;
    let size = json.get("size")?.as_f64()?;
    let status = json.get("status")?.as_str()?.to_string();
    
    // OPU pnl is often absent until closed/deleted, or inside a nested field depending on the version.
    // We try 'profitAndLoss' first, otherwise default to 0.0 (the server doesn't always send the final PnL strictly in the OPU payload; in production you'd reconcile this with account/trade history).
    let pnl = json.get("profitAndLoss").and_then(|v| v.as_f64()).unwrap_or(0.0);
    
    Some(Opu {
        deal_id,
        epic,
        direction,
        level,
        size,
        status,
        pnl,
    })
}
