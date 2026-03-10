#![allow(dead_code)]
//! CandleStore for caching historical price data.
//!
//! Supports optional disk persistence via JSONL files so that candle history
//! survives engine restarts. On startup, call [`load_from_disk`] to restore
//! previously saved bars. On each bar close (and on shutdown) call
//! [`CandleStore::persist_series`] or [`CandleStore::persist_all`] to flush
//! the current in-memory state.

use std::collections::HashMap;
use std::io::{BufRead, Write};
use std::path::PathBuf;
use tracing::{info, warn, debug};
use crate::indicators::Candle;

/// Maximum number of candles to store per epic-resolution series.
const MAX_CANDLES_PER_SERIES: usize = 1000;

/// Directory under the engine working directory where candle JSONL files live.
const CANDLE_DATA_DIR: &str = "data/candles";

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
            .or_default()
            .entry(resolution.to_string())
            .or_default();

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
            .or_default()
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

    /// Persist a single epic-resolution series to disk as JSONL.
    ///
    /// Writes to a `.tmp` file and atomically renames to avoid corruption.
    /// Failures are logged as warnings — never panics.
    pub fn persist_series(&self, epic: &str, resolution: &str) {
        if let Some(candles) = self.get_candles(epic, resolution) {
            save_to_disk(epic, resolution, candles);
        }
    }

    /// Persist all in-memory series to disk (called on graceful shutdown).
    pub fn persist_all(&self) {
        let mut count = 0usize;
        for (epic, res_map) in &self.storage {
            for (resolution, candles) in res_map {
                save_to_disk(epic, resolution, candles);
                count += 1;
            }
        }
        info!("Persisted {} candle series to disk", count);
    }
}

impl Default for CandleStore {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Free functions for disk I/O
// ---------------------------------------------------------------------------

/// Build the file path for a given epic + resolution.
///
/// Dots in the epic are replaced with underscores to keep file names safe.
/// Example: `CS.D.EURUSD.CSD.IP`, `HOUR` → `data/candles/CS_D_EURUSD_CSD_IP_HOUR.jsonl`
fn candle_file_path(epic: &str, resolution: &str) -> PathBuf {
    let safe_name = format!("{}_{}", epic.replace('.', "_"), resolution);
    PathBuf::from(CANDLE_DATA_DIR).join(format!("{safe_name}.jsonl"))
}

/// Load candles from a JSONL file on disk.
///
/// Returns an empty `Vec` if the file does not exist.
/// Corrupt lines are skipped with a warning — the engine never panics on I/O errors.
pub fn load_from_disk(epic: &str, resolution: &str) -> Vec<Candle> {
    let path = candle_file_path(epic, resolution);
    let file = match std::fs::File::open(&path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            debug!("No candle file on disk for {} [{}]", epic, resolution);
            return Vec::new();
        }
        Err(e) => {
            warn!("Failed to open candle file {}: {}", path.display(), e);
            return Vec::new();
        }
    };

    let reader = std::io::BufReader::new(file);
    let mut candles = Vec::new();
    let mut bad_lines = 0usize;

    for (line_no, line) in reader.lines().enumerate() {
        match line {
            Ok(text) => {
                let text = text.trim().to_string();
                if text.is_empty() {
                    continue;
                }
                match serde_json::from_str::<Candle>(&text) {
                    Ok(c) => candles.push(c),
                    Err(e) => {
                        bad_lines += 1;
                        warn!(
                            "Skipping corrupt line {} in {}: {}",
                            line_no + 1,
                            path.display(),
                            e
                        );
                    }
                }
            }
            Err(e) => {
                bad_lines += 1;
                warn!("IO error reading line {} of {}: {}", line_no + 1, path.display(), e);
            }
        }
    }

    if bad_lines > 0 {
        warn!(
            "Loaded {} candles from disk for {} [{}] ({} corrupt lines skipped)",
            candles.len(),
            epic,
            resolution,
            bad_lines
        );
    } else {
        debug!(
            "Loaded {} candles from disk for {} [{}]",
            candles.len(),
            epic,
            resolution
        );
    }
    candles
}

/// Write candles to a JSONL file atomically (temp + rename).
///
/// Creates the directory tree if it does not exist.
/// All errors are logged as warnings — never panics.
fn save_to_disk(epic: &str, resolution: &str, candles: &[Candle]) {
    let path = candle_file_path(epic, resolution);
    if let Some(parent) = path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            warn!("Failed to create candle data directory {}: {}", parent.display(), e);
            return;
        }
    }

    let tmp_path = path.with_extension("jsonl.tmp");

    let result = (|| -> std::io::Result<()> {
        let mut file = std::fs::File::create(&tmp_path)?;
        for candle in candles {
            let json = serde_json::to_string(candle)
                .map_err(std::io::Error::other)?;
            writeln!(file, "{}", json)?;
        }
        file.flush()?;
        std::fs::rename(&tmp_path, &path)?;
        Ok(())
    })();

    if let Err(e) = result {
        warn!("Failed to persist candles for {} [{}]: {}", epic, resolution, e);
        // Clean up temp file if rename failed
        let _ = std::fs::remove_file(&tmp_path);
    }
}

/// Merge two candle vectors, dedup by timestamp, sort ascending, and trim to
/// the most recent `MAX_CANDLES_PER_SERIES` entries.
pub fn merge_candles(a: Vec<Candle>, b: Vec<Candle>) -> Vec<Candle> {
    let mut combined = a;
    combined.extend(b);
    combined.sort_by_key(|c| c.timestamp);
    combined.dedup_by_key(|c| c.timestamp);
    if combined.len() > MAX_CANDLES_PER_SERIES {
        let start = combined.len() - MAX_CANDLES_PER_SERIES;
        combined = combined[start..].to_vec();
    }
    combined
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn sample_candle(ts: i64) -> Candle {
        Candle {
            timestamp: ts,
            open: 1.0,
            high: 2.0,
            low: 0.5,
            close: 1.5,
            volume: 100,
        }
    }

    #[test]
    fn test_new() {
        let store = CandleStore::new();
        assert_eq!(store.storage.len(), 0);
    }

    #[test]
    fn test_candle_file_path() {
        let path = candle_file_path("CS.D.EURUSD.CSD.IP", "HOUR");
        assert_eq!(
            path,
            PathBuf::from("data/candles/CS_D_EURUSD_CSD_IP_HOUR.jsonl")
        );
    }

    #[test]
    fn test_save_and_load_roundtrip() {
        // Uses a unique epic name to avoid collisions with concurrent tests.
        let epic = "ROUNDTRIP.TEST";
        let resolution = "HOUR";
        let candles = vec![sample_candle(1000), sample_candle(2000), sample_candle(3000)];

        // Write via save_to_disk, read via load_from_disk
        save_to_disk(epic, resolution, &candles);
        let loaded = load_from_disk(epic, resolution);

        assert_eq!(loaded.len(), 3);
        assert_eq!(loaded[0].timestamp, 1000);
        assert_eq!(loaded[2].timestamp, 3000);
        assert!((loaded[1].close - 1.5).abs() < f64::EPSILON);

        // Cleanup
        let _ = fs::remove_file(candle_file_path(epic, resolution));
    }

    #[test]
    fn test_load_nonexistent() {
        // load_from_disk returns empty vec for missing files
        let candles = load_from_disk("NONEXISTENT.EPIC", "HOUR");
        assert!(candles.is_empty());
    }

    #[test]
    fn test_load_corrupt_lines() {
        let epic = "CORRUPT.TEST";
        let resolution = "HOUR";
        let path = candle_file_path(epic, resolution);
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }

        // Write a mix of valid and corrupt lines
        {
            let valid1 = serde_json::to_string(&sample_candle(100)).expect("serialize");
            let valid2 = serde_json::to_string(&sample_candle(200)).expect("serialize");
            let content = format!("{}\nTHIS IS GARBAGE\n{}\n{{\"bad\":true}}\n", valid1, valid2);
            fs::write(&path, content).expect("write test file");
        }

        let loaded = load_from_disk(epic, resolution);
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].timestamp, 100);
        assert_eq!(loaded[1].timestamp, 200);

        // Cleanup
        let _ = fs::remove_file(path);
    }

    #[test]
    fn test_merge_dedup_and_trim() {
        let a = vec![sample_candle(100), sample_candle(200), sample_candle(300)];
        let b = vec![sample_candle(200), sample_candle(400)];

        let merged = merge_candles(a, b);
        assert_eq!(merged.len(), 4);
        assert_eq!(merged[0].timestamp, 100);
        assert_eq!(merged[1].timestamp, 200); // deduped
        assert_eq!(merged[2].timestamp, 300);
        assert_eq!(merged[3].timestamp, 400);
    }

    #[test]
    fn test_merge_trim_to_max() {
        // Create more than MAX_CANDLES_PER_SERIES
        let a: Vec<Candle> = (0..600).map(|i| sample_candle(i)).collect();
        let b: Vec<Candle> = (500..1100).map(|i| sample_candle(i)).collect();
        let merged = merge_candles(a, b);
        assert_eq!(merged.len(), MAX_CANDLES_PER_SERIES);
        // Should keep the most recent 1000 (timestamps 100..1099)
        assert_eq!(merged[0].timestamp, 100);
        assert_eq!(merged.last().expect("non-empty").timestamp, 1099);
    }

    #[test]
    fn test_persist_series_and_all() {
        let mut store = CandleStore::new();
        store.push("A.B", "HOUR", sample_candle(10));
        store.push("A.B", "HOUR", sample_candle(20));
        store.push("C.D", "HOUR", sample_candle(30));

        // persist_series for non-existent epic does nothing (no panic)
        store.persist_series("MISSING", "HOUR");

        // persist_all writes all series to disk
        store.persist_all();

        // Verify files were written and can be read back
        let loaded = load_from_disk("A.B", "HOUR");
        assert_eq!(loaded.len(), 2);

        // Cleanup
        let _ = fs::remove_file(candle_file_path("A.B", "HOUR"));
        let _ = fs::remove_file(candle_file_path("C.D", "HOUR"));
        let _ = fs::remove_dir(CANDLE_DATA_DIR);
    }
}
