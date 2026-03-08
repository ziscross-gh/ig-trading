#!/usr/bin/env python3
"""
backtest.py — IG Trading Engine Python Backtester
--------------------------------------------------
Per-instrument strategy sets (Gold ≠ FX):
  - GOLD : MA Crossover + MACD only  (trend-following — Gold trends strongly)
  - FX   : All 4 strategies + ADX range filter (skip RSI/Bollinger in trending markets)
  - Ensemble vote: min 2 agree, avg strength >= 6.0
  - Session filter: 07:00–20:00 UTC only
  - Trailing stop ratchet (activates after 0.3% profit, trails at 1.5%)
  - Risk: 1% of balance per trade

Usage:
    python scripts/backtest.py                        # all instruments
    python scripts/backtest.py --epic EURUSD          # single instrument
    python scripts/backtest.py --epic GOLD --balance 5000
"""

import argparse
import json
import os
import math
from datetime import datetime, timezone
from dataclasses import dataclass, field
from typing import Optional
import warnings
warnings.filterwarnings("ignore")

try:
    import numpy as np
    import pandas as pd
except ImportError:
    raise SystemExit("Run: pip install numpy pandas")

# ── Global constants ──────────────────────────────────────────────────────────

INITIAL_BALANCE    = 10_000.0
RISK_PER_TRADE_PCT = 1.0
SESSION_START_UTC  = 7
SESSION_END_UTC    = 20
MIN_STRATEGIES     = 2
MIN_AVG_STRENGTH   = 6.0
MAX_DRAWDOWN_HALT  = 5.0

DATA_DIR = os.path.join(os.path.dirname(__file__), "..", "data")

INSTRUMENTS = {
    "EURUSD": "EURUSD_1H.json",
    "USDJPY": "USDJPY_1H.json",
    "GOLD":   "GOLD_1H.json",
}

# ── Per-instrument config ─────────────────────────────────────────────────────
# Gold is a commodity that trends strongly — use trend-following strategies only.
# FX pairs range more — use all 4 strategies but filter mean-reversion when trending.

INSTRUMENT_CONFIG = {
    "EURUSD": {
        "strategy_set":      "all",   # MA Cross + RSI + MACD + Bollinger
        "adx_range_filter":  True,    # skip RSI/Bollinger when ADX > adx_max
        "adx_max":           25.0,
        "sl_pct":            2.0,
        "tp_pct":            6.0,
        "trail_act_pct":     0.3,
        "trail_dist_pct":    1.5,
    },
    "USDJPY": {
        "strategy_set":      "all",
        "adx_range_filter":  True,
        "adx_max":           25.0,
        "sl_pct":            1.5,
        "tp_pct":            6.0,
        "trail_act_pct":     0.5,
        "trail_dist_pct":    1.5,
    },
    "GOLD": {
        # Gold uses all 4 strategies but with ADX range filter (same as FX).
        # Key difference: tighter SL (1.0%), wider TP (5.0%) — Gold moves in larger %
        # Gold rarely ranges (ADX usually high) so RSI/Bollinger naturally fire less often.
        "strategy_set":      "all",
        "adx_range_filter":  True,
        "adx_max":           25.0,
        "sl_pct":            1.0,
        "tp_pct":            5.0,
        "trail_act_pct":     0.3,
        "trail_dist_pct":    1.5,
    },
}

# Strategies belonging to each set
TREND_STRATEGIES     = {"MACross", "MACD"}
REVERSION_STRATEGIES = {"RSIReversal", "Bollinger"}

# ── Data structures ───────────────────────────────────────────────────────────

@dataclass
class Signal:
    direction: str          # "Buy" | "Sell"
    strength:  float        # 0.0–10.0
    strategy:  str

@dataclass
class Trade:
    direction:   str
    entry_price: float
    entry_time:  datetime
    size:        float
    strategy:    str
    sl:          float      # current stop level (moves with trailing)
    tp:          float
    trail_active: bool = False
    exit_price:  Optional[float] = None
    exit_time:   Optional[datetime] = None
    exit_reason: Optional[str] = None
    pnl:         Optional[float] = None

# ── Indicators (pandas-based, mirrors Rust implementations) ───────────────────

def sma(series: pd.Series, n: int) -> pd.Series:
    return series.rolling(n).mean()

def ema(series: pd.Series, n: int) -> pd.Series:
    return series.ewm(span=n, adjust=False).mean()

def rsi(series: pd.Series, n: int = 14) -> pd.Series:
    delta = series.diff()
    gain  = delta.clip(lower=0)
    loss  = (-delta).clip(lower=0)
    avg_gain = gain.ewm(alpha=1/n, adjust=False).mean()
    avg_loss = loss.ewm(alpha=1/n, adjust=False).mean()
    rs = avg_gain / avg_loss.replace(0, np.nan)
    return 100 - (100 / (1 + rs))

def macd(series: pd.Series, fast=12, slow=26, signal=9):
    ema_fast   = ema(series, fast)
    ema_slow   = ema(series, slow)
    macd_line  = ema_fast - ema_slow
    signal_line = ema(macd_line, signal)
    return macd_line, signal_line

def bollinger(series: pd.Series, n=20, std_dev=2.0):
    mid   = sma(series, n)
    sigma = series.rolling(n).std()
    upper = mid + std_dev * sigma
    lower = mid - std_dev * sigma
    return upper, mid, lower

def adx(high: pd.Series, low: pd.Series, close: pd.Series, n: int = 14) -> pd.Series:
    """Wilder's ADX — matches Rust ADX indicator."""
    prev_high  = high.shift(1)
    prev_low   = low.shift(1)
    prev_close = close.shift(1)

    tr = pd.concat([
        high - low,
        (high - prev_close).abs(),
        (low  - prev_close).abs(),
    ], axis=1).max(axis=1)

    dm_plus  = np.where((high - prev_high) > (prev_low - low),
                        np.maximum(high - prev_high, 0), 0)
    dm_minus = np.where((prev_low - low) > (high - prev_high),
                        np.maximum(prev_low - low, 0), 0)

    dm_plus_s  = pd.Series(dm_plus,  index=close.index).ewm(alpha=1/n, adjust=False).mean()
    dm_minus_s = pd.Series(dm_minus, index=close.index).ewm(alpha=1/n, adjust=False).mean()
    atr_s      = tr.ewm(alpha=1/n, adjust=False).mean()

    di_plus  = 100 * dm_plus_s  / atr_s.replace(0, np.nan)
    di_minus = 100 * dm_minus_s / atr_s.replace(0, np.nan)
    dx       = 100 * (di_plus - di_minus).abs() / (di_plus + di_minus).replace(0, np.nan)
    return dx.ewm(alpha=1/n, adjust=False).mean()

# ── Strategy signals ──────────────────────────────────────────────────────────

def strategy_ma_crossover(df: pd.DataFrame, i: int) -> Optional[Signal]:
    """SMA 9/21 crossover + ADX > 25."""
    if i < 1:
        return None
    fast_now, fast_prev = df["sma9"].iloc[i],  df["sma9"].iloc[i-1]
    slow_now, slow_prev = df["sma21"].iloc[i], df["sma21"].iloc[i-1]
    adx_val = df["adx"].iloc[i]

    if any(math.isnan(v) for v in [fast_now, fast_prev, slow_now, slow_prev, adx_val]):
        return None
    if adx_val <= 25:
        return None

    # Strength scales with ADX: base 7.0, max 9.5
    strength = min(9.5, 7.0 + (adx_val - 25) / 30)

    if fast_prev <= slow_prev and fast_now > slow_now:
        return Signal("Buy",  strength, "MACross")
    if fast_prev >= slow_prev and fast_now < slow_now:
        return Signal("Sell", strength, "MACross")
    return None

def strategy_rsi_reversal(df: pd.DataFrame, i: int) -> Optional[Signal]:
    """RSI 14: crossover of 30 (buy) / 70 (sell)."""
    if i < 1:
        return None
    rsi_now, rsi_prev = df["rsi"].iloc[i], df["rsi"].iloc[i-1]
    if math.isnan(rsi_now) or math.isnan(rsi_prev):
        return None

    if rsi_prev <= 30 and rsi_now > 30:
        return Signal("Buy",  8.0, "RSIReversal")
    if rsi_prev >= 70 and rsi_now < 70:
        return Signal("Sell", 8.0, "RSIReversal")
    return None

def strategy_macd_momentum(df: pd.DataFrame, i: int) -> Optional[Signal]:
    """MACD 12/26/9 crossover below/above zero line."""
    if i < 1:
        return None
    m_now, m_prev = df["macd"].iloc[i],   df["macd"].iloc[i-1]
    s_now, s_prev = df["msig"].iloc[i], df["msig"].iloc[i-1]
    if any(math.isnan(v) for v in [m_now, m_prev, s_now, s_prev]):
        return None

    if m_prev <= s_prev and m_now > s_now and m_now < 0:
        return Signal("Buy",  7.0, "MACD")
    if m_prev >= s_prev and m_now < s_now and m_now > 0:
        return Signal("Sell", 7.0, "MACD")
    return None

def strategy_bollinger_reversion(df: pd.DataFrame, i: int) -> Optional[Signal]:
    """Price crosses back inside Bollinger Band (mean reversion)."""
    if i < 1:
        return None
    close_now  = df["close"].iloc[i]
    close_prev = df["close"].iloc[i-1]
    upper = df["bb_upper"].iloc[i]
    lower = df["bb_lower"].iloc[i]
    if any(math.isnan(v) for v in [close_now, close_prev, upper, lower]):
        return None

    if close_prev <= lower and close_now > lower:
        return Signal("Buy",  6.5, "Bollinger")
    if close_prev >= upper and close_now < upper:
        return Signal("Sell", 6.5, "Bollinger")
    return None

STRATEGIES = [
    strategy_ma_crossover,
    strategy_rsi_reversal,
    strategy_macd_momentum,
    strategy_bollinger_reversion,
]

# ── Ensemble voter (mirrors EnsembleVoter in Rust) ────────────────────────────

def ensemble_vote(df: pd.DataFrame, i: int, icfg: dict) -> Optional[Signal]:
    """Vote across strategies, respecting per-instrument strategy set + ADX filter."""
    adx_val = df["adx"].iloc[i] if "adx" in df.columns else float("nan")

    raw = [s(df, i) for s in STRATEGIES]
    filtered = []
    for sig in raw:
        if sig is None:
            continue
        # Gold: trend-only — skip mean-reversion strategies
        if icfg["strategy_set"] == "trend" and sig.strategy in REVERSION_STRATEGIES:
            continue
        # FX: ADX range filter — skip mean-reversion in trending markets
        if icfg["adx_range_filter"] and sig.strategy in REVERSION_STRATEGIES:
            if not math.isnan(adx_val) and adx_val > icfg["adx_max"]:
                continue
        filtered.append(sig)

    for direction in ("Buy", "Sell"):
        matching = [s for s in filtered if s.direction == direction]
        if len(matching) >= MIN_STRATEGIES:
            avg_strength = sum(s.strength for s in matching) / len(matching)
            if avg_strength >= MIN_AVG_STRENGTH:
                names = "+".join(s.strategy for s in matching)
                return Signal(direction, avg_strength, names)
    return None

# ── Session filter ────────────────────────────────────────────────────────────

def in_session(ts: datetime) -> bool:
    hour = ts.hour
    return SESSION_START_UTC <= hour < SESSION_END_UTC

# ── Trade management ──────────────────────────────────────────────────────────

def open_trade(signal: Signal, candle: pd.Series, balance: float, icfg: dict) -> Trade:
    price    = candle["close"]
    sl_pct   = icfg["sl_pct"]
    tp_pct   = icfg["tp_pct"]
    risk_amt = balance * (RISK_PER_TRADE_PCT / 100)
    sl_dist  = price  * (sl_pct / 100)
    size     = max(1.0, round(risk_amt / sl_dist, 2))

    if signal.direction == "Buy":
        sl = price * (1 - sl_pct / 100)
        tp = price * (1 + tp_pct / 100)
    else:
        sl = price * (1 + sl_pct / 100)
        tp = price * (1 - tp_pct / 100)

    return Trade(
        direction   = signal.direction,
        entry_price = price,
        entry_time  = candle["dt"],
        size        = size,
        strategy    = signal.strategy,
        sl          = sl,
        tp          = tp,
    )

def update_trailing_stop(trade: Trade, price: float, icfg: dict) -> None:
    """Ratchet trailing stop — only moves in profitable direction."""
    pnl_pct = (
        (price - trade.entry_price) / trade.entry_price * 100
        if trade.direction == "Buy"
        else (trade.entry_price - price) / trade.entry_price * 100
    )

    if pnl_pct >= icfg["trail_act_pct"]:
        trade.trail_active = True

    if trade.trail_active:
        if trade.direction == "Buy":
            new_sl = price * (1 - icfg["trail_dist_pct"] / 100)
            trade.sl = max(trade.sl, new_sl)  # ratchet up only
        else:
            new_sl = price * (1 + icfg["trail_dist_pct"] / 100)
            trade.sl = min(trade.sl, new_sl)  # ratchet down only

def check_exit(trade: Trade, candle: pd.Series) -> Optional[str]:
    """Returns exit reason or None."""
    high, low = candle["high"], candle["low"]

    if trade.direction == "Buy":
        if low  <= trade.sl: return "SL"
        if high >= trade.tp: return "TP"
    else:
        if high >= trade.sl: return "SL"
        if low  <= trade.tp: return "TP"
    return None

def close_trade(trade: Trade, candle: pd.Series, reason: str) -> float:
    price = trade.sl if reason == "SL" else trade.tp
    pnl_pct = (
        (price - trade.entry_price) / trade.entry_price * 100
        if trade.direction == "Buy"
        else (trade.entry_price - price) / trade.entry_price * 100
    )
    pnl = trade.size * (pnl_pct / 100) * trade.entry_price
    trade.exit_price  = price
    trade.exit_time   = candle["dt"]
    trade.exit_reason = reason
    trade.pnl         = pnl
    return pnl

# ── Main backtest loop ────────────────────────────────────────────────────────

def run_backtest(name: str, candles_raw: list, initial_balance: float, icfg: Optional[dict] = None) -> dict:
    icfg = icfg or INSTRUMENT_CONFIG.get(name, INSTRUMENT_CONFIG["EURUSD"])
    # Build DataFrame
    df = pd.DataFrame(candles_raw)
    df["dt"] = pd.to_datetime(df["timestamp"], unit="s", utc=True)

    # Pre-compute all indicators
    df["sma9"]     = sma(df["close"], 9)
    df["sma21"]    = sma(df["close"], 21)
    df["rsi"]      = rsi(df["close"], 14)
    df["macd"], df["msig"] = macd(df["close"])
    df["bb_upper"], _, df["bb_lower"] = bollinger(df["close"])
    df["adx"]      = adx(df["high"], df["low"], df["close"])

    balance        = initial_balance
    peak_balance   = initial_balance
    max_drawdown   = 0.0
    equity_curve   = [balance]
    trades: list[Trade] = []
    active: Optional[Trade] = None
    halted = False

    for i in range(1, len(df)):
        candle = df.iloc[i]

        # Weekly drawdown circuit breaker
        drawdown_pct = (peak_balance - balance) / peak_balance * 100
        if drawdown_pct >= MAX_DRAWDOWN_HALT:
            if not halted:
                halted = True
            # Allow existing trade to close but no new entries

        # ── Manage active trade ───────────────────────────────────────────
        if active is not None:
            update_trailing_stop(active, candle["close"], icfg)
            reason = check_exit(active, candle)
            if reason:
                pnl     = close_trade(active, candle, reason)
                balance += pnl
                trades.append(active)
                active  = None

                if balance > peak_balance:
                    peak_balance = balance
                dd = (peak_balance - balance) / peak_balance * 100
                max_drawdown = max(max_drawdown, dd)
                # Reset halt if recovered
                if dd < MAX_DRAWDOWN_HALT:
                    halted = False

            equity_curve.append(balance)
            continue

        # ── Session filter ────────────────────────────────────────────────
        if halted or not in_session(candle["dt"]):
            equity_curve.append(balance)
            continue

        # ── Ensemble vote ─────────────────────────────────────────────────
        signal = ensemble_vote(df, i, icfg)
        if signal:
            active = open_trade(signal, candle, balance, icfg)

        equity_curve.append(balance)

    # Close any open trade at last price
    if active is not None:
        last = df.iloc[-1]
        pnl_pct = (
            (last["close"] - active.entry_price) / active.entry_price * 100
            if active.direction == "Buy"
            else (active.entry_price - last["close"]) / active.entry_price * 100
        )
        pnl     = active.size * (pnl_pct / 100) * active.entry_price
        active.exit_price  = last["close"]
        active.exit_time   = last["dt"]
        active.exit_reason = "EOD"
        active.pnl         = pnl
        balance += pnl
        trades.append(active)

    # ── Statistics ────────────────────────────────────────────────────────
    total  = len(trades)
    wins   = [t for t in trades if (t.pnl or 0) > 0]
    losses = [t for t in trades if (t.pnl or 0) <= 0]
    pnls   = [t.pnl or 0 for t in trades]

    win_rate     = len(wins) / total * 100 if total else 0
    total_pnl    = balance - initial_balance
    total_pnl_pct = total_pnl / initial_balance * 100
    total_gain   = sum(t.pnl for t in wins)
    total_loss   = abs(sum(t.pnl for t in losses))
    profit_factor = total_gain / total_loss if total_loss > 0 else (10.0 if total_gain > 0 else 0.0)

    sharpe = 0.0
    if total > 1:
        rets   = [p / initial_balance for p in pnls]
        mean   = sum(rets) / len(rets)
        var    = sum((r - mean) ** 2 for r in rets) / (len(rets) - 1)
        std    = math.sqrt(var)
        sharpe = (mean / std) * math.sqrt(total) if std > 0 else 0.0

    return {
        "instrument":    name,
        "period":        f"{df['dt'].iloc[0].date()} → {df['dt'].iloc[-1].date()}",
        "candles":       len(df),
        "total_trades":  total,
        "win_trades":    len(wins),
        "loss_trades":   len(losses),
        "win_rate":      win_rate,
        "total_pnl":     total_pnl,
        "total_pnl_pct": total_pnl_pct,
        "max_drawdown":  max_drawdown,
        "profit_factor": profit_factor,
        "sharpe_ratio":  sharpe,
        "final_balance": balance,
        "trades":        trades,
    }

# ── Reporting ─────────────────────────────────────────────────────────────────

def print_results(r: dict) -> None:
    pnl_sign   = "+" if r["total_pnl"]     >= 0 else ""
    pnl_color  = "\033[92m" if r["total_pnl"] >= 0 else "\033[91m"
    reset      = "\033[0m"

    icfg = INSTRUMENT_CONFIG.get(r["instrument"], {})
    sl  = icfg.get("sl_pct", "?")
    tp  = icfg.get("tp_pct", "?")
    strat_label = f"All 4 + ADX filter  SL {sl}% / TP {tp}%"
    print(f"\n{'─'*62}")
    print(f"  {r['instrument']}  |  {r['period']}  |  {strat_label}")
    print(f"{'─'*62}")
    print(f"  Candles examined   : {r['candles']:,}")
    print(f"  Total trades       : {r['total_trades']}")
    print(f"  Win / Loss         : {r['win_trades']} / {r['loss_trades']}")
    print(f"  Win rate           : {r['win_rate']:.1f}%")
    print(f"  Net P&L            : {pnl_color}{pnl_sign}${r['total_pnl']:,.2f}  ({pnl_sign}{r['total_pnl_pct']:.2f}%){reset}")
    print(f"  Final balance      : ${r['final_balance']:,.2f}")
    print(f"  Max drawdown       : {r['max_drawdown']:.2f}%")
    print(f"  Profit factor      : {r['profit_factor']:.2f}")
    print(f"  Sharpe ratio       : {r['sharpe_ratio']:.2f}")

    if r["trades"]:
        print(f"\n  Last 5 trades:")
        print(f"  {'Entry':19} {'Dir':5} {'Strategy':25} {'P&L':>10} {'Exit':6}")
        print(f"  {'-'*70}")
        for t in r["trades"][-5:]:
            entry_str = t.entry_time.strftime("%Y-%m-%d %H:%M")
            pnl_str   = f"+${t.pnl:,.2f}" if (t.pnl or 0) >= 0 else f"-${abs(t.pnl):,.2f}"
            color     = "\033[92m" if (t.pnl or 0) >= 0 else "\033[91m"
            print(f"  {entry_str:19} {t.direction:5} {t.strategy:25} {color}{pnl_str:>10}{reset} {t.exit_reason}")

# ── Entry point ───────────────────────────────────────────────────────────────

def main():
    parser = argparse.ArgumentParser(description="IG Trading Engine — Python Backtester")
    parser.add_argument("--epic",    default=None, help="EURUSD | USDJPY | GOLD (default: all)")
    parser.add_argument("--balance", type=float, default=INITIAL_BALANCE, help="Starting balance")
    args = parser.parse_args()

    targets = {args.epic: INSTRUMENTS[args.epic]} if args.epic and args.epic in INSTRUMENTS else INSTRUMENTS

    print("\n🔬 IG Trading Engine — Python Backtester (Per-Instrument Strategy Sets)")
    print(f"   GOLD   : All 4 + ADX filter, Gold-tuned params  (SL 1.0% / TP 5.0%)")
    print(f"   FX     : All 4 + ADX filter, FX-tuned params   (SL 1.5-2.0% / TP 6.0%)")
    print(f"   Ensemble: min {MIN_STRATEGIES} agree, avg strength ≥ {MIN_AVG_STRENGTH}")
    print(f"   Session : {SESSION_START_UTC:02d}:00–{SESSION_END_UTC:02d}:00 UTC")
    print(f"   Balance : ${args.balance:,.2f}")

    all_results = []

    for name, filename in targets.items():
        path = os.path.join(DATA_DIR, filename)
        if not os.path.exists(path):
            print(f"\n⚠️  {path} not found — run fetch_historical_data.py first")
            continue

        with open(path) as f:
            candles = json.load(f)

        result = run_backtest(name, candles, args.balance)
        print_results(result)
        all_results.append(result)

    # ── Portfolio summary ─────────────────────────────────────────────────
    if len(all_results) > 1:
        total_trades = sum(r["total_trades"] for r in all_results)
        total_pnl    = sum(r["total_pnl"]    for r in all_results)
        avg_wr       = sum(r["win_rate"]      for r in all_results) / len(all_results)
        avg_pf       = sum(r["profit_factor"] for r in all_results) / len(all_results)
        avg_dd       = sum(r["max_drawdown"]  for r in all_results) / len(all_results)

        print(f"\n{'═'*58}")
        print(f"  PORTFOLIO SUMMARY  ({len(all_results)} instruments)")
        print(f"{'═'*58}")
        print(f"  Total trades    : {total_trades}")
        print(f"  Avg win rate    : {avg_wr:.1f}%")
        print(f"  Total net P&L   : {'+'if total_pnl>=0 else ''}${total_pnl:,.2f}")
        print(f"  Avg profit fac  : {avg_pf:.2f}")
        print(f"  Avg max DD      : {avg_dd:.2f}%")
        print()

if __name__ == "__main__":
    main()
