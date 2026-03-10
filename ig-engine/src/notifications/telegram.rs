#![allow(dead_code)]
//! Telegram notifications for trading engine
//! Sends trade alerts, risk warnings, and daily summaries via Telegram

use reqwest::Client;
use serde_json::json;
use std::error::Error;
use tracing::{error, info, warn};

use crate::engine::config::TelegramConfig;
use crate::engine::state::get_instrument_name;

/// Sends notifications to Telegram via the Telegram Bot API
#[derive(Clone)]
pub struct TelegramNotifier {
    bot_token: String,
    chat_id: String,
    enabled: bool,
    trade_alerts: bool,
    risk_alerts: bool,
    daily_summary: bool,
    client: Client,
}

impl TelegramNotifier {
    /// Creates a new TelegramNotifier from environment variables and config
    ///
    /// Reads TELEGRAM_BOT_TOKEN and TELEGRAM_CHAT_ID from the environment.
    /// If either is missing, the notifier will be disabled.
    pub fn new(config: &Option<TelegramConfig>) -> Self {
        let bot_token = std::env::var("TELEGRAM_BOT_TOKEN")
            .ok()
            .filter(|s| !s.trim().is_empty());
        let chat_id = std::env::var("TELEGRAM_CHAT_ID")
            .ok()
            .filter(|s| !s.trim().is_empty());

        let env_ok = bot_token.is_some() && chat_id.is_some();
        
        let (enabled, trade_alerts, risk_alerts, daily_summary) = if let Some(cfg) = config {
            (cfg.enabled && env_ok, cfg.trade_alerts, cfg.risk_alerts, cfg.daily_summary)
        } else {
            (env_ok, true, true, true)
        };

        if !env_ok && config.as_ref().is_some_and(|c| c.enabled) {
            warn!(
                "Telegram notifier requested in config but disabled: missing {} {}",
                if bot_token.is_none() { "TELEGRAM_BOT_TOKEN" } else { "" },
                if chat_id.is_none() { "TELEGRAM_CHAT_ID" } else { "" }
            );
        }

        Self {
            bot_token: bot_token.unwrap_or_default(),
            chat_id: chat_id.unwrap_or_default(),
            enabled,
            trade_alerts,
            risk_alerts,
            daily_summary,
            client: Client::new(),
        }
    }

    /// Returns whether the notifier is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Sends a startup message to confirm the bot is alive and can reach Telegram.
    pub async fn send_startup_ping(&self, mode: &str, markets: &[String]) {
        if !self.enabled {
            warn!("⚠️  Telegram notifier is DISABLED — no alerts will be sent.");
            return;
        }

        let resolved_markets: Vec<String> = markets.iter()
            .map(|m| get_instrument_name(m))
            .collect();

        let market_list = if resolved_markets.len() <= 5 {
            resolved_markets.join(", ")
        } else {
            format!("{} (+{} more)", resolved_markets[..3].join(", "), resolved_markets.len() - 3)
        };

        let message = format!(
            "🤖 <b>IG Trading Engine Online</b>\n\n\
            <b>Mode:</b> {}\n\
            <b>Markets:</b> {}\n\
            <b>Time:</b> {}",
            mode,
            market_list,
            (chrono::Utc::now() + chrono::Duration::hours(8)).format("%Y-%m-%d %H:%M:%S SGT"),
        );

        match self.send_message(&message).await {
            Ok(_) => info!("✅ Telegram startup ping sent successfully"),
            Err(e) => error!("❌ Telegram startup ping FAILED: {}", e),
        }
    }

    /// Sends a raw text message to Telegram
    pub async fn send_message(&self, text: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
        if !self.enabled {
            return Ok(());
        }

        let url = format!(
            "https://api.telegram.org/bot{}/sendMessage",
            self.bot_token
        );

        let body = json!({
            "chat_id": self.chat_id,
            "text": text,
            "parse_mode": "HTML"
        });

        match self.client.post(&url).json(&body).send().await {
            Ok(resp) => {
                if !resp.status().is_success() {
                    let status = resp.status();
                    let body_text = resp.text().await.unwrap_or_default();
                    error!("Telegram API error {}: {}", status, body_text);
                } else {
                    info!("Telegram message sent successfully");
                }
                Ok(())
            }
            Err(e) => {
                error!("Failed to send Telegram message: {}", e);
                Ok(())
            }
        }
    }

    /// Sends a trade alert to Telegram
    pub async fn send_trade_alert(
        &self,
        epic: &str,
        direction: &str,
        size: f64,
        price: f64,
        sl: f64,
        tp: Option<f64>,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        if !self.enabled || !self.trade_alerts {
            return Ok(());
        }

        let direction_emoji = if direction.to_uppercase() == "BUY" {
            "🟢"
        } else {
            "🔴"
        };

        let instrument = get_instrument_name(epic);
        let mut message = format!(
            "{} <b>TRADE ALERT</b>\n\n\
            <b>Instrument:</b> {}\n\
            <b>Direction:</b> {}\n\
            <b>Size:</b> {}\n\
            <b>Entry Price:</b> {}\n\
            <b>Stop Loss:</b> {}\n\
            <b>Time:</b> {}",
            direction_emoji, instrument, direction, size, price, sl,
            (chrono::Utc::now() + chrono::Duration::hours(8)).format("%H:%M:%S SGT")
        );

        if let Some(tp_price) = tp {
            message.push_str(&format!("\n<b>Take Profit:</b> {}", tp_price));
        }

        self.send_message(&message).await
    }

    /// Sends a risk alert to Telegram
    pub async fn send_risk_alert(&self, message: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
        if !self.enabled || !self.risk_alerts {
            return Ok(());
        }

        let formatted = format!("⚠️ <b>RISK ALERT</b>\n\n{}", message);
        self.send_message(&formatted).await
    }

    /// Sends a risk alert for a specific instrument, resolving the EPIC to a name
    pub async fn send_instrument_risk_alert(&self, epic: &str, reason: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
        if !self.enabled || !self.risk_alerts {
            return Ok(());
        }

        let instrument = get_instrument_name(epic);
        let formatted = format!(
            "⚠️ <b>RISK ALERT: {}</b>\n\n{}\n\n<b>Time:</b> {}", 
            instrument, 
            reason,
            (chrono::Utc::now() + chrono::Duration::hours(8)).format("%H:%M:%S SGT")
        );
        self.send_message(&formatted).await
    }

    /// Sends a daily trading summary to Telegram
    pub async fn send_daily_summary(
        &self,
        trades: u32,
        wins: u32,
        pnl: f64,
        balance: f64,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        if !self.enabled || !self.daily_summary {
            return Ok(());
        }

        let win_rate = if trades > 0 {
            (wins as f64 / trades as f64) * 100.0
        } else {
            0.0
        };

        let pnl_emoji = if pnl >= 0.0 { "📈" } else { "📉" };

        let message = format!(
            "{} <b>DAILY SUMMARY</b>\n\n\
            <b>Trades:</b> {}\n\
            <b>Wins:</b> {} ({:.1}%)\n\
            <b>P&L:</b> {}\n\
            <b>Balance:</b> {}\n\
            <b>Time:</b> {}",
            pnl_emoji, trades, wins, win_rate, pnl, balance,
            (chrono::Utc::now() + chrono::Duration::hours(8)).format("%Y-%m-%d %H:%M:%S SGT")
        );

        self.send_message(&message).await
    }

    /// Polls for new messages and handles commands like /status and /positions.
    /// This is a simple long-polling implementation.
    pub async fn start_listener(&self, state: std::sync::Arc<tokio::sync::RwLock<crate::engine::state::EngineState>>) {
        if !self.enabled {
            return;
        }

        info!("Telegram command listener started");
        let mut last_update_id = 0;

        loop {
            let url = format!(
                "https://api.telegram.org/bot{}/getUpdates?offset={}&timeout=30",
                self.bot_token,
                last_update_id + 1
            );

            match self.client.get(&url).send().await {
                Ok(resp) => {
                    if let Ok(json) = resp.json::<serde_json::Value>().await {
                        if let Some(updates) = json["result"].as_array() {
                            for update in updates {
                                if let Some(update_id) = update["update_id"].as_i64() {
                                    last_update_id = update_id;
                                }

                                if let Some(message) = update["message"].as_object() {
                                    let from_id = message["from"]["id"].as_i64().unwrap_or(0).to_string();
                                    
                                    // Security check: only respond to the authorized chat_id
                                    if from_id != self.chat_id {
                                        continue;
                                    }

                                    if let Some(text) = message["text"].as_str() {
                                        self.process_command(text, &state).await;
                                    }
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    error!("Error polling Telegram updates: {}", e);
                    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                }
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        }
    }

    async fn process_command(&self, text: &str, state: &tokio::sync::RwLock<crate::engine::state::EngineState>) {
        let command = text.split_whitespace().next().unwrap_or("").to_lowercase();
        
        match command.as_str() {
            "/status" => {
                let s = state.read().await;
                let status_msg = format!(
                    "🤖 <b>Engine Status</b>\n\n\
                    <b>Status:</b> {:?}\n\
                    <b>Balance:</b> ${:.2} {}\n\
                    <b>Uptime:</b> {}s\n\
                    <b>Time:</b> {}\n\
                    <b>Active Trades:</b> {}",
                    s.status,
                    s.account.balance,
                    s.account.currency,
                    s.started_at.map(|t| (chrono::Utc::now() - t).num_seconds()).unwrap_or(0),
                    (chrono::Utc::now() + chrono::Duration::hours(8)).format("%H:%M:%S SGT"),
                    s.trades.active.len()
                );
                let _ = self.send_message(&status_msg).await;
            }
            "/positions" => {
                let s = state.read().await;
                if s.trades.active.is_empty() {
                    let _ = self.send_message("📝 No active positions.").await;
                    return;
                }

                let mut msg = "📋 <b>Active Positions</b>\n\n".to_string();
                for pos in &s.trades.active {
                    let name = get_instrument_name(&pos.epic);
                    let emoji = if pos.direction == crate::engine::state::Direction::Buy { "🟢" } else { "🔴" };
                    let opened_sgt = (pos.opened_at + chrono::Duration::hours(8)).format("%H:%M");
                    msg.push_str(&format!(
                        "{} <b>{}</b>\n\
                        Entry: {:.2} | Current: {:.2}\n\
                        Size: {} | P&L: <b>{:.2}</b>\n\
                        Opened: {} SGT\n\n",
                        emoji, name, pos.open_price, pos.current_price, pos.size, pos.pnl, opened_sgt
                    ));
                }
                let _ = self.send_message(&msg).await;
            }
            "/help" => {
                let msg = "Available commands:\n/status - Check engine health\n/positions - List open trades";
                let _ = self.send_message(msg).await;
            }
            _ => {}
        }
    }
}

impl Default for TelegramNotifier {
    fn default() -> Self {
        Self::new(&None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_telegram_notifier_disabled_without_env() {
        // Clear env vars for test
        std::env::remove_var("TELEGRAM_BOT_TOKEN");
        std::env::remove_var("TELEGRAM_CHAT_ID");

        let notifier = TelegramNotifier::new(&None);
        assert!(!notifier.enabled);
    }

    #[tokio::test]
    async fn test_send_message_when_disabled() {
        let mut notifier = TelegramNotifier::new(&None);
        notifier.enabled = false;

        let result = notifier.send_message("test").await;
        assert!(result.is_ok());
    }
}
