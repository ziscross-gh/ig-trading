#![allow(dead_code)]
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use chrono::{DateTime, Utc};

use crate::engine::config::EngineConfig;
use crate::indicators::IndicatorSet;
use crate::data::candle_store::CandleStore;
use crate::data::bar_accumulator::BarAccumulator;
use crate::learning::adaptive_weights::{WeightAdjustment, AdaptiveWeightManager};
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
    pub strategy: String,
    pub opened_at: DateTime<Utc>,
    pub is_virtual: bool,
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

pub struct MarketStateContainer {
    pub live: HashMap<String, MarketState>,
    pub indicators: HashMap<String, HashMap<String, IndicatorSet>>, // Epic -> Timeframe -> IndicatorSet
    pub history: CandleStore,
    /// Accumulates WS ticks into OHLCV bars; pushes completed bars to history + indicators.
    pub bar_accumulator: BarAccumulator,
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
}

pub struct TradeState {
    pub active: Vec<Position>,
    pub signals: VecDeque<Signal>,
    pub signal_records: VecDeque<SignalRecord>,
    pub history: VecDeque<ClosedTrade>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MetricsState {
    pub daily: DailyStats,
    pub circuit_breaker_active: bool,
    pub circuit_breaker_until: Option<DateTime<Utc>>,
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
            },
            trades: TradeState {
                active: Vec::new(),
                signals: VecDeque::new(),
                signal_records: VecDeque::new(),
                history: VecDeque::new(),
            },
            metrics: MetricsState {
                daily: DailyStats {
                    date: Utc::now().format("%Y-%m-%d").to_string(),
                    ..Default::default()
                },
                circuit_breaker_active: false,
                circuit_breaker_until: None,
            },
            learning: LearningState {
                snapshot: LearningSnapshot::default(),
                scorecard: None,
                weight_manager: None,
            },
            session: SessionState::default(),
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
    pub fn reload_strategy_config(&mut self, new_strategies: crate::engine::config::StrategiesConfig) {
        self.config.strategies.instrument_overrides = new_strategies.instrument_overrides;
        self.config.strategies.min_consensus        = new_strategies.min_consensus;
        self.config.strategies.min_avg_strength     = new_strategies.min_avg_strength;
        tracing::info!("Strategy config hot-reloaded: instrument_overrides + consensus thresholds updated");
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
        self.metrics.daily.trades += 1;
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

    pub fn add_signal_record(&mut self, signal: Signal, was_executed: bool, rejection_reason: Option<String>) {
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
        self.trades.history.push_back(trade);
        if self.trades.history.len() > 500 {
            self.trades.history.pop_front();
        }
    }
}
