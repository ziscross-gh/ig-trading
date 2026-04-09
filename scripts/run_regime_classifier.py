#!/usr/bin/env python3
"""
run_regime_classifier.py
------------------------
Loads trained regime classifier models, computes features from the latest
candle data, and writes per-instrument regime predictions to:

    data/regime_latest.json

The Rust engine reads this file at each bar close to adjust ensemble weights:
  TRENDING  → boost MA Crossover + MACD (×1.5), mute RSI + Bollinger (×0.3)
  RANGING   → boost RSI + Bollinger (×1.5), mute MA + MACD (×0.3)
  VOLATILE  → mute all signals (×0.4), making consensus harder to reach

Schedule via cron (run every hour, shortly after bar close):
    0 * * * * /path/to/venv/bin/python /path/to/scripts/run_regime_classifier.py

Usage:
    python scripts/run_regime_classifier.py [--once]
"""

import json
import math
import os
import sys
import time
from datetime import datetime, timezone

import warnings
warnings.filterwarnings("ignore")

try:
    import numpy as np
    import pandas as pd
    import joblib
except ImportError:
    sys.exit("Run: pip install numpy pandas scikit-learn joblib")


# ── Config ─────────────────────────────────────────────────────────────────────

LOOKBACK = 50  # must match train_regime_classifier.py

INSTRUMENTS = {
    "EURUSD": {"file": "EURUSD_1H.json", "epic": "CS.D.EURUSD.CSD.IP"},
    "USDJPY": {"file": "USDJPY_1H.json", "epic": "CS.D.USDJPY.CSD.IP"},
    "GOLD":   {"file": "GOLD_1H.json",   "epic": "CS.D.CFIGOLD.CFI.IP"},
}

BASE_DIR   = os.path.join(os.path.dirname(__file__), "..")
DATA_DIR   = os.path.join(BASE_DIR, "data")
MODELS_DIR = os.path.join(BASE_DIR, "models")


# ── Indicators (must match train_regime_classifier.py exactly) ────────────────

def _sma(s: pd.Series, n: int) -> pd.Series:
    return s.rolling(n).mean()

def _ema(s: pd.Series, n: int) -> pd.Series:
    return s.ewm(span=n, adjust=False).mean()

def _rsi(s: pd.Series, n: int = 14) -> pd.Series:
    d = s.diff()
    g = d.clip(lower=0).ewm(alpha=1/n, adjust=False).mean()
    l = (-d).clip(lower=0).ewm(alpha=1/n, adjust=False).mean()
    rs = g / l.replace(0, np.nan)
    return 100 - (100 / (1 + rs))

def _bollinger(s: pd.Series, n: int = 20, k: float = 2.0):
    mid = _sma(s, n)
    sig = s.rolling(n).std()
    return mid + k * sig, mid, mid - k * sig

def _adx(high: pd.Series, low: pd.Series, close: pd.Series, n: int = 14) -> pd.Series:
    ph, pl, pc = high.shift(1), low.shift(1), close.shift(1)
    tr = pd.concat([high - low, (high - pc).abs(), (low - pc).abs()], axis=1).max(axis=1)
    dm_p = np.where((high - ph) > (pl - low), np.maximum(high - ph, 0), 0)
    dm_m = np.where((pl - low) > (high - ph), np.maximum(pl - low, 0), 0)
    atr_ = tr.ewm(alpha=1/n, adjust=False).mean()
    di_p = 100 * pd.Series(dm_p, index=close.index).ewm(alpha=1/n, adjust=False).mean() / atr_.replace(0, np.nan)
    di_m = 100 * pd.Series(dm_m, index=close.index).ewm(alpha=1/n, adjust=False).mean() / atr_.replace(0, np.nan)
    dx   = 100 * (di_p - di_m).abs() / (di_p + di_m).replace(0, np.nan)
    return dx.ewm(alpha=1/n, adjust=False).mean()

def _atr(high: pd.Series, low: pd.Series, close: pd.Series, n: int = 14) -> pd.Series:
    pc = close.shift(1)
    tr = pd.concat([high - low, (high - pc).abs(), (low - pc).abs()], axis=1).max(axis=1)
    return tr.ewm(alpha=1/n, adjust=False).mean()

def _hurst(returns: np.ndarray, max_lag: int = 20) -> float:
    """
    Hurst exponent via Rescaled Range (R/S) analysis.
    H > 0.5 → trending (persistent momentum)
    H < 0.5 → mean-reverting (anti-persistent)
    H ≈ 0.5 → random walk
    """
    N = len(returns)
    if N < 20:
        return 0.5
    # Use chunk sizes from 8 up to N//2
    chunk_sizes = []
    s = 8
    while s <= N // 2:
        chunk_sizes.append(s)
        s = int(s * 1.5)
    if len(chunk_sizes) < 3:
        return 0.5
    rs_values = []
    for cs in chunk_sizes:
        n_chunks = N // cs
        if n_chunks < 1:
            continue
        rs_chunk = []
        for i in range(n_chunks):
            chunk = returns[i * cs : (i + 1) * cs]
            mean_c = np.mean(chunk)
            cumdev = np.cumsum(chunk - mean_c)
            R = np.max(cumdev) - np.min(cumdev)
            S = np.std(chunk, ddof=1)
            if S > 0:
                rs_chunk.append(R / S)
        if rs_chunk:
            rs_values.append((cs, np.mean(rs_chunk)))
    if len(rs_values) < 3:
        return 0.5
    try:
        log_n = np.log([v[0] for v in rs_values])
        log_rs = np.log([v[1] for v in rs_values])
        slope = np.polyfit(log_n, log_rs, 1)[0]
        return float(np.clip(slope, 0.1, 0.9))
    except Exception:
        return 0.5

def _safe(v, default: float) -> float:
    try:
        f = float(v)
        return f if math.isfinite(f) else default
    except Exception:
        return default


# ── Feature extraction ─────────────────────────────────────────────────────────

def compute_features(df: pd.DataFrame) -> dict:
    """Compute features from the last LOOKBACK candles of df (plus SMA200 from full history)."""
    win  = df.iloc[-LOOKBACK:]
    close, high, low = win["close"], win["high"], win["low"]
    vol  = win["volume"] if "volume" in win.columns else pd.Series(np.zeros(len(win)), index=win.index)

    adx_val  = _adx(high, low, close, 14).iloc[-1]
    atr_val  = _atr(high, low, close, 14).iloc[-1]
    atr_pct  = atr_val / close.iloc[-1] * 100 if close.iloc[-1] > 0 else 0.5
    bb_u, bb_m, bb_l = _bollinger(close, 20, 2.0)
    bb_width = ((bb_u - bb_l) / bb_m.replace(0, np.nan)).iloc[-1]
    rsi_val  = _rsi(close, 14).iloc[-1]
    sma200   = _sma(df["close"], 200).iloc[-1]
    p_vs_200 = (close.iloc[-1] - sma200) / sma200 if sma200 > 0 else 0.0
    vol_mean = vol.mean()
    vol_ratio = vol.iloc[-20:].mean() / vol_mean if vol_mean > 0 else 1.0
    returns  = close.pct_change().dropna().values
    hurst    = _hurst(returns, 20)

    return {
        "adx_14":          _safe(adx_val,  20.0),
        "atr_pct":         _safe(atr_pct,   0.5),
        "bb_width":        _safe(bb_width,  0.02),
        "rsi_14":          _safe(rsi_val,  50.0),
        "price_vs_sma200": _safe(p_vs_200,  0.0),
        "vol_ratio":       _safe(vol_ratio, 1.0),
        "hurst":           hurst,
    }


# ── Inference ──────────────────────────────────────────────────────────────────

def run_once() -> dict:
    output  = {}
    ts_now  = int(time.time())
    dt_str  = datetime.now(tz=timezone.utc).strftime("%Y-%m-%d %H:%M UTC")
    print(f"\n🔬 Regime Classifier — {dt_str}")

    for name, cfg in INSTRUMENTS.items():
        model_path = os.path.join(MODELS_DIR, f"regime_{name}.pkl")
        data_path  = os.path.join(DATA_DIR, cfg["file"])

        if not os.path.exists(model_path):
            print(f"  ⚠️  [{name}] No model found — run train_regime_classifier.py first")
            continue
        if not os.path.exists(data_path):
            print(f"  ⚠️  [{name}] No data file — run fetch_historical_data.py first")
            continue

        bundle       = joblib.load(model_path)
        model        = bundle["model"]
        feature_cols = bundle["feature_cols"]

        with open(data_path) as f:
            candles = json.load(f)

        df = pd.DataFrame(candles)
        min_rows = LOOKBACK + 200
        if len(df) < min_rows:
            print(f"  ⚠️  [{name}] Only {len(df)} candles (need {min_rows}) — skipping")
            continue

        feats = compute_features(df)
        X     = np.array([[feats[c] for c in feature_cols]])

        regime     = model.predict(X)[0]
        proba      = model.predict_proba(X)[0]
        confidence = float(max(proba))

        # Hurst-based override: the ML model was trained with broken hurst=0.1,
        # so it can't use Hurst to distinguish regimes. Apply rule-based correction:
        #   H >= 0.65 + ADX >= 25 → TRENDING (persistent momentum confirmed by ADX)
        #   H <= 0.35            → RANGING  (strong mean-reversion)
        hurst_val = feats["hurst"]
        adx_val   = feats["adx_14"]
        override  = None
        if hurst_val >= 0.65 and adx_val >= 25.0 and regime == "VOLATILE":
            override = "TRENDING"
            confidence = max(confidence, 0.70)
        elif hurst_val <= 0.35 and regime == "VOLATILE":
            override = "RANGING"
            confidence = max(confidence, 0.60)
        if override:
            print(f"    ↳ Hurst override: {regime} → {override} (H={hurst_val:.2f}, ADX={adx_val:.1f})")
            regime = override

        epic = cfg["epic"]
        output[epic] = {
            "regime":     regime,
            "confidence": round(confidence, 4),
            "instrument": name,
            "timestamp":  ts_now,
            "features": {k: round(v, 4) for k, v in feats.items()},
        }

        # Pretty print with regime indicator
        icon = {"TRENDING": "📈", "RANGING": "↔️", "VOLATILE": "⚡"}.get(regime, "❓")
        print(
            f"  {icon} {name:8s} → {regime:10s}  conf={confidence:.2f}"
            f"  | ADX={feats['adx_14']:5.1f}  Hurst={feats['hurst']:.2f}"
            f"  RSI={feats['rsi_14']:5.1f}  BB-width={feats['bb_width']:.4f}"
        )

    if not output:
        print("  No predictions generated — check models and data files exist.")
        return output

    out_path = os.path.join(DATA_DIR, "regime_latest.json")
    with open(out_path, "w") as f:
        json.dump(output, f, indent=2)
    print(f"\n  ✅ Predictions written → {out_path}")
    return output


# ── Entry point ────────────────────────────────────────────────────────────────

if __name__ == "__main__":
    run_once()
