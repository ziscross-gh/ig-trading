#![allow(dead_code)]
pub mod sentiment;
pub use sentiment::{GlobalSentimentRegistry, SentimentData};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};

use crate::data::bar_accumulator::BarAccumulator;
use crate::data::candle_store::CandleStore;
use crate::engine::config::EngineConfig;
use crate::indicators::IndicatorSet;
use crate::learning::adaptive_weights::{AdaptiveWeightManager, WeightAdjustment};
use crate::learning::scorecard::StrategyScorecard;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum EngineStatus {
    Starting,
    Running,
    Paused,
    Stopped,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Session {
    Asia,      // 00:00–08:00 UTC
    London,    // 08:00–13:00 UTC
    UsOverlap, // 13:00–16:00 UTC
}

impl Session {
    pub fn from_utc_hour(hour: u32) -> Self {
        match hour {
            0..=7 => Session::Asia,
            8..=12 => Session::London,
            _ => Session::UsOverlap,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Session::Asia => "Asia",
            Session::London => "London",
            Session::UsOverlap => "US",
        }
    }
}

/// Get human-readable instrument name from epic code.
/// Single source of truth — used by http_server.rs, handlers.rs, and Telegram alerts.
/// Covers both demo (*.CSD.IP / *.CFI.IP) and live (*.CFD) epic variants.
pub fn get_instrument_name(epic: &str) -> String {
    match epic {
        // Gold variants
        "CS.D.CFIGOLD.CFI.IP" => "Spot Gold (SGD1)".to_string(),
        "CS.D.CFDGOLD.CMG.IP" => "Spot Gold ($1)".to_string(),
        "CS.D.GOL.CFD" => "Spot Gold".to_string(),
        "CS.D.XAUUSD.CFD" | "CS.D.GOLDUSD.CFD" => "Gold (XAU/USD)".to_string(),
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Direction {
    #[serde(rename = "BUY")]
    Buy,
    #[serde(rename = "SELL")]
    Sell,
}

impl std::fmt::Display for Direction {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Direction::Buy => write!(f, "BUY"),
            Direction::Sell => write!(f, "SELL"),
        }
    }
}

/// Represents a live open position
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub deal_id: String,
    pub deal_reference: String,
    pub epic: String,
    pub direction: Direction,
    pub size: f64,
    pub open_price: f64,
    pub stop_loss: Option<f64>,
    pub take_profit: Option<f64>,
    pub trailing_stop: Option<f64>,
    pub current_price: f64,
    pub pnl: f64,
    pub currency: String,
    pub strategy: String,
    pub opened_at: DateTime<Utc>,
    pub is_virtual: bool,
    /// ML regime at the moment this position was opened ("TRENDING", "RANGING", "VOLATILE").
    /// Used by management personalities to preserve original stop logic regardless of current regime.
    pub opened_in_regime: Option<String>,
}

/// A closed trade record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClosedTrade {
    pub deal_id: String,
    pub epic: String,
    pub direction: Direction,
    pub size: f64,
    pub entry_price: f64,
    pub exit_price: f64,
    pub stop_loss: f64,
    pub take_profit: Option<f64>,
    pub pnl: f64,
    pub strategy: String,
    pub status: String,
    pub opened_at: DateTime<Utc>,
    pub closed_at: DateTime<Utc>,
    pub is_virtual: bool,
    /// ML regime at position open — persisted to trades.jsonl for genetic P&L analysis (Phase 13.3).
    pub opened_in_regime: Option<String>,
}

/// A strategy signal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Signal {
    pub id: String,
    pub epic: String,
    pub direction: Direction,
    pub strength: f64,
    pub strategy: String,
    pub reason: String,
    pub price: f64,
    pub stop_loss: f64,
    pub take_profit: f64,
    pub trailing_stop_distance: Option<f64>,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalRecord {
    pub signal: Signal,
    pub was_executed: bool,
    pub rejection_reason: Option<String>,
}

/// --- SUB-STATES ---

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AccountState {
    pub balance: f64,
    pub available: f64,
    pub margin: f64,
    pub equity: f64,
    pub pnl: f64,
    pub deposit: f64,
    pub currency: String,
}

/// H1-level directional bias per epic, updated on each H1 bar close analysis.
/// Used as a gate to prevent M15 entries against the prevailing H1 direction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct H1DirectionBias {
    /// Net direction from raw H1 strategy signals (None = tied or no signals).
    pub direction: Option<Direction>,
    pub buy_count: usize,
    pub sell_count: usize,
    pub updated_at: DateTime<Utc>,
}

pub struct MarketStateContainer {
    pub live: HashMap<String, MarketState>,
    pub indicators: HashMap<String, HashMap<String, IndicatorSet>>, // Epic -> Timeframe -> IndicatorSet
    pub history: CandleStore,
    /// Accumulates WS ticks into H1 OHLCV bars; pushes completed bars to history + indicators.
    pub bar_accumulator: BarAccumulator,
    /// Accumulates WS ticks into M15 OHLCV bars; pushes completed bars to history + indicators.
    pub bar_accumulator_m15: BarAccumulator,
    /// Latest H1 directional bias per epic — written by analyze_market(), read by analyze_market_m15().
    pub h1_bias: HashMap<String, H1DirectionBias>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketState {
    pub epic: String,
    pub bid: f64,
    pub ask: f64,
    pub spread: f64,
    pub high: f64,
    pub low: f64,
    pub change_pct: f64,
    /// IG MARKET_STATE field: "TRADEABLE", "EDIT", "OFFLINE", "CLOSED", etc.
    /// None means the field has not yet arrived from Lightstreamer.
    pub market_state: Option<String>,
    pub last_update: DateTime<Utc>,
    /// Rolling EMA of historical spread (α=0.05). Used by the Dynamic Spread Gate (12.3)
    /// to reject trades when live spread is unusually wide (> 1.5× this baseline).
    pub avg_spread: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TradeState {
    pub active: Vec<Position>,
    pub signals: VecDeque<Signal>,
    pub signal_records: VecDeque<SignalRecord>,
    pub history: VecDeque<ClosedTrade>,
    /// Per-epic cooldown: blocks re-entry until the stored timestamp passes.
    /// Set when any position closes (TP or SL) to prevent immediate re-entry
    /// while price is potentially reversing.
    #[serde(default)]
    pub cooldowns: HashMap<String, DateTime<Utc>>,
    /// Deal IDs of positions closed in this session (OPU deduplication).
    /// Lightstreamer can replay the last OPU on reconnect — this set prevents
    /// double-processing and double Telegram notifications.
    #[serde(skip)]
    pub recently_closed_deal_ids: std::collections::HashSet<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MetricsState {
    pub daily: DailyStats,
    pub circuit_breaker_active: bool,
    pub circuit_breaker_until: Option<DateTime<Utc>>,
    /// Set when a sentiment velocity spike is detected (delta > 0.5 in 15 min).
    /// All new trade entries are blocked until this timestamp passes (2-hour macro pause).
    pub macro_pause_until: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DailyStats {
    pub date: String,
    pub trades: u32,
    pub wins: u32,
    pub losses: u32,
    pub pnl: f64,
    pub max_drawdown: f64,
    pub high_watermark: f64,
    pub consecutive_losses: u32,
    pub consecutive_wins: u32,
    /// Per-instrument trade count for the day. Reset alongside `trades` on daily rollover.
    #[serde(default)]
    pub trades_by_epic: HashMap<String, u32>,
    /// Net overnight financing P&L for today (fetched from IG /history/transactions).
    /// Positive = credit received, negative = charge paid. Separate from trade P&L.
    #[serde(default)]
    pub financing_pnl: f64,
}

pub struct LearningState {
    pub snapshot: LearningSnapshot,
    pub scorecard: Option<StrategyScorecard>,
    pub weight_manager: Option<AdaptiveWeightManager>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LearningSnapshot {
    pub total_trades_processed: u64,
    pub strategies: Vec<StrategyLearningEntry>,
    pub recent_adjustments: Vec<WeightAdjustment>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyLearningEntry {
    pub name: String,
    pub win_rate: f64,
    pub profit_factor: f64,
    pub current_multiplier: f64,
    pub effective_weight: f64,
    pub max_consecutive_losses: u32,
    pub trades_in_window: usize,
    pub sessions: HashMap<String, SessionStats>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SessionStats {
    pub win_rate: f64,
    pub profit_factor: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SessionState {
    pub ig_session_token: Option<String>,
    pub ig_security_token: Option<String>,
}

use crate::data::trade_logger::TradeLogger;

/// Tracks M15 trade count per H1 candle boundary per epic, plus the last
/// entry time per epic for minimum same-instrument entry spacing.
/// Prevents overtrading: max `max_trades` M15 trades within a single H1
/// candle, and (Phase 17.G) no two entries on one instrument closer than the
/// configured spacing — live data showed entries stacked 1–15 min apart
/// consistently winning or dying together (same signal, same noise band),
/// doubling risk without diversifying it.
pub struct M15CooldownTracker {
    /// epic -> (h1_candle_start_ts, trade_count)
    pub trades_per_h1_candle: HashMap<String, (i64, u32)>,
    /// epic -> unix ts of the most recent entry attempt
    pub last_entry_ts: HashMap<String, i64>,
}

impl Default for M15CooldownTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl M15CooldownTracker {
    pub fn new() -> Self {
        Self {
            trades_per_h1_candle: HashMap::new(),
            last_entry_ts: HashMap::new(),
        }
    }

    /// Returns true if a new M15 trade is allowed for this epic in the current H1 candle.
    pub fn can_trade(&self, epic: &str, h1_ts: i64, max_trades: u32) -> bool {
        match self.trades_per_h1_candle.get(epic) {
            Some((ts, count)) if *ts == h1_ts => *count < max_trades,
            _ => true,
        }
    }

    /// Seconds since the last recorded entry on this epic; None if no entry yet.
    pub fn secs_since_last_entry(&self, epic: &str, now_ts: i64) -> Option<i64> {
        self.last_entry_ts.get(epic).map(|ts| now_ts - ts)
    }

    /// Record a new M15 trade for this epic in the current H1 candle.
    pub fn record_trade(&mut self, epic: &str, h1_ts: i64, now_ts: i64) {
        let entry = self
            .trades_per_h1_candle
            .entry(epic.to_string())
            .or_insert((h1_ts, 0));
        if entry.0 != h1_ts {
            *entry = (h1_ts, 1);
        } else {
            entry.1 += 1;
        }
        self.last_entry_ts.insert(epic.to_string(), now_ts);
    }
}

/// The complete engine state — decoupled into domain-specific sub-states
pub struct EngineState {
    pub config: EngineConfig,
    pub status: EngineStatus,
    pub started_at: Option<DateTime<Utc>>,

    pub account: AccountState,
    pub markets: MarketStateContainer,
    pub trades: TradeState,
    pub metrics: MetricsState,
    pub learning: LearningState,
    pub session: SessionState,
    pub trade_logger: TradeLogger,
    /// Live IG crowd-sentiment data, updated every 15 minutes for all `context_market_ids`.
    pub sentiment: GlobalSentimentRegistry,
    /// M15 trade cooldown tracker — limits M15 trades per H1 candle boundary.
    pub m15_cooldown: M15CooldownTracker,
}

impl EngineState {
    pub fn new(config: EngineConfig) -> Self {
        let mut indicators = HashMap::new();
        for epic in &config.markets.epics {
            let mut tf_map = HashMap::new();
            tf_map.insert("HOUR".to_string(), IndicatorSet::default_config());

            // If multi-timeframe is enabled, initialize those timeframes as well
            if let Some(mtf) = &config.strategies.multi_timeframe {
                if mtf.enabled {
                    tf_map.insert(mtf.trend_tf.clone(), IndicatorSet::default_config());
                    tf_map.insert(mtf.signal_tf.clone(), IndicatorSet::default_config());
                    tf_map.insert(mtf.entry_tf.clone(), IndicatorSet::default_config());
                }
            }

            indicators.insert(epic.clone(), tf_map);
        }

        let is_paper = config.general.mode == crate::engine::config::EngineMode::Paper;
        let account = if is_paper {
            AccountState {
                balance: 10000.0,
                available: 10000.0,
                equity: 10000.0,
                currency: "USD".to_string(),
                ..Default::default()
            }
        } else {
            AccountState::default()
        };

        Self {
            config,
            status: EngineStatus::Starting,
            started_at: None,
            account,
            markets: MarketStateContainer {
                live: HashMap::new(),
                indicators,
                history: CandleStore::new(),
                bar_accumulator: BarAccumulator::new(3600), // 1-hour bars
                bar_accumulator_m15: BarAccumulator::new(900), // 15-minute bars
                h1_bias: HashMap::new(),
            },
            trades: TradeState {
                active: Vec::new(),
                signals: VecDeque::new(),
                signal_records: VecDeque::new(),
                history: VecDeque::new(),
                cooldowns: HashMap::new(),
                recently_closed_deal_ids: std::collections::HashSet::new(),
            },
            metrics: MetricsState {
                daily: DailyStats {
                    date: Utc::now().format("%Y-%m-%d").to_string(),
                    ..Default::default()
                },
                circuit_breaker_active: false,
                circuit_breaker_until: None,
                macro_pause_until: None,
            },
            learning: LearningState {
                snapshot: LearningSnapshot::default(),
                scorecard: None,
                weight_manager: None,
            },
            session: SessionState::default(),
            trade_logger: TradeLogger::default(),
            sentiment: GlobalSentimentRegistry::new(),
            m15_cooldown: M15CooldownTracker::new(),
        }
    }

    pub fn is_running(&self) -> bool {
        self.status == EngineStatus::Running
    }

    pub fn can_trade(&self) -> bool {
        self.status == EngineStatus::Running && !self.metrics.circuit_breaker_active
    }

    /// Hot-reload non-risk strategy parameters (triggered by SIGUSR1).
    ///
    /// Only updates: `instrument_overrides`, `min_consensus`, `min_avg_strength`.
    /// Risk parameters (`max_risk_per_trade`, `max_daily_loss_pct`, etc.) are
    /// intentionally never modified by hot-reload — those require a full restart.
    pub fn reload_strategy_config(
        &mut self,
        new_strategies: crate::engine::config::StrategiesConfig,
    ) {
        self.config.strategies.instrument_overrides = new_strategies.instrument_overrides;
        self.config.strategies.min_consensus = new_strategies.min_consensus;
        self.config.strategies.min_avg_strength = new_strategies.min_avg_strength;
        tracing::info!(
            "Strategy config hot-reloaded: instrument_overrides + consensus thresholds updated"
        );
    }

    pub fn check_daily_reset(&mut self) {
        let today = Utc::now().format("%Y-%m-%d").to_string();
        if self.metrics.daily.date != today {
            self.metrics.daily = DailyStats {
                date: today,
                ..Default::default()
            };
        }
    }

    pub fn record_trade_result(&mut self, pnl: f64) {
        self.record_trade_result_for_epic(pnl, None);
    }

    pub fn record_trade_result_for_epic(&mut self, pnl: f64, epic: Option<&str>) {
        self.metrics.daily.trades += 1;
        if let Some(e) = epic {
            *self
                .metrics
                .daily
                .trades_by_epic
                .entry(e.to_string())
                .or_insert(0) += 1;
        }
        self.metrics.daily.pnl += pnl;

        if self.config.general.mode == crate::engine::config::EngineMode::Paper {
            self.account.balance += pnl;
            self.account.equity += pnl;
            self.account.available = self.account.balance;
        }

        if pnl > 0.0 {
            self.metrics.daily.wins += 1;
            self.metrics.daily.consecutive_wins += 1;
            self.metrics.daily.consecutive_losses = 0;
        } else {
            self.metrics.daily.losses += 1;
            self.metrics.daily.consecutive_losses += 1;
            self.metrics.daily.consecutive_wins = 0;
        }

        if self.metrics.daily.pnl > self.metrics.daily.high_watermark {
            self.metrics.daily.high_watermark = self.metrics.daily.pnl;
        }
        let drawdown = self.metrics.daily.high_watermark - self.metrics.daily.pnl;
        if drawdown > self.metrics.daily.max_drawdown {
            self.metrics.daily.max_drawdown = drawdown;
        }
    }

    pub fn add_signal(&mut self, signal: Signal) {
        self.trades.signals.push_back(signal);
        if self.trades.signals.len() > 200 {
            self.trades.signals.pop_front();
        }
    }

    pub fn add_signal_record(
        &mut self,
        signal: Signal,
        was_executed: bool,
        rejection_reason: Option<String>,
    ) {
        self.trades.signal_records.push_back(SignalRecord {
            signal,
            was_executed,
            rejection_reason,
        });
        if self.trades.signal_records.len() > 200 {
            self.trades.signal_records.pop_front();
        }
    }

    pub fn add_closed_trade(&mut self, trade: ClosedTrade) {
        // Log the trade outcome to structured JSONL for Task 8.6
        if let Err(e) = self.trade_logger.log_trade(&trade) {
            tracing::error!("Failed to log trade to JSONL: {}", e);
        }

        self.trades.history.push_back(trade);
        if self.trades.history.len() > 500 {
            self.trades.history.pop_front();
        }
    }

    /// Set a re-entry cooldown for the given epic.
    /// Call this whenever a position closes (TP or SL hit).
    pub fn set_trade_cooldown(&mut self, epic: &str, cooldown_secs: u64) {
        if cooldown_secs == 0 {
            return;
        }
        let until = Utc::now() + chrono::Duration::seconds(cooldown_secs as i64);
        self.trades.cooldowns.insert(epic.to_string(), until);
        tracing::info!(
            "[{}] Re-entry cooldown set — blocked for {}s until {}",
            epic,
            cooldown_secs,
            until.format("%H:%M:%S UTC")
        );
    }

    /// Returns true if the epic is currently in cooldown (re-entry blocked).
    pub fn is_in_cooldown(&self, epic: &str) -> bool {
        self.trades
            .cooldowns
            .get(epic)
            .map(|&until| Utc::now() < until)
            .unwrap_or(false)
    }

    /// Persist active positions to disk for crash recovery.
    /// Written every monitor tick (~5s); read on next startup to detect
    /// positions that closed while the engine was offline.
    pub fn save_active_positions(&self) {
        let path = "data/recovery/active_positions.json";
        if let Some(dir) = std::path::Path::new(path).parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        match serde_json::to_string_pretty(&self.trades.active) {
            Ok(json) => {
                if let Err(e) = std::fs::write(path, json) {
                    tracing::debug!("Failed to persist active positions: {}", e);
                }
            }
            Err(e) => tracing::debug!("Failed to serialize active positions: {}", e),
        }
    }

    /// Load active positions persisted from the previous session.
    /// Returns an empty vec if the file does not exist or cannot be parsed.
    pub fn load_persisted_positions() -> Vec<Position> {
        let path = "data/recovery/active_positions.json";
        std::fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str::<Vec<Position>>(&s).ok())
            .unwrap_or_default()
    }

    /// Persist today's DailyStats to disk so engine restarts don't wipe the day's P&L.
    /// Written to data/recovery/daily_stats.json on every trade close.
    pub fn save_daily_stats(&self) {
        let path = "data/recovery/daily_stats.json";
        if let Some(dir) = std::path::Path::new(path).parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        match serde_json::to_string_pretty(&self.metrics.daily) {
            Ok(json) => {
                if let Err(e) = std::fs::write(path, json) {
                    tracing::warn!("Failed to persist daily stats: {}", e);
                }
            }
            Err(e) => tracing::warn!("Failed to serialize daily stats: {}", e),
        }
    }

    /// Load persisted DailyStats from disk.
    /// Returns None if the file is missing, unparseable, or from a different UTC date.
    pub fn load_persisted_daily_stats() -> Option<DailyStats> {
        let path = "data/recovery/daily_stats.json";
        let today = Utc::now().format("%Y-%m-%d").to_string();
        std::fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str::<DailyStats>(&s).ok())
            .filter(|stats| stats.date == today)
    }
}

#[cfg(test)]
mod m15_cooldown_tests {
    use super::M15CooldownTracker;

    #[test]
    fn entry_spacing_tracks_per_epic() {
        let mut t = M15CooldownTracker::new();
        let h1 = 1_700_000_000_i64 / 3600 * 3600;

        assert_eq!(t.secs_since_last_entry("EPIC.A", 1_700_000_000), None);
        t.record_trade("EPIC.A", h1, 1_700_000_000);
        // 60s later: well inside a 2700s minimum spacing window
        assert_eq!(t.secs_since_last_entry("EPIC.A", 1_700_000_060), Some(60));
        // other epics are unaffected
        assert_eq!(t.secs_since_last_entry("EPIC.B", 1_700_000_060), None);
        // 45 min later the window has elapsed
        assert_eq!(
            t.secs_since_last_entry("EPIC.A", 1_700_000_000 + 2700),
            Some(2700)
        );
    }

    #[test]
    fn h1_candle_counter_still_enforced() {
        let mut t = M15CooldownTracker::new();
        let h1 = 1_700_000_000_i64 / 3600 * 3600;
        assert!(t.can_trade("EPIC.A", h1, 2));
        t.record_trade("EPIC.A", h1, 1_700_000_000);
        assert!(t.can_trade("EPIC.A", h1, 2));
        t.record_trade("EPIC.A", h1, 1_700_000_100);
        assert!(!t.can_trade("EPIC.A", h1, 2));
        // new H1 candle resets the counter
        assert!(t.can_trade("EPIC.A", h1 + 3600, 2));
    }
}
