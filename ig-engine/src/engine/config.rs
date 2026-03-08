use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use anyhow::Result;
use crate::risk::{RiskConfig, SizingMethod, CircuitBreakerConfig};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineConfig {
    pub general: GeneralConfig,
    pub ig: IGConfig,
    pub markets: MarketsConfig,
    pub risk: RiskConfig,
    pub strategies: StrategiesConfig,
    pub trading_hours: TradingHoursConfig,
    pub notifications: NotificationsConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    pub mode: EngineMode,
    pub log_level: String,
    pub heartbeat_interval_secs: u64,
    pub api_port: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum EngineMode {
    Paper,
    Demo,
    Live,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IGConfig {
    pub environment: String, // "demo" or "live"
    pub session_refresh_mins: u64,
    pub rate_limit_per_minute: u32,
    pub confirm_timeout_ms: u64,
    pub confirm_max_retries: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketsConfig {
    pub epics: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategiesConfig {
    pub min_consensus: usize,
    pub min_avg_strength: f64,
    pub ma_crossover: Option<MACrossoverConfig>,
    pub rsi_divergence: Option<RSIDivergenceConfig>,
    pub macd_momentum: Option<MACDMomentumConfig>,
    pub bollinger_reversion: Option<BollingerReversionConfig>,
    pub multi_timeframe: Option<MultiTimeframeConfig>,
    pub default_atr_sl_multiplier: f64,
    pub default_atr_tp_multiplier: f64,
    /// Per-instrument strategy overrides keyed by IG epic string.
    /// Allows Gold and FX pairs to use different strategy filtering and risk params.
    #[serde(default)]
    pub instrument_overrides: HashMap<String, InstrumentStrategyOverride>,
}

/// Per-instrument strategy configuration.
/// Gold trends strongly → ADX range filter keeps mean-reversion signals rare.
/// FX pairs range more → ADX range filter prevents false mean-reversion in trends.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct InstrumentStrategyOverride {
    /// Skip RSI_Reversal and Bollinger_Bands signals when ADX > adx_range_max.
    /// Prevents mean-reversion entries in strongly trending markets.
    #[serde(default)]
    pub adx_range_filter: bool,
    /// ADX threshold above which mean-reversion strategies are suppressed (default 25.0).
    pub adx_range_max: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MACrossoverConfig {
    pub enabled: bool,
    pub weight: f64,
    pub short_period: usize,
    pub long_period: usize,
    pub trend_filter_period: usize,
    pub require_adx_above: f64,
    pub atr_sl_multiplier: Option<f64>,
    pub atr_tp_multiplier: Option<f64>,
    pub trailing_stop_pips: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RSIDivergenceConfig {
    pub enabled: bool,
    pub weight: f64,
    pub period: usize,
    pub overbought: f64,
    pub oversold: f64,
    pub detect_divergence: bool,
    pub atr_sl_multiplier: Option<f64>,
    pub atr_tp_multiplier: Option<f64>,
    pub trailing_stop_pips: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MACDMomentumConfig {
    pub enabled: bool,
    pub weight: f64,
    pub fast: usize,
    pub slow: usize,
    pub signal: usize,
    pub atr_sl_multiplier: Option<f64>,
    pub atr_tp_multiplier: Option<f64>,
    pub trailing_stop_pips: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BollingerReversionConfig {
    pub enabled: bool,
    pub weight: f64,
    pub period: usize,
    pub std_dev: f64,
    pub atr_sl_multiplier: Option<f64>,
    pub atr_tp_multiplier: Option<f64>,
    pub trailing_stop_pips: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiTimeframeConfig {
    pub enabled: bool,
    pub weight: f64,
    pub trend_tf: String,
    pub signal_tf: String,
    pub entry_tf: String,
    pub trailing_stop_pips: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradingHoursConfig {
    pub start: String,
    pub end: String,
    pub timezone: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationsConfig {
    pub telegram: Option<TelegramConfig>,
    pub slack: Option<SlackConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramConfig {
    pub enabled: bool,
    pub trade_alerts: bool,
    pub risk_alerts: bool,
    pub daily_summary: bool,
    pub summary_time: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackConfig {
    pub enabled: bool,
    pub webhook_url: String,
}

impl EngineConfig {
    pub fn load(path: &str) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: EngineConfig = toml::from_str(&content)?;
        Ok(config)
    }

    /// Load from environment variables (for Docker)
    pub fn from_env() -> Result<Self> {
        let path = std::env::var("CONFIG_PATH")
            .unwrap_or_else(|_| "config/default.toml".to_string());
        Self::load(&path)
    }
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            general: GeneralConfig {
                mode: EngineMode::Paper,
                log_level: "info".to_string(),
                heartbeat_interval_secs: 30,
                api_port: Some(9090),
            },
            ig: IGConfig {
                environment: "demo".to_string(),
                session_refresh_mins: 50,
                rate_limit_per_minute: 30,
                confirm_timeout_ms: 5000,
                confirm_max_retries: 10,
            },
            markets: MarketsConfig {
                epics: vec![
                    "CS.D.EURUSD.CSD.IP".to_string(),
                    "CS.D.USDJPY.CSD.IP".to_string(),
                    "CS.D.CFIGOLD.CFI.IP".to_string(),
                ],
            },
            risk: RiskConfig {
                max_risk_per_trade: 1.0,
                max_daily_loss_pct: 3.0,
                max_weekly_drawdown_pct: 5.0,
                max_daily_trades: 20,
                max_open_positions: 3,
                max_correlated_positions: 1,
                max_margin_usage_pct: 30.0,
                min_risk_reward: 1.5,
                sizing_method: SizingMethod::HalfKelly,
                instrument_specs: HashMap::new(),
                circuit_breaker: CircuitBreakerConfig {
                    consecutive_losses_reduce: 3,
                    consecutive_losses_pause: 5,
                    pause_duration_mins: 60,
                    daily_loss_warning_pct: 70.0,
                },
                trading_hours_utc: Some((0, 16)),
                limited_risk_account: true,
                min_guaranteed_stop_distance: None,
                use_trailing_stop: false,
                allowed_sessions: vec![
                    crate::engine::state::Session::Asia,
                    crate::engine::state::Session::London,
                    crate::engine::state::Session::UsOverlap,
                ],
                news_blackout_windows_utc: vec![(8, 30), (13, 30), (15, 0)],
                news_blackout_mins: 15,
                macro_events: vec![], // default.toml populates these via TOML; overridden on load
            },
            strategies: StrategiesConfig {
                min_consensus: 2,
                min_avg_strength: 6.0,
                default_atr_sl_multiplier: 2.0,
                default_atr_tp_multiplier: 3.0,
                ma_crossover: Some(MACrossoverConfig {
                    enabled: true,
                    weight: 1.0,
                    short_period: 9,
                    long_period: 21,
                    trend_filter_period: 200,
                    require_adx_above: 25.0,
                    atr_sl_multiplier: None,
                    atr_tp_multiplier: None,
                    trailing_stop_pips: None,
                }),
                rsi_divergence: Some(RSIDivergenceConfig {
                    enabled: true,
                    weight: 0.9,
                    period: 14,
                    overbought: 70.0,
                    oversold: 30.0,
                    detect_divergence: true,
                    atr_sl_multiplier: None,
                    atr_tp_multiplier: None,
                    trailing_stop_pips: None,
                }),
                macd_momentum: Some(MACDMomentumConfig {
                    enabled: true,
                    weight: 1.0,
                    fast: 12,
                    slow: 26,
                    signal: 9,
                    atr_sl_multiplier: None,
                    atr_tp_multiplier: None,
                    trailing_stop_pips: None,
                }),
                bollinger_reversion: Some(BollingerReversionConfig {
                    enabled: true,
                    weight: 0.8,
                    period: 20,
                    std_dev: 2.0,
                    atr_sl_multiplier: None,
                    atr_tp_multiplier: None,
                    trailing_stop_pips: None,
                }),
                multi_timeframe: None,
                instrument_overrides: {
                    let mut m = HashMap::new();
                    // Gold and FX all use ADX range filter (suppress mean-reversion in trends)
                    for epic in &[
                        "CS.D.CFIGOLD.CFI.IP",
                        "CS.D.EURUSD.CSD.IP",
                        "CS.D.USDJPY.CSD.IP",
                    ] {
                        m.insert(epic.to_string(), InstrumentStrategyOverride {
                            adx_range_filter: true,
                            adx_range_max: Some(25.0),
                        });
                    }
                    m
                },
            },
            trading_hours: TradingHoursConfig {
                start: "07:00".to_string(),
                end: "20:00".to_string(),
                timezone: "UTC".to_string(),
            },
            notifications: NotificationsConfig {
                telegram: Some(TelegramConfig {
                    enabled: false,
                    trade_alerts: true,
                    risk_alerts: true,
                    daily_summary: true,
                    summary_time: "21:00".to_string(),
                }),
                slack: None,
            },
        }
    }
}
