#!/usr/bin/env python3
"""
train_regime_classifier.py
--------------------------
Generates regime labels from historical OHLCV data and trains an ML classifier
(LightGBM with scikit-learn fallback) per instrument.

Labels (assigned from forward-window mini-backtest):
  TRENDING  — MA Crossover + MACD outperform in the next 60 candles
  RANGING   — RSI Reversal + Bollinger outperform in the next 60 candles
  VOLATILE  — all strategies lose (engine will mute signals and reduce sizing)

Features (computed on trailing 50-candle window):
  adx_14, atr_pct, bb_width, rsi_14, price_vs_sma200, vol_ratio, hurst_exp

Walk-forward cross-validation: no look-ahead bias in features.

Usage:
    pip install scikit-learn joblib pandas numpy lightgbm
    python scripts/train_regime_classifier.py
    python scripts/train_regime_classifier.py --instrument GOLD

Output:
    models/regime_GOLD.pkl
    models/regime_EURUSD.pkl
    models/regime_USDJPY.pkl
    data/regime_labels_GOLD.csv    (for inspection / debugging)
"""

import argparse
import json
import math
import os
import sys
import warnings
from typing import Optional

warnings.filterwarnings("ignore")

try:
    import numpy as np
    import pandas as pd
except ImportError:
    sys.exit("Run: pip install numpy pandas scikit-learn joblib")

try:
    import lightgbm as lgb
    USE_LGBM = True
except ImportError:
    USE_LGBM = False

try:
    from sklearn.ensemble import GradientBoostingClassifier
    from sklearn.model_selection import TimeSeriesSplit
    from sklearn.metrics import classification_report
    import joblib
except ImportError:
    sys.exit("Run: pip install scikit-learn joblib")


# ── Constants ──────────────────────────────────────────────────────────────────

LOOKBACK         = 50    # trailing candles used for feature computation
FORWARD          = 60    # forward candles used to determine regime label
STEP             = 5     # stride between training samples
MIN_TREND_FACTOR = 1.1   # trend P&L must beat reversion by this factor to be TRENDING
MIN_SAMPLES      = 60    # abort training if fewer samples than this

INSTRUMENTS = {
    "EURUSD": {"file": "EURUSD_1H.json", "epic": "CS.D.EURUSD.CSD.IP", "sl_pct": 0.5, "tp_pct": 1.5},
    "USDJPY": {"file": "USDJPY_1H.json", "epic": "CS.D.USDJPY.CSD.IP", "sl_pct": 0.5, "tp_pct": 1.5},
    "GOLD":   {"file": "GOLD_1H.json",   "epic": "CS.D.CFIGOLD.CFI.IP","sl_pct": 1.0, "tp_pct": 3.0},
}

FEATURE_COLS = ["adx_14", "atr_pct", "bb_width", "rsi_14", "price_vs_sma200", "vol_ratio", "hurst"]

BASE_DIR   = os.path.join(os.path.dirname(__file__), "..")
DATA_DIR   = os.path.join(BASE_DIR, "data")
MODELS_DIR = os.path.join(BASE_DIR, "models")


# ── Technical indicators ───────────────────────────────────────────────────────

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
    Hurst exponent via variance scaling.
    H > 0.5 → trending (persistent momentum)
    H < 0.5 → mean-reverting (anti-persistent)
    H ≈ 0.5 → random walk
    """
    lags = list(range(2, min(max_lag, len(returns) // 2)))
    if len(lags) < 3:
        return 0.5
    tau = []
    for lag in lags:
        diff = returns[lag:] - returns[:-lag]
        std  = np.std(diff)
        tau.append(std if std > 0 else np.nan)
    tau = np.array(tau, dtype=float)
    valid = ~np.isnan(tau)
    if valid.sum() < 3:
        return 0.5
    try:
        slope = np.polyfit(np.log(np.array(lags)[valid]), np.log(tau[valid]), 1)[0]
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

def compute_features(df: pd.DataFrame, i: int) -> Optional[dict]:
    """
    Compute features from df[i-LOOKBACK:i] (trailing window).
    Needs df[0:i] for SMA200.
    Returns None if data is insufficient or all indicators are NaN.
    """
    if i < max(LOOKBACK, 205):   # need 200+ for SMA200
        return None

    win  = df.iloc[i - LOOKBACK: i]
    hist = df.iloc[: i]           # for SMA200
    close, high, low = win["close"], win["high"], win["low"]
    vol = win["volume"] if "volume" in win.columns else pd.Series(np.zeros(len(win)), index=win.index)

    adx_val  = _adx(high, low, close, 14).iloc[-1]
    atr_val  = _atr(high, low, close, 14).iloc[-1]
    atr_pct  = atr_val / close.iloc[-1] * 100 if close.iloc[-1] > 0 else 0.5
    bb_u, bb_m, bb_l = _bollinger(close, 20, 2.0)
    bb_width = ((bb_u - bb_l) / bb_m.replace(0, np.nan)).iloc[-1]
    rsi_val  = _rsi(close, 14).iloc[-1]
    sma200   = _sma(hist["close"], 200).iloc[-1]
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


# ── Label generation (mini-backtest on forward window) ────────────────────────

def _mini_backtest(window: pd.DataFrame, strategy_set: str, sl_pct: float, tp_pct: float) -> float:
    """
    Run either trend (MA + MACD) or reversion (RSI + Bollinger) strategies
    on the forward window and return cumulative P&L as % of initial price.
    """
    df = window.copy().reset_index(drop=True)
    close, high, low = df["close"], df["high"], df["low"]

    df["sma9"]  = _sma(close, 9)
    df["sma21"] = _sma(close, 21)
    df["rsi"]   = _rsi(close, 14)
    ml, ms      = _ema(close, 12) - _ema(close, 26), None
    ms          = _ema(ml, 9)
    df["macd_line"] = ml
    df["macd_sig"]  = ms
    bb_u, _, bb_l   = _bollinger(close, 20)
    df["bb_u"]  = bb_u
    df["bb_l"]  = bb_l

    balance = 1000.0
    active  = None  # (direction, entry, sl, tp)

    for i in range(1, len(df)):
        r    = df.iloc[i]
        prev = df.iloc[i - 1]
        price = r["close"]

        if active:
            direction, entry, sl, tp = active
            if direction == "Buy":
                if r["low"] <= sl:
                    balance += balance * (sl - entry) / entry
                    active   = None
                    continue
                if r["high"] >= tp:
                    balance += balance * (tp - entry) / entry
                    active   = None
                    continue
            else:
                if r["high"] >= sl:
                    balance += balance * (entry - sl) / entry
                    active   = None
                    continue
                if r["low"] <= tp:
                    balance += balance * (entry - tp) / entry
                    active   = None
                    continue
            continue   # still in trade

        direction = None

        if strategy_set == "trend":
            # MA Crossover
            if all(math.isfinite(v) for v in [r["sma9"], r["sma21"], prev["sma9"], prev["sma21"]]):
                if prev["sma9"] <= prev["sma21"] and r["sma9"] > r["sma21"]:
                    direction = "Buy"
                elif prev["sma9"] >= prev["sma21"] and r["sma9"] < r["sma21"]:
                    direction = "Sell"
            # MACD (only take if agrees with MA, or if MA gave no signal)
            if all(math.isfinite(v) for v in [r["macd_line"], r["macd_sig"], prev["macd_line"], prev["macd_sig"]]):
                macd_dir = None
                if prev["macd_line"] <= prev["macd_sig"] and r["macd_line"] > r["macd_sig"] and r["macd_line"] < 0:
                    macd_dir = "Buy"
                elif prev["macd_line"] >= prev["macd_sig"] and r["macd_line"] < r["macd_sig"] and r["macd_line"] > 0:
                    macd_dir = "Sell"
                if macd_dir and macd_dir == direction:  # agreement strengthens conviction
                    pass  # direction already set; keep it
                elif macd_dir and direction is None:
                    direction = macd_dir
                elif macd_dir and macd_dir != direction:
                    direction = None  # conflict → skip

        elif strategy_set == "reversion":
            # RSI Reversal
            if all(math.isfinite(v) for v in [r["rsi"], prev["rsi"]]):
                if prev["rsi"] <= 30 and r["rsi"] > 30:
                    direction = "Buy"
                elif prev["rsi"] >= 70 and r["rsi"] < 70:
                    direction = "Sell"
            # Bollinger Reversion
            if all(math.isfinite(v) for v in [r["bb_u"], r["bb_l"], prev["bb_u"], prev["bb_l"]]):
                bb_dir = None
                if prev["close"] <= prev["bb_l"] and r["close"] > r["bb_l"]:
                    bb_dir = "Buy"
                elif prev["close"] >= prev["bb_u"] and r["close"] < r["bb_u"]:
                    bb_dir = "Sell"
                if bb_dir and bb_dir == direction:
                    pass
                elif bb_dir and direction is None:
                    direction = bb_dir
                elif bb_dir and bb_dir != direction:
                    direction = None

        if direction:
            sl = price * (1 - sl_pct / 100) if direction == "Buy" else price * (1 + sl_pct / 100)
            tp = price * (1 + tp_pct / 100) if direction == "Buy" else price * (1 - tp_pct / 100)
            active = (direction, price, sl, tp)

    return (balance - 1000.0) / 10.0   # return as % of initial


def label_window(df: pd.DataFrame, i: int, sl_pct: float, tp_pct: float) -> Optional[str]:
    """Determine the regime label at position i by forward-testing both strategy families."""
    if i + FORWARD > len(df):
        return None

    fwd = df.iloc[i: i + FORWARD].copy()
    trend_pnl     = _mini_backtest(fwd, "trend",     sl_pct, tp_pct)
    reversion_pnl = _mini_backtest(fwd, "reversion", sl_pct, tp_pct)

    if trend_pnl > 0 and trend_pnl > abs(reversion_pnl) * MIN_TREND_FACTOR:
        return "TRENDING"
    elif reversion_pnl > 0 and reversion_pnl > abs(trend_pnl) * MIN_TREND_FACTOR:
        return "RANGING"
    else:
        return "VOLATILE"


# ── Dataset builder ────────────────────────────────────────────────────────────

def build_dataset(name: str, df: pd.DataFrame, cfg: dict) -> pd.DataFrame:
    """Slide through the candle history, extracting features + labels."""
    rows = []
    positions = list(range(LOOKBACK, len(df) - FORWARD, STEP))
    total = len(positions)

    print(f"  Generating {total} windows (lookback={LOOKBACK}, forward={FORWARD}, step={STEP})…")
    for idx, i in enumerate(positions):
        feats = compute_features(df, i)
        if feats is None:
            continue
        label = label_window(df, i, cfg["sl_pct"], cfg["tp_pct"])
        if label is None:
            continue
        rows.append({"timestamp": int(df["timestamp"].iloc[i]), **feats, "label": label})
        if (idx + 1) % 200 == 0:
            print(f"    {idx + 1}/{total} …", end="\r", flush=True)

    print(f"    Done: {len(rows)} labelled samples generated.          ")
    return pd.DataFrame(rows)


# ── Model factory ──────────────────────────────────────────────────────────────

def make_model(n_estimators: int = 200):
    if USE_LGBM:
        return lgb.LGBMClassifier(
            n_estimators=n_estimators,
            learning_rate=0.05,
            max_depth=5,
            num_leaves=31,
            min_child_samples=10,
            class_weight="balanced",
            random_state=42,
            verbose=-1,
        )
    return GradientBoostingClassifier(
        n_estimators=n_estimators // 2,
        learning_rate=0.05,
        max_depth=4,
        min_samples_leaf=5,
        random_state=42,
    )


# ── Training pipeline ──────────────────────────────────────────────────────────

def train_instrument(name: str, candles: list, cfg: dict) -> None:
    print(f"\n{'='*60}")
    print(f"  {name}  ({cfg['epic']})")
    print(f"  Model: {'LightGBM' if USE_LGBM else 'sklearn GradientBoosting'}")
    print(f"{'='*60}")

    df = pd.DataFrame(candles)
    df["dt"] = pd.to_datetime(df["timestamp"], unit="s", utc=True)
    print(f"  Candles loaded: {len(df):,}  [{df['dt'].iloc[0].date()} → {df['dt'].iloc[-1].date()}]")

    labels_df = build_dataset(name, df, cfg)
    if len(labels_df) < MIN_SAMPLES:
        print(f"  ⚠️  Only {len(labels_df)} samples (need {MIN_SAMPLES}) — skipping {name}")
        return

    # Save labels for inspection
    os.makedirs(DATA_DIR, exist_ok=True)
    csv_path = os.path.join(DATA_DIR, f"regime_labels_{name}.csv")
    labels_df.to_csv(csv_path, index=False)

    # Label distribution
    dist = labels_df["label"].value_counts()
    print(f"\n  Label distribution ({len(labels_df)} samples):")
    for label, count in dist.items():
        bar = "█" * int(count / len(labels_df) * 40)
        print(f"    {label:12s}: {count:4d} ({count/len(labels_df)*100:.1f}%)  {bar}")
    print(f"  Labels → {csv_path}")

    X = labels_df[FEATURE_COLS].values
    y = labels_df["label"].values

    # Walk-forward cross-validation (respects time ordering)
    tscv      = TimeSeriesSplit(n_splits=5)
    cv_scores = []
    print(f"\n  Walk-forward CV:")
    for fold, (tr_idx, val_idx) in enumerate(tscv.split(X)):
        m = make_model(200)
        m.fit(X[tr_idx], y[tr_idx])
        score = m.score(X[val_idx], y[val_idx])
        cv_scores.append(score)
        print(f"    Fold {fold+1}: {score:.3f}")
    print(f"  Mean accuracy: {np.mean(cv_scores):.3f} ± {np.std(cv_scores):.3f}")

    # Final model on all data
    final = make_model(300)
    final.fit(X, y)

    # Feature importances
    if hasattr(final, "feature_importances_"):
        imp_pairs = sorted(zip(FEATURE_COLS, final.feature_importances_), key=lambda x: -x[1])
        print(f"\n  Feature importances:")
        for feat, imp in imp_pairs:
            bar = "█" * int(imp * 60)
            print(f"    {feat:20s}: {imp:.4f}  {bar}")

    # Sanity-check: last 20 predictions
    preds = final.predict(X[-20:])
    print(f"\n  Last 20 predictions: {list(preds)}")

    # Save model bundle
    os.makedirs(MODELS_DIR, exist_ok=True)
    bundle = {"model": final, "feature_cols": FEATURE_COLS, "name": name, "epic": cfg["epic"]}
    model_path = os.path.join(MODELS_DIR, f"regime_{name}.pkl")
    joblib.dump(bundle, model_path)
    print(f"\n  ✅ Model saved → {model_path}")


# ── Entry point ────────────────────────────────────────────────────────────────

def main():
    parser = argparse.ArgumentParser(description="Train ML regime classifier per instrument")
    parser.add_argument("--instrument", default=None,
                        help="EURUSD | USDJPY | GOLD  (default: all)")
    args = parser.parse_args()

    targets = {args.instrument: INSTRUMENTS[args.instrument]} if args.instrument else INSTRUMENTS

    print("\n🤖 Regime Classifier — Training Pipeline")
    print(f"   Lookback={LOOKBACK}  Forward={FORWARD}  Step={STEP}")
    print(f"   Backend: {'LightGBM' if USE_LGBM else 'sklearn GradientBoosting (install lightgbm for speed)'}")

    for name, cfg in targets.items():
        path = os.path.join(DATA_DIR, cfg["file"])
        if not os.path.exists(path):
            print(f"\n⚠️  {path} not found — run fetch_historical_data.py first")
            continue
        with open(path) as f:
            candles = json.load(f)
        train_instrument(name, candles, cfg)

    print("\n✅ Done. Run run_regime_classifier.py to generate live predictions.")


if __name__ == "__main__":
    main()
