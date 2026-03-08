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

use serde::Deserialize;
use chrono::Utc;
use tracing::{debug, info};

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
            RegimeKind::Ranging  => write!(f, "RANGING"),
            RegimeKind::Volatile => write!(f, "VOLATILE"),
        }
    }
}

/// Regime state for one instrument, as read from the JSON file.
#[derive(Debug, Clone)]
pub struct Regime {
    pub kind:       RegimeKind,
    pub confidence: f64,
    pub timestamp:  i64,
    pub instrument: String,
}

// ── JSON deserialization helpers ───────────────────────────────────────────────

#[derive(Deserialize)]
struct RawEntry {
    regime:     String,
    confidence: f64,
    instrument: String,
    timestamp:  i64,
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
const TRENDING_BOOST:   f64 = 1.5;   // trend family boosted
const TRENDING_MUTE:    f64 = 0.3;   // reversion family muted
const RANGING_BOOST:    f64 = 1.5;   // reversion family boosted
const RANGING_MUTE:     f64 = 0.3;   // trend family muted
const VOLATILE_MUTE:    f64 = 0.4;   // all strategies muted

const TREND_STRATEGIES:     &[&str] = &["MA_Crossover", "MACD_Momentum", "Multi_Timeframe"];
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
            epic, age_secs / 60, MAX_AGE_SECS / 60
        );
        return None;
    }

    let kind = match entry.regime.as_str() {
        "TRENDING" => RegimeKind::Trending,
        "RANGING"  => RegimeKind::Ranging,
        _          => RegimeKind::Volatile,   // includes "VOLATILE" and any unknown
    };

    Some(Regime {
        kind,
        confidence: entry.confidence,
        timestamp:  entry.timestamp,
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
/// | Regime   | MA_Crossover | MACD_Momentum | RSI_Reversal | Bollinger_Bands | Gold_Sentiment |
/// |----------|-------------|---------------|-------------|-----------------|----------------|
/// | TRENDING | ×1.5        | ×1.5          | ×0.3        | ×0.3            | ×1.0           |
/// | RANGING  | ×0.3        | ×0.3          | ×1.5        | ×1.5            | ×1.0           |
/// | VOLATILE | ×0.4        | ×0.4          | ×0.4        | ×0.4            | ×0.4           |
pub fn apply_regime_multipliers(signals: &mut [Signal], regime: &Regime) {
    let icon = match regime.kind {
        RegimeKind::Trending => "📈",
        RegimeKind::Ranging  => "↔️",
        RegimeKind::Volatile => "⚡",
    };
    info!(
        "Regime {} {} (conf={:.2}) — adjusting {} signal strengths",
        icon, regime.kind, regime.confidence, signals.len()
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

fn regime_multiplier(kind: &RegimeKind, strategy: &str) -> f64 {
    match kind {
        RegimeKind::Trending => {
            if TREND_STRATEGIES.contains(&strategy)     { TRENDING_BOOST }
            else if REVERSION_STRATEGIES.contains(&strategy) { TRENDING_MUTE }
            else { 1.0 }
        }
        RegimeKind::Ranging => {
            if REVERSION_STRATEGIES.contains(&strategy) { RANGING_BOOST }
            else if TREND_STRATEGIES.contains(&strategy)     { RANGING_MUTE }
            else { 1.0 }
        }
        RegimeKind::Volatile => {
            // Sentiment signals are still valid in volatility (often driven by them)
            if strategy == "Gold_Sentiment" { 1.0 }
            else { VOLATILE_MUTE }
        }
    }
}
