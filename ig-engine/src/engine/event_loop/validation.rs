use anyhow::Result;
use crate::engine::config::EngineConfig;

/// Validate configuration for obvious logical errors before engine start
pub fn validate_config(config: &EngineConfig) -> Result<()> {
    // MA Crossover: short < long period
    if let Some(ma) = &config.strategies.ma_crossover {
        if ma.enabled && ma.short_period >= ma.long_period {
            anyhow::bail!(
                "ma_crossover: short_period ({}) must be less than long_period ({})",
                ma.short_period, ma.long_period
            );
        }
    }

    // RSI: oversold < overbought
    if let Some(rsi) = &config.strategies.rsi_divergence {
        if rsi.enabled && rsi.oversold >= rsi.overbought {
            anyhow::bail!(
                "rsi_divergence: oversold ({}) must be less than overbought ({})",
                rsi.oversold, rsi.overbought
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
        config.strategies.ma_crossover.as_ref().map_or(false, |s| s.enabled),
        config.strategies.rsi_divergence.as_ref().map_or(false, |s| s.enabled),
        config.strategies.macd_momentum.as_ref().map_or(false, |s| s.enabled),
        config.strategies.bollinger_reversion.as_ref().map_or(false, |s| s.enabled),
    ].iter().filter(|&&e| e).count();

    if config.strategies.min_consensus > enabled_count {
        anyhow::bail!(
            "strategies.min_consensus ({}) cannot exceed the number of enabled strategies ({})",
            config.strategies.min_consensus, enabled_count
        );
    }
    if enabled_count == 0 {
        anyhow::bail!("At least one strategy must be enabled");
    }

    Ok(())
}
