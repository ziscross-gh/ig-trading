#!/usr/bin/env python3
"""
fetch_historical_data.py
------------------------
Downloads 2 years of hourly OHLCV data for the 3 IG instruments using yfinance.
Saves as JSON files compatible with the Rust backtester's Candle struct.

Usage:
    pip install yfinance pandas
    python scripts/fetch_historical_data.py

Output:
    data/EURUSD_1H.json
    data/USDJPY_1H.json
    data/GOLD_1H.json
"""

import argparse
import json
import os
from datetime import datetime, timedelta, timezone

try:
    import yfinance as yf
    import pandas as pd
except ImportError:
    print("Missing dependencies. Run: pip install yfinance pandas")
    raise SystemExit(1)

# ── Config ─────────────────────────────────────────────────────────────────────

INSTRUMENTS = {
    "EURUSD": "EURUSD=X",   # CS.D.EURUSD.CSD.IP
    "USDJPY": "JPY=X",      # CS.D.USDJPY.CSD.IP
    "GOLD":   "GC=F",       # CS.D.CFIGOLD.CFI.IP
}

DEFAULT_MONTHS = 24  # 2 years — override with --months N
INTERVAL = "1h"     # Hourly bars — matches the engine's timeframe

OUTPUT_DIR = os.path.join(os.path.dirname(__file__), "..", "data")

# ── Helpers ────────────────────────────────────────────────────────────────────

def to_candle(row, ts: int) -> dict:
    """Convert a yfinance row to Rust Candle JSON format."""
    return {
        "timestamp": ts,                        # Unix seconds (i64 in Rust)
        "open":      round(float(row["Open"]),  6),
        "high":      round(float(row["High"]),  6),
        "low":       round(float(row["Low"]),   6),
        "close":     round(float(row["Close"]), 6),
        "volume":    round(float(row["Volume"] if "Volume" in row else 0), 2),
    }

def fetch_and_save(name: str, ticker: str, months: int) -> None:
    start_dt = datetime.now(tz=timezone.utc) - timedelta(days=months * 30)
    print(f"Fetching {name} ({ticker}) — {months} months from {start_dt.strftime('%Y-%m-%d')} at {INTERVAL} intervals...")

    df = yf.download(ticker, start=start_dt, interval=INTERVAL, progress=False)

    if df.empty:
        print(f"  ⚠️  No data returned for {ticker}")
        return

    # yfinance 1.x returns MultiIndex columns like ('Open', 'EURUSD=X') — flatten them
    if isinstance(df.columns, pd.MultiIndex):
        df.columns = df.columns.get_level_values(0)

    # Drop rows with any NaN OHLC values
    df = df.dropna(subset=["Open", "High", "Low", "Close"])

    candles = []
    for ts, row in df.iterrows():
        # Convert pandas Timestamp → Unix seconds
        if hasattr(ts, "timestamp"):
            unix_ts = int(ts.timestamp())
        else:
            unix_ts = int(pd.Timestamp(ts).timestamp())
        candles.append(to_candle(row, unix_ts))

    out_path = os.path.join(OUTPUT_DIR, f"{name}_1H.json")
    os.makedirs(OUTPUT_DIR, exist_ok=True)

    with open(out_path, "w") as f:
        json.dump(candles, f, indent=2)

    start = datetime.fromtimestamp(candles[0]["timestamp"],  tz=timezone.utc).strftime("%Y-%m-%d")
    end   = datetime.fromtimestamp(candles[-1]["timestamp"], tz=timezone.utc).strftime("%Y-%m-%d")

    print(f"  ✅ {len(candles)} candles saved → {out_path}  [{start} → {end}]")

# ── Main ───────────────────────────────────────────────────────────────────────

def main():
    parser = argparse.ArgumentParser(description="Fetch historical OHLCV data via yfinance")
    parser.add_argument("--months", type=int, default=DEFAULT_MONTHS,
                        help=f"Months of history to fetch (default: {DEFAULT_MONTHS})")
    args = parser.parse_args()

    print("=" * 60)
    print(f"yfinance Historical Data Fetcher — IG Trading Engine ({args.months}mo)")
    print("=" * 60)

    for name, ticker in INSTRUMENTS.items():
        fetch_and_save(name, ticker, args.months)

    print()
    print("Done. Feed these JSON files into the Rust backtester via:")
    print("  POST /backtest  { epic: 'EURUSD', data_file: 'data/EURUSD_1H.json' }")

if __name__ == "__main__":
    main()
