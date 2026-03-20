use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SentimentData {
    pub market_id: String,
    pub long_pct: f64,
    pub short_pct: f64,
    pub score: f64,      // -1.0 to +1.0
    pub prev_score: f64, // Score from 15 mins ago
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GlobalSentimentRegistry {
    /// market_id -> SentimentData
    pub data: HashMap<String, SentimentData>,
}

impl GlobalSentimentRegistry {
    pub fn new() -> Self {
        Self {
            data: HashMap::new(),
        }
    }

    pub fn update(&mut self, market_id: String, long_pct: f64, short_pct: f64) {
        // Calculate score: (Long - Short) / 100
        let new_score = (long_pct - short_pct) / 100.0;

        let prev_score = self
            .data
            .get(&market_id)
            .map(|d| d.score)
            .unwrap_or(new_score);

        self.data.insert(
            market_id.clone(),
            SentimentData {
                market_id,
                long_pct,
                short_pct,
                score: new_score,
                prev_score,
                updated_at: Utc::now(),
            },
        );
    }

    /// Returns the absolute change in sentiment score since the last update
    pub fn get_velocity(&self, market_id: &str) -> f64 {
        self.data
            .get(market_id)
            .map(|d| (d.score - d.prev_score).abs())
            .unwrap_or(0.0)
    }

    pub fn get(&self, market_id: &str) -> Option<&SentimentData> {
        self.data.get(market_id)
    }
}
