use crate::engine::config::{EngineConfig, EngineMode};
use anyhow::Result;
use tracing::{info, warn};

/// Validate configuration for obvious logical errors before engine start
pub fn validate_config(config: &EngineConfig) -> Result<()> {
    // MA Crossover: short < long period
    if let Some(ma) = &config.strategies.ma_crossover {
        if ma.enabled && ma.short_period >= ma.long_period {
            anyhow::bail!(
                "ma_crossover: short_period ({}) must be less than long_period ({})",
                ma.short_period,
                ma.long_period
            );
        }
    }

    // RSI: oversold < overbought
    if let Some(rsi) = &config.strategies.rsi_divergence {
        if rsi.enabled && rsi.oversold >= rsi.overbought {
            anyhow::bail!(
                "rsi_divergence: oversold ({}) must be less than overbought ({})",
                rsi.oversold,
                rsi.overbought
            );
        }
    }

    // Risk: sensible ranges
    if config.risk.max_risk_per_trade <= 0.0 || config.risk.max_risk_per_trade > 10.0 {
        anyhow::bail!(
            "risk.max_risk_per_trade must be between 0 and 10% (got {})",
            config.risk.max_risk_per_trade
        );
    }
    if config.risk.max_open_positions == 0 {
        anyhow::bail!("risk.max_open_positions must be > 0");
    }

    // Consensus: can't require more strategies than are enabled
    let enabled_count = [
        config
            .strategies
            .ma_crossover
            .as_ref()
            .is_some_and(|s| s.enabled),
        config
            .strategies
            .rsi_divergence
            .as_ref()
            .is_some_and(|s| s.enabled),
        config
            .strategies
            .macd_momentum
            .as_ref()
            .is_some_and(|s| s.enabled),
        config
            .strategies
            .bollinger_reversion
            .as_ref()
            .is_some_and(|s| s.enabled),
    ]
    .iter()
    .filter(|&&e| e)
    .count();

    if config.strategies.min_consensus > enabled_count {
        anyhow::bail!(
            "strategies.min_consensus ({}) cannot exceed the number of enabled strategies ({})",
            config.strategies.min_consensus,
            enabled_count
        );
    }
    if enabled_count == 0 {
        anyhow::bail!("At least one strategy must be enabled");
    }

    Ok(())
}

/// Live-specific readiness checks run AFTER account balance is known (post get_accounts()).
/// Only runs when mode = Live. Logs warnings for dangerous-but-non-fatal conditions.
/// Returns Err only for hard blockers that would make live trading fundamentally unsafe.
pub fn validate_live_readiness(config: &EngineConfig, balance: f64) -> Result<()> {
    if !matches!(config.general.mode, EngineMode::Live) {
        return Ok(());
    }

    info!(
        "🔍 Running live readiness checks (balance={:.2})...",
        balance
    );

    // 1. Macro events — warn if empty (no NFP/FOMC blackouts)
    if config.risk.macro_events.is_empty() {
        warn!(
            "⚠️  LIVE: risk.macro_events is empty — \
             engine will trade through NFP, FOMC, CPI and other high-impact events! \
             Add macro_events to live.toml."
        );
    } else {
        info!(
            "✅ Macro events: {} blackout windows configured",
            config.risk.macro_events.len()
        );
    }

    // 2. Instrument spec completeness — warn for any epic missing a spec
    for epic in &config.markets.epics {
        if !config.risk.instrument_specs.contains_key(epic.as_str()) {
            warn!(
                "⚠️  LIVE: No instrument_spec found for epic '{}' — \
                 position sizer will use hardcoded fallback (min_deal_size=0.5). \
                 Add [risk.instrument_specs.\"{}\"] to live.toml.",
                epic, epic
            );
        }
    }

    // 3. Margin feasibility per epic — warn when minimum position exceeds margin cap
    if balance > 0.0 {
        for epic in &config.markets.epics {
            if let Some(spec) = config.risk.instrument_specs.get(epic.as_str()) {
                // Estimate current price from pip_scale (order of magnitude only)
                let assumed_price = if spec.pip_scale >= 1.0 {
                    2700.0_f64 // Gold ~$2700
                } else if spec.pip_scale >= 0.01 {
                    150.0_f64 // JPY pairs ~150
                } else {
                    1.10_f64 // EUR/USD, GBP/USD ~1.10
                };
                let min_margin =
                    spec.min_deal_size * assumed_price * spec.margin_requirement_pct / 100.0;
                let margin_pct_of_balance = min_margin / balance * 100.0;

                if margin_pct_of_balance > config.risk.max_margin_usage_pct {
                    warn!(
                        "⚠️  LIVE: {} minimum position (~{:.0} lots) requires {:.1}% of balance as margin \
                         (configured cap: {:.1}%). Engine will reject all {} trades. \
                         Either increase account balance or remove this epic from live.toml.",
                        epic, spec.min_deal_size, margin_pct_of_balance,
                        config.risk.max_margin_usage_pct, epic
                    );
                } else {
                    info!(
                        "✅ {}: min position uses ~{:.1}% margin of balance (cap={:.1}%)",
                        epic, margin_pct_of_balance, config.risk.max_margin_usage_pct
                    );
                }
            }
        }
    }

    info!("✅ Live readiness checks complete");
    Ok(())
}
