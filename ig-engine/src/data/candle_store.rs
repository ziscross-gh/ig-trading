#![allow(dead_code)]
//! CandleStore for caching historical price data.

use std::collections::HashMap;
use crate::indicators::Candle;

/// Maximum number of candles to store per epic-resolution series.
const MAX_CANDLES_PER_SERIES: usize = 1000;

/// A thread-safe cache for storing historical candlestick data.
///
/// Organizes candles by epic (instrument) and resolution (timeframe).
/// Each series is limited to a maximum of 1000 candles, with the oldest
/// data being automatically discarded when the limit is reached.
#[derive(Debug)]
pub struct CandleStore {
    /// Storage structure: epic → resolution → candles
    storage: HashMap<String, HashMap<String, Vec<Candle>>>,
}

impl CandleStore {
    /// Creates a new empty CandleStore.
    pub fn new() -> Self {
        CandleStore {
            storage: HashMap::new(),
        }
    }

    /// Appends a candle to the specified epic-resolution series.
    ///
    /// If the series reaches the maximum of 1000 candles, the oldest
    /// candle is removed before inserting the new one.
    ///
    /// # Arguments
    ///
    /// * `epic` - The instrument identifier (e.g., "EUR/USD")
    /// * `resolution` - The timeframe resolution (e.g., "1H", "5M", "D")
    /// * `candle` - The candle data to append
    pub fn push(&mut self, epic: &str, resolution: &str, candle: Candle) {
        let resolution_data = self
            .storage
            .entry(epic.to_string())
            .or_insert_with(HashMap::new)
            .entry(resolution.to_string())
            .or_insert_with(Vec::new);

        // Enforce maximum candles per series
        if resolution_data.len() >= MAX_CANDLES_PER_SERIES {
            resolution_data.remove(0);
        }

        resolution_data.push(candle);
    }

    /// Retrieves the candle series for the specified epic-resolution pair.
    ///
    /// # Arguments
    ///
    /// * `epic` - The instrument identifier
    /// * `resolution` - The timeframe resolution
    ///
    /// # Returns
    ///
    /// `Some(&Vec<Candle>)` if the series exists, `None` otherwise.
    pub fn get_candles(&self, epic: &str, resolution: &str) -> Option<&Vec<Candle>> {
        self.storage
            .get(epic)
            .and_then(|resolutions| resolutions.get(resolution))
    }

    /// Retrieves the most recent candle for the specified epic-resolution pair.
    ///
    /// # Arguments
    ///
    /// * `epic` - The instrument identifier
    /// * `resolution` - The timeframe resolution
    ///
    /// # Returns
    ///
    /// `Some(&Candle)` if a candle exists, `None` otherwise.
    pub fn get_latest(&self, epic: &str, resolution: &str) -> Option<&Candle> {
        self.get_candles(epic, resolution)
            .and_then(|candles| candles.last())
    }

    /// Bulk loads a series of candles for a given epic-resolution pair.
    ///
    /// Replaces any existing data for this epic-resolution combination.
    /// If the provided candles exceed the maximum of 1000, only the last
    /// 1000 candles are retained.
    ///
    /// # Arguments
    ///
    /// * `epic` - The instrument identifier
    /// * `resolution` - The timeframe resolution
    /// * `candles` - The vector of candles to load
    pub fn warm_up(&mut self, epic: &str, resolution: &str, mut candles: Vec<Candle>) {
        // Retain only the last MAX_CANDLES_PER_SERIES if needed
        if candles.len() > MAX_CANDLES_PER_SERIES {
            let start_idx = candles.len() - MAX_CANDLES_PER_SERIES;
            candles = candles[start_idx..].to_vec();
        }

        self.storage
            .entry(epic.to_string())
            .or_insert_with(HashMap::new)
            .insert(resolution.to_string(), candles);
    }

    /// Returns the number of candles in the specified epic-resolution series.
    ///
    /// # Arguments
    ///
    /// * `epic` - The instrument identifier
    /// * `resolution` - The timeframe resolution
    ///
    /// # Returns
    ///
    /// The count of candles, or 0 if the series does not exist.
    pub fn len(&self, epic: &str, resolution: &str) -> usize {
        self.get_candles(epic, resolution)
            .map(|candles| candles.len())
            .unwrap_or(0)
    }
}

impl Default for CandleStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let store = CandleStore::new();
        assert_eq!(store.storage.len(), 0);
    }
}
