use std::fs::{OpenOptions, create_dir_all};
use std::io::Write;
use std::path::Path;
use anyhow::Result;
use tracing::{error, info};

use crate::engine::state::ClosedTrade;

/// Logger for recording trade outcomes to a structured JSONL file.
/// Used for performance analysis and reinforcement learning training data.
pub struct TradeLogger {
    log_path: String,
}

impl TradeLogger {
    /// Create a new TradeLogger. 
    /// Ensures the log directory exists.
    pub fn new(path: &str) -> Self {
        if let Some(parent) = Path::new(path).parent() {
            if let Err(e) = create_dir_all(parent) {
                error!("Failed to create log directory {}: {}", parent.display(), e);
            }
        }
        
        Self {
            log_path: path.to_string(),
        }
    }

    /// Appends a closed trade record to the log file in JSONL format.
    pub fn log_trade(&self, trade: &ClosedTrade) -> Result<()> {
        let json = serde_json::to_string(trade)?;
        
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_path)?;
            
        writeln!(file, "{}", json)?;
        
        info!("Trade logged to {}: deal_id={}", self.log_path, trade.deal_id);
        Ok(())
    }
}

impl Default for TradeLogger {
    fn default() -> Self {
        Self::new("logs/trades.jsonl")
    }
}
