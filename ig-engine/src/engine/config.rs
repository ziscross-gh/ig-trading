use crate::risk::{CircuitBreakerConfig, RiskConfig, SizingMethod};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

fn default_m15_min_consensus() -> usize {
    2
}
fn default_m15_min_avg_strength() -> f64 {
    6.5
}
fn default_m15_position_multiplier() -> f64 {
    0.5
}
fn default_m15_max_trades() -> u32 {
    2
}
fn default_h1_direction_gate() -> bool {
    true
}
fn default_h1_alignment_bonus() -> f64 {
    1.2
}
fn default_h1_macro_trend_gate() -> bool {
    true
}
fn default_h1_macro_trend_lookback() -> usize {
    5
}
fn default_volatile_atr_sl_multiplier() -> f64 {
    1.0
}
fn default_volatile_atr_tp_multiplier() -> f64 {
    2.5
}
fn default_post_trade_cooldown_secs() -> u64 {
    1800
} // 30 min = 2 M15 bars
fn default_m15_min_entry_spacing_secs() -> i64 {
    2700
} // 45 min — stacked same-instrument entries die together (Phase 17.G)
fn default_require_h1_confirmation() -> bool {
    true
}
fn default_regime_cooldown_days() -> Option<u64> {
    Some(7)
}
fn default_regime_cooldown_sl_multiplier() -> Option<f64> {
    Some(1.25)
}
fn default_regime_cooldown_tp_multiplier() -> Option<f64> {
    Some(3.0)
}
fn default_regime_cooldown_disable_be_snap() -> Option<bool> {
    Some(true)
}
fn default_ensemble_signal_floor() -> f64 {
    5.0
}

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
    /// Weekend-specific epics (e.g. IG's Sunday Gold spot epic) used when
    /// `allow_weekend_trading = true` and the regular epics are offline.
    #[serde(default)]
    pub weekend_epics: Vec<String>,
    /// Epics or market IDs to poll for sentiment context only (no trading).
    /// E.g. "USDIND" for the USD Index as a Gold driver.
    #[serde(default)]
    pub context_market_ids: Vec<String>,
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
    pub stochastic_momentum: Option<StochasticMomentumConfig>,
    #[serde(default)]
    pub m15_momentum_burst: Option<M15MomentumBurstConfig>,
    #[serde(default)]
    pub m15_ema_microtrend: Option<M15EmaMicrotrendConfig>,
    #[serde(default)]
    pub m15_bollinger_reversion: Option<M15BollingerReversionConfig>,
    #[serde(default = "default_m15_min_consensus")]
    pub m15_min_consensus: usize,
    #[serde(default = "default_m15_min_avg_strength")]
    pub m15_min_avg_strength: f64,
    #[serde(default = "default_m15_position_multiplier")]
    pub m15_position_size_multiplier: f64,
    #[serde(default = "default_m15_max_trades")]
    pub m15_max_trades_per_h1: u32,
    /// Minimum seconds between entries on the same instrument (M15 path).
    /// Live evidence (06-10/06-11): entries 1–15 min apart on one epic always
    /// shared the same fate — stacked risk, no diversification. 0 disables.
    #[serde(default = "default_m15_min_entry_spacing_secs")]
    pub m15_min_entry_spacing_secs: i64,
    pub default_atr_sl_multiplier: f64,
    pub default_atr_tp_multiplier: f64,
    /// Per-instrument strategy overrides keyed by IG epic string.
    /// Allows Gold and FX pairs to use different strategy filtering and risk params.
    #[serde(default)]
    pub instrument_overrides: HashMap<String, InstrumentStrategyOverride>,
    /// Regime-specific consensus overrides: different barrier + min_strength per market state.
    /// Keys: "trending", "ranging", "volatile". Used by Phase 12.1 regime-switching logic.
    #[serde(default)]
    pub consensus_matrix: HashMap<String, ConsensusMatrixEntry>,
    /// Per-epic strategy overrides for weekend trading (e.g. Sunday Gold spot epic).
    /// Applied when the active epic is a weekend_epic from MarketsConfig.
    #[serde(default)]
    pub weekend_overrides: HashMap<String, WeekendOverride>,
    /// Block M15 entries that contradict the prevailing H1 directional bias.
    /// If any H1 strategy leans BUY, M15 SELL entries are rejected (and vice versa).
    /// Disable to allow M15 to trade freely regardless of H1 direction.
    #[serde(default = "default_h1_direction_gate")]
    pub h1_direction_gate_enabled: bool,
    /// Strength multiplier applied to M15 signals that align with the H1 directional bias.
    /// E.g. 1.2 boosts a 7.0-strength aligned signal to 8.4, helping clear avg_strength threshold.
    /// Applied per-signal before ensemble vote. Set to 1.0 to disable.
    #[serde(default = "default_h1_alignment_bonus")]
    pub h1_alignment_bonus: f64,
    /// Block H1 entries that trade against the recent H1 price trend.
    /// Uses linear regression slope of the last N H1 bar closes:
    ///   positive slope → uptrend → block SELL
    ///   negative slope → downtrend → block BUY
    /// This prevents counter-trend H1 entries during strong directional moves.
    #[serde(default = "default_h1_macro_trend_gate")]
    pub h1_macro_trend_gate_enabled: bool,
    /// Number of recent H1 closes to use for slope computation (default: 5).
    #[serde(default = "default_h1_macro_trend_lookback")]
    pub h1_macro_trend_lookback: usize,
    /// ATR SL multiplier used for H1 ensemble signals in VOLATILE regime.
    /// Wider stop (1.0× ATR) reduces premature SL hits in VOLATILE swings.
    /// Must preserve R:R >= 2.5: volatile_atr_tp / volatile_atr_sl >= 2.5.
    #[serde(default = "default_volatile_atr_sl_multiplier")]
    pub volatile_atr_sl_multiplier: f64,
    /// ATR TP multiplier used for H1 ensemble signals in VOLATILE regime.
    /// 2.5× ATR gives R:R = 2.5 at the wider SL (vs 4.0× which is rarely hit).
    #[serde(default = "default_volatile_atr_tp_multiplier")]
    pub volatile_atr_tp_multiplier: f64,
    /// Seconds to block re-entry on the same epic after any trade closes (TP or SL).
    /// Prevents the engine from immediately re-entering after a TP when price is reversing.
    /// Default: 1800s (30 min = 2 M15 bars). Set to 0 to disable.
    #[serde(default = "default_post_trade_cooldown_secs")]
    pub post_trade_cooldown_secs: u64,
    /// When true, block ALL M15 entries until H1 has completed at least one analysis
    /// cycle with at least one strategy signal. Prevents counter-trend M15 trades during
    /// the cold-start window (first 0-59 min after engine start) when H1 direction gate
    /// has no data. Default: true.
    #[serde(default = "default_require_h1_confirmation")]
    pub require_h1_confirmation: bool,

    // ── Regime cooldown ─────────────────────────────────────────────────────
    // When VOLATILE persists for many days, the tight SL/TP and BE snap become
    // the permanent (losing) default. After `regime_cooldown_days`, relax the
    // VOLATILE restrictions to intermediate values between VOLATILE and normal.
    /// Days before VOLATILE restrictions start relaxing (None = disabled).
    #[serde(default = "default_regime_cooldown_days")]
    pub regime_cooldown_days: Option<u64>,
    /// SL multiplier after cooldown kicks in (between VOLATILE 1.0 and normal 1.5).
    #[serde(default = "default_regime_cooldown_sl_multiplier")]
    pub regime_cooldown_sl_multiplier: Option<f64>,
    /// TP multiplier after cooldown kicks in (between VOLATILE 2.5 and normal 4.0).
    #[serde(default = "default_regime_cooldown_tp_multiplier")]
    pub regime_cooldown_tp_multiplier: Option<f64>,
    /// Whether to disable the breakeven snap after cooldown (let trailing stop handle risk).
    #[serde(default = "default_regime_cooldown_disable_be_snap")]
    pub regime_cooldown_disable_be_snap: Option<bool>,

    // ── Ensemble signal floor ────────────────────────────────────────────────
    // After regime multipliers crush a strategy's signal (e.g. MA_Crossover 8.0
    // × 0.3 = 2.4 in TRENDING), that crushed signal poisons the ensemble average.
    // Signals below this floor are excluded from the consensus count AND the
    // average strength calculation — effectively "this strategy is unreliable in
    // this regime, don't let it vote". Default 5.0; set to 0.0 to disable.
    /// Minimum post-multiplier signal strength to be included in ensemble voting.
    #[serde(default = "default_ensemble_signal_floor")]
    pub ensemble_signal_floor: f64,
}

/// Regime-specific consensus threshold entry (Phase 12.1).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ConsensusMatrixEntry {
    /// Minimum number of strategies that must agree (overrides min_consensus).
    pub barrier: usize,
    /// Minimum average signal strength (overrides min_avg_strength).
    pub min_strength: f64,
}

/// Per-epic strategy overrides used during weekend sessions.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WeekendOverride {
    pub min_consensus: Option<usize>,
    pub min_avg_strength: Option<f64>,
    pub atr_sl_multiplier: Option<f64>,
    pub atr_tp_multiplier: Option<f64>,
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

    // ── Consensus override ───────────────────────────────────────────────────
    /// Override global m15_min_consensus for this instrument.
    /// Gold: 2 (need 2/3 strategies to agree — higher bar than default 1).
    pub min_consensus: Option<usize>,
    /// Override global m15_min_avg_strength for this instrument.
    pub min_avg_strength: Option<f64>,

    // ── ADX trend-lock gate ──────────────────────────────────────────────────
    /// When true and ADX exceeds adx_trend_lock_threshold, block signals that
    /// oppose the DI-dominant direction. Prevents counter-trend entries in
    /// Gold's strong trending moves (ADX=62.9 scenario).
    #[serde(default)]
    pub adx_trend_lock_enabled: bool,
    /// ADX threshold above which the trend-lock activates. Gold: 45.0.
    pub adx_trend_lock_threshold: Option<f64>,

    // ── RSI extreme block ────────────────────────────────────────────────────
    /// Block mean-reversion BUY signals when RSI is below this floor AND
    /// ADX >= rsi_extreme_block_adx_min. Prevents catching a falling knife.
    /// Gold: 15.0 (RSI=8.76 today would trigger immediately).
    pub rsi_extreme_oversold_floor: Option<f64>,
    /// Block mean-reversion SELL signals when RSI is above this ceiling.
    /// Gold: 85.0.
    pub rsi_extreme_overbought_ceiling: Option<f64>,
    /// Minimum ADX required for the RSI extreme block to activate.
    /// Below this ADX the market is ranging — RSI extremes are valid. Gold: 40.0.
    pub rsi_extreme_block_adx_min: Option<f64>,

    // ── Mean-reversion weight suppression ───────────────────────────────────
    /// Multiply RSI_Reversal and Bollinger signal strength by this factor when
    /// ADX >= mean_reversion_suppress_adx_min. 0.0 = fully silenced. Gold: 0.0.
    pub mean_reversion_weight_in_strong_trend: Option<f64>,
    /// ADX threshold above which mean-reversion suppression kicks in. Gold: 45.0.
    pub mean_reversion_suppress_adx_min: Option<f64>,

    // ── Per-instrument daily trade limit ────────────────────────────────────
    /// Max trades per day for this instrument. Gold: 2. EUR/USD: 5. None = global limit.
    pub max_daily_trades: Option<u32>,

    // ── Volatility ceiling ───────────────────────────────────────────────────
    /// Block all entries if ATR% (atr/price * 100) exceeds this value.
    /// Gold: 1.8 (blocks truly chaotic conditions; current ~1.0% is fine).
    pub atr_pct_max_entry: Option<f64>,

    // ── M15 SL/TP override (whipsaw protection) ─────────────────────────────
    /// Recompute the M15 ensemble signal's stop loss as `M15 ATR × this` before
    /// the risk gate. EUR/USD: 2.5 — the strategy default (1.5× ≈ 5–6 pips) sat
    /// inside spread noise and every first-live-week loss was a whipsaw stop-out.
    pub m15_atr_sl_multiplier: Option<f64>,
    /// Recompute the M15 take profit as `M15 ATR × this`. Must stay ≥
    /// min_risk_reward × m15_atr_sl_multiplier or RiskManager rejects the trade.
    pub m15_atr_tp_multiplier: Option<f64>,
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
pub struct StochasticMomentumConfig {
    pub enabled: bool,
    pub weight: f64,
    pub overbought: f64,
    pub oversold: f64,
    pub atr_sl_multiplier: Option<f64>,
    pub atr_tp_multiplier: Option<f64>,
    pub trailing_stop_pips: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct M15MomentumBurstConfig {
    pub enabled: bool,
    pub weight: f64,
    pub rsi_min: f64,
    pub rsi_max: f64,
    pub atr_sl_multiplier: Option<f64>,
    pub atr_tp_multiplier: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct M15EmaMicrotrendConfig {
    pub enabled: bool,
    pub weight: f64,
    pub atr_sl_multiplier: Option<f64>,
    pub atr_tp_multiplier: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct M15BollingerReversionConfig {
    pub enabled: bool,
    pub weight: f64,
    pub percent_b_threshold: f64,
    pub rsi_threshold: f64,
    pub h1_rsi_confirm: f64,
    pub atr_sl_multiplier: Option<f64>,
    pub atr_tp_multiplier: Option<f64>,
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
        let path =
            std::env::var("CONFIG_PATH").unwrap_or_else(|_| "config/default.toml".to_string());
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
                weekend_epics: vec![],
                context_market_ids: vec![],
            },
            risk: RiskConfig {
                max_risk_per_trade: 1.0,
                max_daily_loss_pct: 3.0,
                max_weekly_drawdown_pct: 5.0,
                max_daily_trades: 20,
                max_open_positions: 9,
                max_positions_per_instrument: 3,
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
                trailing_stop_min_pips: 7.5,
                volatile_breakeven_trigger: 0.7,
                allowed_sessions: vec![
                    crate::engine::state::Session::Asia,
                    crate::engine::state::Session::London,
                    crate::engine::state::Session::UsOverlap,
                ],
                news_blackout_windows_utc: vec![(8, 30), (13, 30), (15, 0)],
                news_blackout_mins: 15,
                macro_events: vec![], // default.toml populates these via TOML; overridden on load
                allow_weekend_trading: false,
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
                stochastic_momentum: Some(StochasticMomentumConfig {
                    enabled: true,
                    weight: 1.0,
                    overbought: 70.0,
                    oversold: 30.0,
                    atr_sl_multiplier: None,
                    atr_tp_multiplier: None,
                    trailing_stop_pips: Some(15.0),
                }),
                m15_momentum_burst: None,
                m15_ema_microtrend: None,
                m15_bollinger_reversion: None,
                m15_min_consensus: 1,
                m15_min_avg_strength: 6.5,
                m15_position_size_multiplier: 0.5,
                m15_max_trades_per_h1: 2,
                m15_min_entry_spacing_secs: default_m15_min_entry_spacing_secs(),
                instrument_overrides: {
                    let mut m = HashMap::new();

                    // ── Gold: very restrictive ──────────────────────────────────────────
                    // Strong trends (ADX=62.9), extreme RSI (8.76), high ATR (~1%).
                    // Mean-reversion strategies fire on oversold but trend keeps going.
                    // Result: 1W/4L → need highest bar of all instruments.
                    m.insert(
                        "CS.D.CFIGOLD.CFI.IP".to_string(),
                        InstrumentStrategyOverride {
                            adx_range_filter: true,
                            adx_range_max: Some(25.0),
                            // Consensus: need 2/3 M15 strategies to agree (was 1/3)
                            min_consensus: Some(2),
                            min_avg_strength: Some(8.0),
                            // ADX trend-lock: when ADX > 45, block counter-DI signals
                            adx_trend_lock_enabled: true,
                            adx_trend_lock_threshold: Some(45.0),
                            // RSI extreme: when ADX > 40 AND RSI < 15, block mean-rev BUYs
                            rsi_extreme_oversold_floor: Some(15.0),
                            rsi_extreme_overbought_ceiling: Some(85.0),
                            rsi_extreme_block_adx_min: Some(40.0),
                            // Silence mean-reversion strategies when ADX > 45
                            mean_reversion_weight_in_strong_trend: Some(0.0),
                            mean_reversion_suppress_adx_min: Some(45.0),
                            // Max 2 Gold trades per day
                            max_daily_trades: Some(2),
                            // Block if ATR% > 1.8% (extreme volatility spike)
                            atr_pct_max_entry: Some(1.8),
                            ..Default::default()
                        },
                    );

                    // ── EUR/USD: tighter after Asia session loss (Mar 20) ────────────────
                    // Loss at 01:16 UTC (Asia session) — 1/3 consensus too low overnight.
                    // Fix: require 2/3 M15 consensus + mild trend-lock at ADX>35.
                    // Trading hours now 07:00-20:00 UTC (London/NY only) in default.toml.
                    m.insert(
                        "CS.D.EURUSD.CSD.IP".to_string(),
                        InstrumentStrategyOverride {
                            adx_range_filter: true,
                            adx_range_max: Some(25.0),
                            // Require 2/3 M15 strategies to agree (was 1/3)
                            min_consensus: Some(2),
                            min_avg_strength: Some(7.5),
                            // Light trend-lock at ADX > 35 (strong trend)
                            adx_trend_lock_enabled: true,
                            adx_trend_lock_threshold: Some(35.0),
                            // RSI extremes: block mean-reversion in strong trends
                            rsi_extreme_oversold_floor: Some(20.0),
                            rsi_extreme_overbought_ceiling: Some(80.0),
                            rsi_extreme_block_adx_min: Some(35.0),
                            // Max 3 trades per day (was 5)
                            max_daily_trades: Some(3),
                            ..Default::default()
                        },
                    );

                    // ── USD/JPY: moderate restrictions (mixed results) ─────────────────
                    m.insert(
                        "CS.D.USDJPY.CSD.IP".to_string(),
                        InstrumentStrategyOverride {
                            adx_range_filter: true,
                            adx_range_max: Some(25.0),
                            min_consensus: Some(2),
                            min_avg_strength: Some(8.0),
                            // Mild trend-lock at higher threshold than Gold
                            adx_trend_lock_enabled: true,
                            adx_trend_lock_threshold: Some(50.0),
                            rsi_extreme_oversold_floor: Some(20.0),
                            rsi_extreme_overbought_ceiling: Some(80.0),
                            rsi_extreme_block_adx_min: Some(45.0),
                            // Partial suppression (0.3×) not full silence
                            mean_reversion_weight_in_strong_trend: Some(0.3),
                            mean_reversion_suppress_adx_min: Some(50.0),
                            max_daily_trades: Some(3),
                            atr_pct_max_entry: Some(1.2),
                            ..Default::default()
                        },
                    );

                    m
                },
                consensus_matrix: HashMap::new(),
                weekend_overrides: HashMap::new(),
                h1_direction_gate_enabled: true,
                h1_alignment_bonus: 1.2,
                h1_macro_trend_gate_enabled: true,
                h1_macro_trend_lookback: 5,
                volatile_atr_sl_multiplier: 1.0,
                volatile_atr_tp_multiplier: 2.5,
                post_trade_cooldown_secs: 1800,
                require_h1_confirmation: true,
                regime_cooldown_days: Some(7),
                regime_cooldown_sl_multiplier: Some(1.25),
                regime_cooldown_tp_multiplier: Some(3.0),
                regime_cooldown_disable_be_snap: Some(true),
                ensemble_signal_floor: 5.0,
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
