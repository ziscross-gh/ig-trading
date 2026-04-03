//! Regime reader — consumes `data/regime_latest.json` written by
//! `scripts/run_regime_classifier.py` (runs hourly via cron).
//!
//! The Python classifier labels each instrument as one of three regimes:
//!   - **Trending**  — momentum strategies (MA Crossover, MACD) outperform
//!   - **Ranging**   — mean-reversion strategies (RSI, Bollinger) outperform
//!   - **Volatile**  — all strategies lose; mute signals to avoid whipsaws
//!
//! The engine calls [`apply_regime_multipliers`] **before** `ensemble.vote()`,
//! scaling individual signal strengths so the winning strategy family gets a
//! consensus boost while the losing family is suppressed.
//!
//! JSON schema (one key per IG epic):
//! ```json
//! {
//!   "CS.D.CFIGOLD.CFI.IP": {
//!     "regime":     "TRENDING",
//!     "confidence": 0.87,
//!     "instrument": "GOLD",
//!     "timestamp":  1741219200,
//!     "features":   { "adx_14": 32.1, "hurst": 0.61, ... }
//!   }
//! }
//! ```

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, info, warn};

use crate::engine::state::Signal;

// ── Types ──────────────────────────────────────────────────────────────────────

/// Market regime as classified by the ML model.
#[derive(Debug, Clone, PartialEq)]
pub enum RegimeKind {
    /// Momentum strategies outperform — boost MA Crossover + MACD.
    Trending,
    /// Mean-reversion strategies outperform — boost RSI Reversal + Bollinger.
    Ranging,
    /// All strategies struggle — mute everything to reduce position risk.
    Volatile,
}

impl std::fmt::Display for RegimeKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RegimeKind::Trending => write!(f, "TRENDING"),
            RegimeKind::Ranging => write!(f, "RANGING"),
            RegimeKind::Volatile => write!(f, "VOLATILE"),
        }
    }
}

/// Regime state for one instrument, as read from the JSON file.
#[derive(Debug, Clone)]
pub struct Regime {
    pub kind: RegimeKind,
    pub confidence: f64,
    pub timestamp: i64,
    pub instrument: String,
}

// ── JSON deserialization helpers ───────────────────────────────────────────────

#[derive(Deserialize)]
struct RawEntry {
    regime: String,
    confidence: f64,
    instrument: String,
    timestamp: i64,
}

// ── Constants ──────────────────────────────────────────────────────────────────

/// Regime file is considered stale after 90 minutes (classifier runs hourly).
const MAX_AGE_SECS: i64 = 90 * 60;

/// Path relative to the working directory when the Rust engine runs.
const REGIME_FILE: &str = "data/regime_latest.json";

// ── Signal multipliers ─────────────────────────────────────────────────────────

/// Strategy weight multipliers for each regime.
/// These adjust signal *strengths* before the ensemble vote, effectively
/// re-weighting the influence of each strategy family without touching
/// the underlying EnsembleVoter weights (which the adaptive system owns).
const TRENDING_BOOST: f64 = 1.5; // trend family boosted
const TRENDING_MUTE: f64 = 0.3; // reversion family muted
const RANGING_BOOST: f64 = 1.5; // reversion family boosted
const RANGING_MUTE: f64 = 0.3; // trend family muted
const VOLATILE_MUTE: f64 = 0.5; // all strategies muted (0.5 lets scalp tier reach avg ≥ 6.0)

const TREND_STRATEGIES: &[&str] = &["MA_Crossover", "MACD_Momentum", "Multi_Timeframe"];
const REVERSION_STRATEGIES: &[&str] = &["RSI_Reversal", "Bollinger_Bands"];

// ── Public API ─────────────────────────────────────────────────────────────────

/// Read the latest regime for a given IG epic from `data/regime_latest.json`.
///
/// Returns `None` if:
/// - The file does not exist (classifier not yet run / Python not set up)
/// - The epic is not in the JSON (unexpected instrument)
/// - The data is stale (older than 90 minutes)
pub fn read_regime(epic: &str) -> Option<Regime> {
    let raw = std::fs::read_to_string(REGIME_FILE).ok()?;
    let map: serde_json::Map<String, serde_json::Value> = serde_json::from_str(&raw).ok()?;
    let entry_val = map.get(epic)?;
    let entry: RawEntry = serde_json::from_value(entry_val.clone()).ok()?;

    let age_secs = Utc::now().timestamp() - entry.timestamp;
    if age_secs > MAX_AGE_SECS {
        debug!(
            "Regime data for {} is stale ({} min old, max {}min) — ignoring",
            epic,
            age_secs / 60,
            MAX_AGE_SECS / 60
        );
        return None;
    }

    let kind = match entry.regime.as_str() {
        "TRENDING" => RegimeKind::Trending,
        "RANGING" => RegimeKind::Ranging,
        _ => RegimeKind::Volatile, // includes "VOLATILE" and any unknown
    };

    Some(Regime {
        kind,
        confidence: entry.confidence,
        timestamp: entry.timestamp,
        instrument: entry.instrument,
    })
}

/// Apply regime-based signal strength multipliers **in-place** before the
/// ensemble vote.
///
/// Scales each signal's `strength` field according to how well its strategy
/// family is expected to perform in the current regime.  The ensemble voter
/// then operates on these scaled strengths, naturally favouring the dominant
/// family without requiring any changes to consensus logic.
///
/// # Example multiplier table
///
/// | Regime   | MA_Crossover | MACD_Momentum | RSI_Reversal | Bollinger_Bands | Gold_Sentiment | Stochastic_Momentum |
/// |----------|-------------|---------------|-------------|-----------------|----------------|---------------------|
/// | TRENDING | ×1.5        | ×1.5          | ×0.3        | ×0.3            | ×1.0           | ×0.5                |
/// | RANGING  | ×0.3        | ×0.3          | ×1.5        | ×1.5            | ×1.0           | ×1.2                |
/// | VOLATILE | ×0.5        | ×0.5          | ×0.5        | ×0.5            | ×1.0           | ×0.8                |
pub fn apply_regime_multipliers(signals: &mut [Signal], regime: &Regime) {
    let icon = match regime.kind {
        RegimeKind::Trending => "📈",
        RegimeKind::Ranging => "↔️",
        RegimeKind::Volatile => "⚡",
    };
    info!(
        "Regime {} {} (conf={:.2}) — adjusting {} signal strengths",
        icon,
        regime.kind,
        regime.confidence,
        signals.len()
    );

    for sig in signals.iter_mut() {
        let multiplier = regime_multiplier(&regime.kind, &sig.strategy);
        let orig = sig.strength;
        sig.strength = (sig.strength * multiplier).clamp(0.0, 10.0);
        if (multiplier - 1.0).abs() > 0.01 {
            debug!(
                "  {} strength {:.1} → {:.1} (×{:.1})",
                sig.strategy, orig, sig.strength, multiplier
            );
        }
    }
}

// ── Regime Persistence Tracking ───────────────────────────────────────────────

/// Path for the regime persistence file (tracks how long each instrument
/// has remained in the same regime).
const PERSISTENCE_FILE: &str = "data/regime_persistence.json";

/// Per-instrument persistence record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistenceEntry {
    pub regime: String,
    pub since: DateTime<Utc>,
}

/// Full persistence state: epic -> PersistenceEntry.
pub type RegimePersistence = HashMap<String, PersistenceEntry>;

/// Read the persistence file from disk. Returns an empty map if the file
/// is missing, unreadable, or contains invalid JSON.
fn load_persistence() -> RegimePersistence {
    match std::fs::read_to_string(PERSISTENCE_FILE) {
        Ok(raw) => serde_json::from_str(&raw).unwrap_or_else(|e| {
            warn!(
                "regime_persistence.json parse error (starting fresh): {}",
                e
            );
            HashMap::new()
        }),
        Err(_) => HashMap::new(), // file not yet created — first run
    }
}

/// Write persistence state back to disk. Logs a warning on failure but
/// does not propagate the error (non-critical bookkeeping).
fn save_persistence(state: &RegimePersistence) {
    match serde_json::to_string_pretty(state) {
        Ok(json) => {
            if let Err(e) = std::fs::write(PERSISTENCE_FILE, json) {
                warn!("Failed to write regime_persistence.json: {}", e);
            }
        }
        Err(e) => warn!("Failed to serialize regime persistence: {}", e),
    }
}

/// Update persistence for `epic` with the given `current_regime` string
/// (e.g. "VOLATILE", "TRENDING", "RANGING") and return how many days
/// the regime has been unchanged.
///
/// - If the regime changed → reset the timestamp, return 0.
/// - If the regime is the same → keep the timestamp, return elapsed days.
/// - If there is no prior record → create one, return 0.
pub fn update_persistence_and_get_days(epic: &str, current_regime: &str) -> u64 {
    let mut state = load_persistence();
    let now = Utc::now();

    let days = match state.get(epic) {
        Some(entry) if entry.regime == current_regime => {
            let elapsed = now.signed_duration_since(entry.since);
            elapsed.num_days().max(0) as u64
        }
        _ => {
            // Regime changed or first time — reset
            state.insert(
                epic.to_string(),
                PersistenceEntry {
                    regime: current_regime.to_string(),
                    since: now,
                },
            );
            0
        }
    };

    save_persistence(&state);
    days
}

// ── Internal helpers ─────────────────────────────────────────────────────────

fn regime_multiplier(kind: &RegimeKind, strategy: &str) -> f64 {
    match kind {
        RegimeKind::Trending => {
            if TREND_STRATEGIES.contains(&strategy) {
                TRENDING_BOOST
            } else if REVERSION_STRATEGIES.contains(&strategy) {
                TRENDING_MUTE
            } else if strategy == "Stochastic_Momentum" {
                VOLATILE_MUTE
            }
            // oscillator less useful in trend
            else {
                1.0
            }
        }
        RegimeKind::Ranging => {
            if REVERSION_STRATEGIES.contains(&strategy) {
                RANGING_BOOST
            } else if TREND_STRATEGIES.contains(&strategy) {
                RANGING_MUTE
            } else if strategy == "Stochastic_Momentum" {
                1.2
            }
            // oscillators excel in ranging
            else {
                1.0
            }
        }
        RegimeKind::Volatile => {
            // Differentiated VOLATILE multipliers (Phase 15.D):
            // In volatile/choppy markets, oscillator and reversion strategies outperform
            // trend-following strategies. Trend strategies (MA, MTF) are actually harmful
            // (they chase false breakouts), so mute them harder.
            //
            //  Stochastic  → 1.2× best for catching overbought/oversold extremes in chop
            //  RSI_Reversal→ 1.0× mean reversion valid in volatile waves
            //  Bollinger   → 1.0× BB squeeze/expansion signals valid in volatile
            //  MACD        → 0.8× can catch initial volatile move direction
            //  MA_Crossover→ 0.3× trend-following = bad in chop
            //  Multi_TF    → 0.3× needs aligned trends = harmful in chop
            //  Sentiment   → 1.0× often the cause of volatility
            match strategy {
                "Stochastic_Momentum" => 1.2,
                "RSI_Reversal" => 1.0,
                "Bollinger_Bands" => 1.0,
                "MACD_Momentum" => 0.8,
                "MA_Crossover" => 0.3,
                "Multi_Timeframe" => 0.3,
                "Gold_Sentiment" => 1.0,
                _ => VOLATILE_MUTE,
            }
        }
    }
}
