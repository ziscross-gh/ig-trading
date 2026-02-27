pub mod position_sizer;

use chrono::{DateTime, Timelike, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, info, warn};

pub use position_sizer::calculate_position_size;

/// Instrument specification for position sizing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstrumentSpec {
    pub epic: String,
    pub min_deal_size: f64,
    pub max_deal_size: f64,
    pub pip_value: f64,
    pub pip_scale: f64,              // 0.0001 for normal, 0.01 for JPY
    pub contract_size: f64,
    pub margin_requirement_pct: f64,
    pub size_decimals: usize,        // 2 for standard indices/FX
    pub min_guaranteed_stop_pips: f64, // Used to guard against IG API rejections
}

impl InstrumentSpec {
    /// Get instrument spec from IG Markets epic code (hardcoded fallbacks)
    pub fn from_epic_fallback(epic: &str) -> Option<Self> {
        match epic {
            "CS.D.CFIGOLD.CFI.IP" | "CS.D.CFDGOLD.CMG.IP" | "CS.D.GOLDUSD.CSD.IP" | "CS.D.GOLDUSD.CFD" => Some(Self {
                epic: epic.to_string(),
                min_deal_size: 3.0,
                max_deal_size: 100.0,
                pip_value: 1.0,
                pip_scale: 1.0,
                contract_size: 1.0,
                margin_requirement_pct: 20.0,
                size_decimals: 2,
                min_guaranteed_stop_pips: 2.0,
            }),
            "CS.D.EURUSD.CSD.IP" | "CS.D.EURUSD.CFD" => Some(Self {
                epic: epic.to_string(),
                min_deal_size: 0.02,
                max_deal_size: 100.0,
                pip_value: 1.27,
                pip_scale: 0.0001,
                contract_size: 1.0,
                margin_requirement_pct: 5.0,
                size_decimals: 2,
                min_guaranteed_stop_pips: 5.0,
            }),
            "CS.D.GBPUSD.CSD.IP" | "CS.D.GBPUSD.CFD" => Some(Self {
                epic: epic.to_string(),
                min_deal_size: 0.01,
                max_deal_size: 100.0,
                pip_value: 1.27,
                pip_scale: 0.0001,
                contract_size: 1.0,
                margin_requirement_pct: 5.0,
                size_decimals: 2,
                min_guaranteed_stop_pips: 8.0,
            }),
            "CS.D.USDJPY.CSD.IP" | "CS.D.USDJPY.CFD" | "CS.D.EURJPY.CSD.IP" | "CS.D.EURJPY.CFD" => Some(Self {
                epic: epic.to_string(),
                min_deal_size: 0.02,
                max_deal_size: 100.0,
                pip_value: 0.81,
                pip_scale: 0.01,
                contract_size: 1.0,
                margin_requirement_pct: 5.0,
                size_decimals: 2,
                min_guaranteed_stop_pips: 7.0,
            }),
            "CS.D.AUDUSD.CSD.IP" | "CS.D.AUDUSD.CFD" => Some(Self {
                epic: epic.to_string(),
                min_deal_size: 0.02,
                max_deal_size: 100.0,
                pip_value: 1.27,
                pip_scale: 0.0001,
                contract_size: 1.0,
                margin_requirement_pct: 5.0,
                size_decimals: 2,
                min_guaranteed_stop_pips: 6.0,
            }),
            _ => None,
        }
    }
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum SizingMethod {
    #[default]
    Fixed,
    FixedFractional,
    HalfKelly,
    QuarterKelly,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitBreakerConfig {
    pub consecutive_losses_reduce: u32,
    pub consecutive_losses_pause: u32,
    pub pause_duration_mins: u64,
    pub daily_loss_warning_pct: f64,
}

/// Risk management configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct RiskConfig {
    pub max_risk_per_trade: f64,           // Maximum % of account to risk per trade
    pub max_daily_loss_pct: f64,           // Maximum daily loss % before stopping
    pub max_weekly_drawdown_pct: f64,      // Maximum weekly drawdown % before pausing
    pub max_daily_trades: u32,             // Maximum trades per day
    pub max_open_positions: usize,         // Maximum concurrent open positions
    pub max_correlated_positions: usize,   
    pub max_margin_usage_pct: f64,
    pub min_risk_reward: f64,        // Minimum risk:reward ratio
    pub sizing_method: SizingMethod,
    pub instrument_specs: HashMap<String, InstrumentSpec>,
    pub circuit_breaker: CircuitBreakerConfig,
    pub trading_hours_utc: Option<(u32, u32)>, // (start_hour, end_hour) in UTC, None = 24/7
    pub limited_risk_account: bool,
    pub min_guaranteed_stop_distance: Option<f64>,
    pub use_trailing_stop: bool,
    pub allowed_sessions: Vec<crate::engine::state::Session>,
}

impl Default for RiskConfig {
    fn default() -> Self {
        Self {
            max_risk_per_trade: 0.5,
            max_daily_loss_pct: 2.0,
            max_weekly_drawdown_pct: 5.0,
            max_daily_trades: 20,
            max_open_positions: 2,
            max_correlated_positions: 1,
            max_margin_usage_pct: 20.0,
            min_risk_reward: 2.0,
            sizing_method: SizingMethod::QuarterKelly,
            instrument_specs: HashMap::new(),
            circuit_breaker: CircuitBreakerConfig {
                consecutive_losses_reduce: 2,
                consecutive_losses_pause: 3,
                pause_duration_mins: 120,
                daily_loss_warning_pct: 60.0,
            },
            trading_hours_utc: Some((0, 16)), // 00:00-16:00 UTC (08:00-00:00 SGT)
            limited_risk_account: true,
            min_guaranteed_stop_distance: None,
            use_trailing_stop: false,
            allowed_sessions: vec![
                crate::engine::state::Session::Asia,
                crate::engine::state::Session::London,
                crate::engine::state::Session::UsOverlap,
            ],
        }
    }
}

/// Risk verdict for trade approval
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RiskVerdict {
    Approved(AdjustedTrade),
    Rejected(String),
}

/// Adjusted trade parameters after risk checks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdjustedTrade {
    pub epic: String,
    pub direction: String,
    pub size: f64,
    pub stop_loss: f64,
    pub take_profit: f64,
    pub trailing_stop_distance: Option<f64>,
    pub strategy: String,
    pub warning: Option<String>,
}

/// Open position information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenPosition {
    pub epic: String,
    pub direction: String,
    pub size: f64,
    pub entry_price: f64,
    pub stop_loss: f64,
    pub take_profit: f64,
}

/// Account information for risk checks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountInfo {
    pub balance: f64,
    pub equity: f64,
    pub available_margin: f64,
}

/// Risk statistics snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct RiskStats {
    pub daily_pnl: f64,
    pub daily_trades: u32,
    pub consecutive_losses: u32,
    pub consecutive_wins: u32,
    pub high_watermark: f64,
    pub is_paused: bool,
    pub paused_until: Option<DateTime<Utc>>,
    pub circuit_breaker_active: bool,
    pub position_size_multiplier: f64,
}

/// Main risk manager for the trading engine
pub struct RiskManager {
    pub config: RiskConfig,
    daily_pnl: f64,
    weekly_pnl: f64,
    week_start: DateTime<Utc>,
    daily_trades: u32,
    consecutive_losses: u32,
    consecutive_wins: u32,
    high_watermark: f64,
    is_paused: bool,
    paused_until: Option<DateTime<Utc>>,
    circuit_breaker_active: bool,
    position_size_multiplier: f64,
    last_reset: DateTime<Utc>,
}

impl RiskManager {
    /// Create a new RiskManager with configuration
    pub fn new(config: RiskConfig) -> Self {
        Self {
            config,
            daily_pnl: 0.0,
            weekly_pnl: 0.0,
            week_start: Utc::now(),
            daily_trades: 0,
            consecutive_losses: 0,
            consecutive_wins: 0,
            high_watermark: 0.0,
            is_paused: false,
            paused_until: None,
            circuit_breaker_active: false,
            position_size_multiplier: 1.0,
            last_reset: Utc::now(),
        }
    }

    /// Check if trade passes all risk checks and return verdict
    pub fn check_trade(
        &mut self,
        epic: &str,
        direction: &str,
        entry_price: f64,
        stop_loss: f64,
        take_profit: f64,
        account_info: &AccountInfo,
        open_positions: &[OpenPosition],
        strategy: &str,
    ) -> RiskVerdict {
        debug!("Starting risk check for {} {}", epic, direction);

        // Layer 1: Kill Switch - Check if trading is paused
        if self.is_paused {
            if let Some(paused_until) = self.paused_until {
                if Utc::now() < paused_until {
                    let reason = format!("Trading paused until {:?}", paused_until);
                    warn!("{}", reason);
                    return RiskVerdict::Rejected(reason);
                }
            } else {
                let reason = "Trading paused indefinitely".to_string();
                warn!("{}", reason);
                return RiskVerdict::Rejected(reason);
            }
        }

        // Layer 1b: Check circuit breaker
        if self.circuit_breaker_active {
            let reason = "Circuit breaker is active - position sizing reduced".to_string();
            debug!("{}", reason);
        }

        // Layer 1.5: Global Session Filter
        let current_hour = Utc::now().hour();
        let current_session = crate::engine::state::Session::from_utc_hour(current_hour);
        if !self.config.allowed_sessions.contains(&current_session) {
            let reason = format!("Trading not allowed in session: {:?}", current_session);
            warn!("{}", reason);
            return RiskVerdict::Rejected(reason);
        }

        // Layer 2: Trading Hours - Check if within configured trading hours
        if let Some((start_hour, end_hour)) = self.config.trading_hours_utc {
            let now = Utc::now();
            let current_hour = now.hour();
            if current_hour < start_hour || current_hour >= end_hour {
                let reason = format!(
                    "Outside trading hours. Current UTC hour: {}. Allowed: {} - {}",
                    current_hour, start_hour, end_hour
                );
                warn!("{}", reason);
                return RiskVerdict::Rejected(reason);
            }
        }

        // Layer 3: Daily Limits - Check daily trade count and loss limit
        if self.daily_trades >= self.config.max_daily_trades {
            let reason = format!(
                "Daily trade limit reached: {} / {}",
                self.daily_trades, self.config.max_daily_trades
            );
            warn!("{}", reason);
            return RiskVerdict::Rejected(reason);
        }

        let max_daily_loss = -(account_info.balance * self.config.max_daily_loss_pct / 100.0);
        if self.daily_pnl < max_daily_loss {
            let reason = format!(
                "Daily loss limit exceeded: {:.2} / {:.2}",
                self.daily_pnl, max_daily_loss
            );
            warn!("{}", reason);
            return RiskVerdict::Rejected(reason);
        }

        // Layer 3b: Weekly Drawdown Circuit Breaker
        self.check_weekly_reset();
        let max_weekly_loss = -(account_info.balance * self.config.max_weekly_drawdown_pct / 100.0);
        if self.weekly_pnl < max_weekly_loss {
            let reason = format!(
                "Weekly drawdown limit exceeded: {:.2} / {:.2}",
                self.weekly_pnl, max_weekly_loss
            );
            warn!("{}", reason);
            return RiskVerdict::Rejected(reason);
        }

        // Layer 4: Position Limits
        if open_positions.len() >= self.config.max_open_positions {
            let reason = format!(
                "Maximum open positions reached: {} / {}",
                open_positions.len(),
                self.config.max_open_positions
            );
            warn!("{}", reason);
            return RiskVerdict::Rejected(reason);
        }

        // Check if already in this market
        if open_positions.iter().any(|pos| pos.epic == epic) {
            let reason = format!("Already have an open position in {}", epic);
            warn!("{}", reason);
            return RiskVerdict::Rejected(reason);
        }

        // Check correlated pairs
        if let Err(e) = self.check_correlated_pairs(epic, open_positions) {
            return RiskVerdict::Rejected(e);
        }

        // Layer 5: Trade Validation
        // Stop loss must be set and valid
        if stop_loss <= 0.0 {
            let reason = "Stop loss must be set and greater than 0".to_string();
            warn!("{}", reason);
            return RiskVerdict::Rejected(reason);
        }

        // Validate stop loss is on correct side of entry price
        if direction.to_lowercase() == "buy" && stop_loss >= entry_price {
            let reason = format!(
                "Invalid stop loss for BUY: stop_loss ({}) must be < entry_price ({})",
                stop_loss, entry_price
            );
            warn!("{}", reason);
            return RiskVerdict::Rejected(reason);
        }

        if direction.to_lowercase() == "sell" && stop_loss <= entry_price {
            let reason = format!(
                "Invalid stop loss for SELL: stop_loss ({}) must be > entry_price ({})",
                stop_loss, entry_price
            );
            warn!("{}", reason);
            return RiskVerdict::Rejected(reason);
        }

        // Layer 5b: Guaranteed Stop Distance Check (Limited Risk Account)
    let mut adjusted_stop_loss = stop_loss;
    let mut warning_note: Option<String> = None;

    // Check IG minimum distances for guaranteed stops so we don't get rejected by API.
    // If we're operating a limited risk account, this check is mandatory.
    let instrument_spec = self.get_instrument_spec(epic);

    let req_min_stop_distance_pips = self.config.min_guaranteed_stop_distance.unwrap_or(instrument_spec.min_guaranteed_stop_pips);
    let req_min_stop_distance_points = req_min_stop_distance_pips * instrument_spec.pip_scale;
    let current_stop_distance = (entry_price - adjusted_stop_loss).abs();

    if self.config.limited_risk_account && current_stop_distance < req_min_stop_distance_points {
        let old_stop = adjusted_stop_loss;
        // Widen the stop to just slightly over the guaranteed limit
        let safe_distance = req_min_stop_distance_points * 1.05; // 5% buffer 
        
        if direction.to_lowercase() == "buy" {
            adjusted_stop_loss = entry_price - safe_distance;
        } else {
            adjusted_stop_loss = entry_price + safe_distance;
        }

        warning_note = Some(format!(
            "Guaranteed stop widened for {}: raw distance {:.5} < min allowed {:.5}. Shifted stop from {:.5} to {:.5}.",
            epic, current_stop_distance, req_min_stop_distance_points, old_stop, adjusted_stop_loss
        ));
        info!("{}", warning_note.as_ref().unwrap());
    }

    // Validate take profit
    if take_profit <= 0.0 {
        let reason = "Take profit must be set and greater than 0".to_string();
        warn!("{}", reason);
        return RiskVerdict::Rejected(reason);
    }

    // Check risk:reward ratio using adjusted stop
    let risk_distance = (entry_price - adjusted_stop_loss).abs();
    let reward_distance = (take_profit - entry_price).abs();

    if reward_distance == 0.0 {
        let reason = "Take profit must differ from entry price".to_string();
        warn!("{}", reason);
        return RiskVerdict::Rejected(reason);
    }
        let risk_reward_ratio = reward_distance / risk_distance;
        if risk_reward_ratio < self.config.min_risk_reward {
            let reason = format!(
                "Risk:reward ratio ({:.2}) below minimum ({:.2})",
                risk_reward_ratio, self.config.min_risk_reward
            );
            warn!("{}", reason);
            return RiskVerdict::Rejected(reason);
        }

        // Layer 6: Position Sizing
        let instrument_spec = self.get_instrument_spec(epic);

        let _risk_amount = account_info.balance * self.config.max_risk_per_trade / 100.0;
        let _stop_distance_in_pips = (entry_price - stop_loss).abs() / instrument_spec.pip_scale;

        let raw_size = calculate_position_size(
            account_info.balance,
            self.config.max_risk_per_trade,
            entry_price,
            stop_loss,
            epic,
            &self.config.instrument_specs,
        );

        // Apply circuit breaker position size reduction
        let adjusted_size = raw_size * self.position_size_multiplier;

        // Clamp to instrument limits
        let final_size = adjusted_size.max(instrument_spec.min_deal_size)
            .min(instrument_spec.max_deal_size);

        debug!(
            "Position sizing: raw={:.2}, multiplier={:.2}, final={:.2}",
            raw_size, self.position_size_multiplier, final_size
        );
        let adjusted_trade = AdjustedTrade {
            epic: epic.to_string(),
            direction: direction.to_string(),
            size: final_size,
            stop_loss: adjusted_stop_loss,
            take_profit,
            trailing_stop_distance: if self.config.use_trailing_stop {
                Some((entry_price - adjusted_stop_loss).abs())
            } else {
                None
            },
            strategy: strategy.to_string(),
            warning: warning_note,
        };

        info!(
            "Trade approved: {} {} @ {} (size={:.2})",
            epic, direction, entry_price, final_size
        );

        RiskVerdict::Approved(adjusted_trade)
    }

    /// Check for correlated pairs that shouldn't be traded simultaneously
    fn check_correlated_pairs(
        &self,
        epic: &str,
        open_positions: &[OpenPosition],
    ) -> Result<(), String> {
        let correlations = self.get_correlations();

        if let Some(correlated) = correlations.get(epic) {
            for pos in open_positions {
                if correlated.contains(&pos.epic.as_str()) {
                    return Err(format!(
                        "Correlated pair already open: {} correlates with {}",
                        epic, pos.epic
                    ));
                }
            }
        }

        Ok(())
    }

    /// Get map of epic to correlated pairs
    /// NOTE: These must match the actual epic codes in config/default.toml
    fn get_correlations(&self) -> HashMap<&'static str, Vec<&'static str>> {
        let mut correlations = HashMap::new();
        // Major USD pairs are correlated (EUR/USD ↔ GBP/USD move similarly)
        correlations.insert("CS.D.EURUSD.CSD.IP", vec!["CS.D.GBPUSD.CSD.IP"]);
        correlations.insert("CS.D.GBPUSD.CSD.IP", vec!["CS.D.EURUSD.CSD.IP"]);
        // JPY pairs
        correlations.insert("CS.D.USDJPY.CSD.IP", vec!["CS.D.EURJPY.CSD.IP"]);
        correlations.insert("CS.D.EURJPY.CSD.IP", vec!["CS.D.USDJPY.CSD.IP"]);
        // Metals (Gold ↔ Silver are heavily correlated)
        correlations.insert("CS.D.CFIGOLD.CFI.IP", vec!["CS.D.SILVUSD.CSD.IP"]);
        correlations.insert("CS.D.SILVUSD.CSD.IP", vec!["CS.D.CFIGOLD.CFI.IP"]);
        correlations
    }

    #[allow(dead_code)]
    pub fn record_trade_result(&mut self, pnl: f64) {
        self.daily_pnl += pnl;
        self.weekly_pnl += pnl;

        if pnl < 0.0 {
            self.consecutive_losses += 1;
            self.consecutive_wins = 0;
        } else if pnl > 0.0 {
            self.consecutive_wins += 1;
            self.consecutive_losses = 0;
        }

        // Update high watermark
        if self.daily_pnl > self.high_watermark {
            self.high_watermark = self.daily_pnl;
        }

        debug!(
            "Trade result recorded: PnL={:.2}, Daily PnL={:.2}, Consecutive losses={}",
            pnl, self.daily_pnl, self.consecutive_losses
        );

        // Check circuit breaker thresholds
        self.check_circuit_breaker();
    }

    #[allow(dead_code)]
    pub fn check_circuit_breaker(&mut self) {
        // If consecutive losses >= reduction threshold, halve position size
        if self.consecutive_losses >= self.config.circuit_breaker.consecutive_losses_reduce
            && !self.circuit_breaker_active
        {
            self.position_size_multiplier = 0.5;
            self.circuit_breaker_active = true;
            warn!(
                "Circuit breaker REDUCE activated. Position size multiplier set to 0.5"
            );
        }

        // If consecutive losses >= stop threshold, pause trading
        if self.consecutive_losses >= self.config.circuit_breaker.consecutive_losses_pause {
            self.is_paused = true;
            self.paused_until = Some(Utc::now() + chrono::Duration::minutes(self.config.circuit_breaker.pause_duration_mins as i64));
            warn!(
                "Circuit breaker STOP activated. Trading paused for {} minutes after {} consecutive losses",
                self.config.circuit_breaker.pause_duration_mins,
                self.consecutive_losses
            );
        }
    }

    #[allow(dead_code)]
    pub fn reset_daily(&mut self) {
        info!(
            "Resetting daily stats. Previous daily PnL: {:.2}, trades: {}",
            self.daily_pnl, self.daily_trades
        );

        self.daily_pnl = 0.0;
        self.daily_trades = 0;
        self.consecutive_losses = 0;
        self.consecutive_wins = 0;
        self.high_watermark = 0.0;
        self.circuit_breaker_active = false;
        self.position_size_multiplier = 1.0;
        self.is_paused = false;
        self.paused_until = None;
        self.last_reset = Utc::now();

        // Also check for weekly reset
        self.check_weekly_reset();

        info!("Daily stats reset");
    }

    /// Check if a new ISO week has started and reset weekly PnL
    fn check_weekly_reset(&mut self) {
        use chrono::Datelike;
        let now = Utc::now();
        if now.iso_week() != self.week_start.iso_week() || now.year() != self.week_start.year() {
            info!(
                "Weekly reset triggered. Previous weekly PnL: {:.2}",
                self.weekly_pnl
            );
            self.weekly_pnl = 0.0;
            self.week_start = now;
        }
    }

    #[allow(dead_code)]
    pub fn get_stats(&self) -> RiskStats {
        RiskStats {
            daily_pnl: self.daily_pnl,
            daily_trades: self.daily_trades,
            consecutive_losses: self.consecutive_losses,
            consecutive_wins: self.consecutive_wins,
            high_watermark: self.high_watermark,
            is_paused: self.is_paused,
            paused_until: self.paused_until,
            circuit_breaker_active: self.circuit_breaker_active,
            position_size_multiplier: self.position_size_multiplier,
        }
    }

    #[allow(dead_code)]
    pub fn pause_trading(&mut self, until: Option<DateTime<Utc>>) {
        self.is_paused = true;
        self.paused_until = until;
        info!("Trading paused until {:?}", until);
    }

    #[allow(dead_code)]
    pub fn resume_trading(&mut self) {
        self.is_paused = false;
        self.paused_until = None;
        info!("Trading resumed");
    }

    #[allow(dead_code)]
    pub fn config(&self) -> &RiskConfig {
        &self.config
    }

    pub fn get_instrument_spec(&self, epic: &str) -> InstrumentSpec {
        self.config.instrument_specs.get(epic).cloned()
            .or_else(|| InstrumentSpec::from_epic_fallback(epic))
            .unwrap_or_else(|| InstrumentSpec {
                epic: epic.to_string(),
                min_deal_size: 0.1,
                max_deal_size: 100.0,
                pip_value: 10.0,
                pip_scale: 0.0001,
                contract_size: 1.0,
                margin_requirement_pct: 2.0,
                size_decimals: 2,
                min_guaranteed_stop_pips: 10.0,
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_risk_manager_creation() {
        let config = RiskConfig::default();
        let rm = RiskManager::new(config);
        assert_eq!(rm.daily_pnl, 0.0);
        assert_eq!(rm.daily_trades, 0);
    }

    #[test]
    fn test_trade_with_invalid_stop_loss() {
        let config = RiskConfig::default();
        let mut rm = RiskManager::new(config);
        let account = AccountInfo {
            balance: 10000.0,
            equity: 10000.0,
            available_margin: 5000.0,
        };

        let verdict = rm.check_trade(
            "CS.D.EURUSD.CFD",
            "buy",
            1.1000,
            0.0, // Invalid stop loss
            1.1100,
            &account,
            &[],
            "test",
        );

        match verdict {
            RiskVerdict::Rejected(_) => assert!(true),
            _ => panic!("Should have rejected trade"),
        }
    }

    #[test]
    fn test_circuit_breaker_activation() {
        let mut config = RiskConfig::default();
        config.circuit_breaker.consecutive_losses_reduce = 3;
        let mut rm = RiskManager::new(config);

        // Simulate 3 consecutive losses
        for _ in 0..3 {
            rm.record_trade_result(-100.0);
        }

        assert!(rm.is_paused);
        assert!(rm.paused_until.is_some());
    }

    #[test]
    fn test_daily_reset() {
        let config = RiskConfig::default();
        let mut rm = RiskManager::new(config);

        rm.daily_pnl = 500.0;
        rm.daily_trades = 5;
        rm.reset_daily();

        assert_eq!(rm.daily_pnl, 0.0);
        assert_eq!(rm.daily_trades, 0);
    }
}
