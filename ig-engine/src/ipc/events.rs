use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

use crate::engine::state::MarketState;

/// EngineEvent is serialized flat for WebSocket consumers.
/// The dashboard expects `{ "type": "StatusChange", "data": {...}, "timestamp": "..." }`
/// We achieve this via #[serde(flatten)] on the variant enum.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineEvent {
    pub timestamp: DateTime<Utc>,
    #[serde(flatten)]
    pub event: EventVariant,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum EventVariant {
    StatusChange {
        old: String,
        new: String,
    },
    IndicatorUpdate {
        epic: String,
        indicators: crate::indicators::IndicatorSnapshot,
    },
    MarketUpdate {
        state: MarketState,
    },
    Signal {
        epic: String,
        direction: String,
        strategy: String,
        strength: f64,
        was_executed: bool,
    },
    TradeExecuted {
        deal_id: String,
        epic: String,
        direction: String,
        size: f64,
        price: f64,
    },
    PositionClosed {
        deal_id: String,
        pnl: f64,
    },
    RiskAlert {
        message: String,
        severity: String,
    },
    Heartbeat {
        uptime_secs: u64,
        open_positions: usize,
    },
    ConfigChanged {
        field: String,
    },
    Shutdown {
        reason: String,
    },
}

impl EngineEvent {
    pub fn shutdown(reason: String) -> Self {
        Self {
            timestamp: Utc::now(),
            event: EventVariant::Shutdown { reason },
        }
    }
    pub fn indicator_update(epic: String, indicators: crate::indicators::IndicatorSnapshot) -> Self {
        Self {
            timestamp: Utc::now(),
            event: EventVariant::IndicatorUpdate { epic, indicators },
        }
    }

    pub fn market_update(state: MarketState) -> Self {
        Self {
            timestamp: Utc::now(),
            event: EventVariant::MarketUpdate { state },
        }
    }

    pub fn status_change(old: String, new: String) -> Self {
        Self {
            timestamp: Utc::now(),
            event: EventVariant::StatusChange { old, new },
        }
    }

    pub fn signal(
        epic: String,
        direction: String,
        strategy: String,
        strength: f64,
        was_executed: bool,
    ) -> Self {
        Self {
            timestamp: Utc::now(),
            event: EventVariant::Signal {
                epic,
                direction,
                strategy,
                strength,
                was_executed,
            },
        }
    }

    pub fn trade_executed(
        deal_id: String,
        epic: String,
        direction: String,
        size: f64,
        price: f64,
    ) -> Self {
        Self {
            timestamp: Utc::now(),
            event: EventVariant::TradeExecuted {
                deal_id,
                epic,
                direction,
                size,
                price,
            },
        }
    }

    pub fn position_closed(deal_id: String, pnl: f64) -> Self {
        Self {
            timestamp: Utc::now(),
            event: EventVariant::PositionClosed { deal_id, pnl },
        }
    }

    #[allow(dead_code)]
    pub fn risk_alert(message: String, severity: String) -> Self {
        Self {
            timestamp: Utc::now(),
            event: EventVariant::RiskAlert { message, severity },
        }
    }

    pub fn heartbeat(uptime_secs: u64, open_positions: usize) -> Self {
        Self {
            timestamp: Utc::now(),
            event: EventVariant::Heartbeat {
                uptime_secs,
                open_positions,
            },
        }
    }

    pub fn config_changed(field: String) -> Self {
        Self {
            timestamp: Utc::now(),
            event: EventVariant::ConfigChanged { field },
        }
    }
}
