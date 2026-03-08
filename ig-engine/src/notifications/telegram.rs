#![allow(dead_code)]
//! Telegram notifications for trading engine
//! Sends trade alerts, risk warnings, and daily summaries via Telegram

use reqwest::Client;
use serde_json::json;
use std::error::Error;
use tracing::{error, info, warn};

/// Get human-readable instrument name from epic code.
/// Single source of truth — used by http_server.rs, handlers.rs, and Telegram alerts.
/// Covers both demo (*.CSD.IP / *.CFI.IP) and live (*.CFD) epic variants.
pub fn get_instrument_name(epic: &str) -> String {
    match epic {
        // Gold variants
        "CS.D.CFIGOLD.CFI.IP" => "Spot Gold (SGD1)".to_string(),
        "CS.D.CFDGOLD.CMG.IP" => "Spot Gold ($1)".to_string(),
        "CS.D.GOL.CFD"        => "Spot Gold".to_string(),
        "CS.D.XAUUSD.CFD" | "CS.D.GOLDUSD.CFD" => "Gold (XAU/USD)".to_string(),
        "IX.D.SUNGOLD.CFI.IP" => "Weekend Spot Gold".to_string(),
        // Forex — demo (*.CSD.IP) and live (*.CFD) variants
        "CS.D.EURUSD.CSD.IP" | "CS.D.EURUSD.CFD" => "EUR/USD".to_string(),
        "CS.D.GBPUSD.CSD.IP" | "CS.D.GBPUSD.CFD" => "GBP/USD".to_string(),
        "CS.D.USDJPY.CSD.IP" | "CS.D.USDJPY.CFD" => "USD/JPY".to_string(),
        "CS.D.AUDUSD.CSD.IP" | "CS.D.AUDUSD.CFD" => "AUD/USD".to_string(),
        _ => {
            // Fallback: extract pair from epic segment (e.g., "CS.D.GBPUSD.CSD.IP" → "GBP/USD")
            let parts: Vec<&str> = epic.split('.').collect();
            if parts.len() >= 3 {
                let pair = parts[2];
                if pair.len() == 6 && pair.chars().all(|c| c.is_ascii_uppercase()) {
                    format!("{}/{}", &pair[0..3], &pair[3..6])
                } else {
                    pair.to_string()
                }
            } else {
                epic.to_string()
            }
        }
    }
}

/// Sends notifications to Telegram via the Telegram Bot API
#[derive(Clone)]
pub struct TelegramNotifier {
    bot_token: String,
    chat_id: String,
    enabled: bool,
    client: Client,
}

impl TelegramNotifier {
    /// Creates a new TelegramNotifier from environment variables
    ///
    /// Reads TELEGRAM_BOT_TOKEN and TELEGRAM_CHAT_ID from the environment.
    /// If either is missing, the notifier will be disabled and a warning will be logged.
    pub fn new() -> Self {
        let bot_token = std::env::var("TELEGRAM_BOT_TOKEN")
            .ok()
            .filter(|s| !s.trim().is_empty());
        let chat_id = std::env::var("TELEGRAM_CHAT_ID")
            .ok()
            .filter(|s| !s.trim().is_empty());

        let enabled = bot_token.is_some() && chat_id.is_some();

        if !enabled {
            warn!(
                "Telegram notifier disabled: missing {} {}",
                if bot_token.is_none() {
                    "TELEGRAM_BOT_TOKEN"
                } else {
                    ""
                },
                if chat_id.is_none() {
                    "TELEGRAM_CHAT_ID"
                } else {
                    ""
                }
            );
        }

        Self {
            bot_token: bot_token.unwrap_or_default(),
            chat_id: chat_id.unwrap_or_default(),
            enabled,
            client: Client::new(),
        }
    }

    /// Returns whether the notifier is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Sends a startup message to confirm the bot is alive and can reach Telegram.
    /// Should be called once during engine init. Logs success/failure clearly.
    pub async fn send_startup_ping(&self, mode: &str, markets: &[String]) {
        if !self.enabled {
            warn!("⚠️  Telegram notifier is DISABLED — no alerts will be sent. Set TELEGRAM_BOT_TOKEN and TELEGRAM_CHAT_ID env vars.");
            return;
        }

        let market_list = if markets.len() <= 5 {
            markets.join(", ")
        } else {
            format!("{} (+{} more)", markets[..3].join(", "), markets.len() - 3)
        };

        let message = format!(
            "🤖 <b>IG Trading Engine Online</b>\n\n\
            <b>Mode:</b> {}\n\
            <b>Markets:</b> {}\n\
            <b>Time:</b> {}",
            mode,
            market_list,
            chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC"),
        );

        match self.send_message(&message).await {
            Ok(_) => info!("✅ Telegram startup ping sent successfully"),
            Err(e) => error!("❌ Telegram startup ping FAILED: {} — notifications will not work!", e),
        }
    }

    /// Sends a raw text message to Telegram
    ///
    /// # Arguments
    /// * `text` - The message text (supports HTML formatting)
    ///
    /// Skips sending if the notifier is disabled. Errors are logged but not propagated.
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
    ///
    /// # Arguments
    /// * `epic` - The instrument epic/code
    /// * `direction` - Trade direction ("BUY" or "SELL")
    /// * `size` - Position size
    /// * `price` - Entry price
    /// * `sl` - Stop loss price
    /// * `tp` - Optional take profit price
    ///
    /// Formats the alert as an HTML message. Fire-and-forget pattern.
    pub async fn send_trade_alert(
        &self,
        epic: &str,
        direction: &str,
        size: f64,
        price: f64,
        sl: f64,
        tp: Option<f64>,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let direction_emoji = if direction.to_uppercase() == "BUY" {
            "🟢"
        } else {
            "🔴"
        };

        let instrument = get_instrument_name(epic);
        let mut message = format!(
            "{} <b>TRADE ALERT</b>\n\n\
            <b>Instrument:</b> {} ({})\n\
            <b>Direction:</b> {}\n\
            <b>Size:</b> {}\n\
            <b>Entry Price:</b> {}\n\
            <b>Stop Loss:</b> {}",
            direction_emoji, instrument, epic, direction, size, price, sl
        );

        if let Some(tp_price) = tp {
            message.push_str(&format!("\n<b>Take Profit:</b> {}", tp_price));
        }

        self.send_message(&message).await
    }

    /// Sends a risk alert to Telegram
    ///
    /// # Arguments
    /// * `message` - The risk warning message
    ///
    /// Formats the alert with a warning emoji. Fire-and-forget pattern.
    pub async fn send_risk_alert(&self, message: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
        let formatted = format!("⚠️ <b>RISK ALERT</b>\n\n{}", message);
        self.send_message(&formatted).await
    }

    /// Sends a daily trading summary to Telegram
    ///
    /// # Arguments
    /// * `trades` - Number of trades executed
    /// * `wins` - Number of winning trades
    /// * `pnl` - Profit/loss in account currency
    /// * `balance` - Current account balance
    ///
    /// Formats daily statistics as an HTML message. Fire-and-forget pattern.
    pub async fn send_daily_summary(
        &self,
        trades: u32,
        wins: u32,
        pnl: f64,
        balance: f64,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
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
            <b>Balance:</b> {}",
            pnl_emoji, trades, wins, win_rate, pnl, balance
        );

        self.send_message(&message).await
    }
}

impl Default for TelegramNotifier {
    fn default() -> Self {
        Self::new()
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

        let notifier = TelegramNotifier::new();
        assert!(!notifier.enabled);
    }

    #[tokio::test]
    async fn test_send_message_when_disabled() {
        let mut notifier = TelegramNotifier::new();
        notifier.enabled = false;

        let result = notifier.send_message("test").await;
        assert!(result.is_ok());
    }
}
