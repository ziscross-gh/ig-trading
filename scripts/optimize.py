#!/usr/bin/env python3
"""
optimize.py — Parameter Grid Search for IG Trading Engine
----------------------------------------------------------
Tests combinations of SL/TP, trailing stop, and ADX range filter
across all 3 instruments. Shows top results sorted by profit factor.

Usage:
    python scripts/optimize.py
    python scripts/optimize.py --epic EURUSD
    python scripts/optimize.py --top 20
"""

import argparse
import datetime
import json
import math
import os
import sys
import warnings
from dataclasses import dataclass
from itertools import product
from typing import Optional

warnings.filterwarnings("ignore")

try:
    import numpy as np
    import pandas as pd
except ImportError:
    raise SystemExit("Run: pip install numpy pandas")

DATA_DIR = os.path.join(os.path.dirname(__file__), "..", "data")

INSTRUMENTS = {
    "EURUSD": "EURUSD_1H.json",
    "USDJPY": "USDJPY_1H.json",
    "GOLD":   "GOLD_1H.json",
}

# ── Parameter grid ─────────────────────────────────────────────────────────────
#
# Key levers:
#   sl_pct           : stop loss distance (%)
#   tp_pct           : take profit distance (%)
#   adx_range_filter : only take RSI/Bollinger signals when ADX < adx_max
#                      (prevents mean-reversion in trending markets)
#   adx_max          : threshold for "ranging" regime
#   trail_dist_pct   : trailing stop distance once activated
#   trail_act_pct    : profit % before trailing activates

PARAM_GRID = {
    "sl_pct":           [1.0, 1.5, 2.0],
    "tp_pct":           [3.0, 4.0, 5.0, 6.0],
    "adx_range_filter": [False, True],
    "adx_max":          [20, 25],           # only used when adx_range_filter=True
    "trail_dist_pct":   [1.0, 1.5, 2.0],
    "trail_act_pct":    [0.3, 0.5],
}

# Fixed settings
SESSION_START_UTC  = 7
SESSION_END_UTC    = 20
MIN_STRATEGIES     = 2
MIN_AVG_STRENGTH   = 6.0
RISK_PER_TRADE_PCT = 1.0
INITIAL_BALANCE    = 10_000.0
MAX_DD_HALT        = 5.0

# ── Indicators ─────────────────────────────────────────────────────────────────

def sma(s, n):  return s.rolling(n).mean()
def ema(s, n):  return s.ewm(span=n, adjust=False).mean()

def rsi(s, n=14):
    d = s.diff()
    g = d.clip(lower=0).ewm(alpha=1/n, adjust=False).mean()
    l = (-d).clip(lower=0).ewm(alpha=1/n, adjust=False).mean()
    return 100 - 100 / (1 + g / l.replace(0, np.nan))

def macd_calc(s, f=12, sl=26, sig=9):
    ml = ema(s, f) - ema(s, sl)
    return ml, ema(ml, sig)

def bollinger(s, n=20, k=2.0):
    m = sma(s, n); sd = s.rolling(n).std()
    return m + k*sd, m, m - k*sd

def adx_calc(high, low, close, n=14):
    ph, pl, pc = high.shift(1), low.shift(1), close.shift(1)
    tr = pd.concat([high-low, (high-pc).abs(), (low-pc).abs()], axis=1).max(axis=1)
    dmp = pd.Series(np.where((high-ph)>(pl-low), np.maximum(high-ph,0), 0), index=close.index)
    dmm = pd.Series(np.where((pl-low)>(high-ph), np.maximum(pl-low,0), 0), index=close.index)
    atr = tr.ewm(alpha=1/n, adjust=False).mean()
    dip = 100 * dmp.ewm(alpha=1/n, adjust=False).mean() / atr.replace(0, np.nan)
    dim = 100 * dmm.ewm(alpha=1/n, adjust=False).mean() / atr.replace(0, np.nan)
    dx  = 100 * (dip-dim).abs() / (dip+dim).replace(0, np.nan)
    return dx.ewm(alpha=1/n, adjust=False).mean()

def build_indicators(df):
    df = df.copy()
    df["sma9"]     = sma(df["close"], 9)
    df["sma21"]    = sma(df["close"], 21)
    df["rsi"]      = rsi(df["close"])
    df["macd"], df["msig"] = macd_calc(df["close"])
    df["bbu"], _, df["bbl"] = bollinger(df["close"])
    df["adx"]      = adx_calc(df["high"], df["low"], df["close"])
    return df

# ── Strategies (config-aware) ──────────────────────────────────────────────────

def nan(*vals):
    return any(math.isnan(v) for v in vals)

def sig_ma_cross(df, i, cfg):
    if i < 1: return None
    f0,f1 = df["sma9"].iloc[i],  df["sma9"].iloc[i-1]
    s0,s1 = df["sma21"].iloc[i], df["sma21"].iloc[i-1]
    a     = df["adx"].iloc[i]
    if nan(f0,f1,s0,s1,a) or a <= 25: return None
    strength = min(9.5, 7.0 + (a - 25) / 30)
    if f1 <= s1 and f0 > s0: return ("Buy",  strength, "MACross")
    if f1 >= s1 and f0 < s0: return ("Sell", strength, "MACross")
    return None

def sig_rsi(df, i, cfg):
    if i < 1: return None
    r0, r1 = df["rsi"].iloc[i], df["rsi"].iloc[i-1]
    if nan(r0, r1): return None
    if cfg["adx_range_filter"] and df["adx"].iloc[i] > cfg["adx_max"]: return None
    if r1 <= 30 and r0 > 30: return ("Buy",  8.0, "RSI")
    if r1 >= 70 and r0 < 70: return ("Sell", 8.0, "RSI")
    return None

def sig_macd(df, i, cfg):
    if i < 1: return None
    m0,m1 = df["macd"].iloc[i], df["macd"].iloc[i-1]
    s0,s1 = df["msig"].iloc[i], df["msig"].iloc[i-1]
    if nan(m0,m1,s0,s1): return None
    if m1 <= s1 and m0 > s0 and m0 < 0: return ("Buy",  7.0, "MACD")
    if m1 >= s1 and m0 < s0 and m0 > 0: return ("Sell", 7.0, "MACD")
    return None

def sig_boll(df, i, cfg):
    if i < 1: return None
    c0,c1 = df["close"].iloc[i], df["close"].iloc[i-1]
    u, l  = df["bbu"].iloc[i], df["bbl"].iloc[i]
    if nan(c0,c1,u,l): return None
    if cfg["adx_range_filter"] and df["adx"].iloc[i] > cfg["adx_max"]: return None
    if c1 <= l and c0 > l: return ("Buy",  6.5, "Boll")
    if c1 >= u and c0 < u: return ("Sell", 6.5, "Boll")
    return None

STRATEGY_FNS = [sig_ma_cross, sig_rsi, sig_macd, sig_boll]

def ensemble(df, i, cfg):
    sigs = [fn(df, i, cfg) for fn in STRATEGY_FNS]
    sigs = [s for s in sigs if s]
    for direction in ("Buy", "Sell"):
        m = [s for s in sigs if s[0] == direction]
        if len(m) >= MIN_STRATEGIES:
            avg = sum(s[1] for s in m) / len(m)
            if avg >= MIN_AVG_STRENGTH:
                return direction, avg, "+".join(s[2] for s in m)
    return None

# ── Simulation ─────────────────────────────────────────────────────────────────

def simulate(df, cfg):
    balance = INITIAL_BALANCE
    peak    = INITIAL_BALANCE
    max_dd  = 0.0
    trades  = 0
    wins    = 0
    gains   = 0.0
    losses  = 0.0
    halted  = False

    active  = None   # (direction, entry, sl, tp, size, trail_on)

    sl  = cfg["sl_pct"]
    tp  = cfg["tp_pct"]
    trd = cfg["trail_dist_pct"]
    tra = cfg["trail_act_pct"]

    for i in range(1, len(df)):
        row  = df.iloc[i]
        hour = row["dt"].hour

        # ── Manage active trade ────────────────────────────────────────
        if active:
            direction, entry, cur_sl, cur_tp, size, trail_on = active

            # Update trailing stop
            pnl_pct = ((row["close"]-entry)/entry*100 if direction=="Buy"
                       else (entry-row["close"])/entry*100)
            if pnl_pct >= tra:
                trail_on = True
            if trail_on:
                if direction == "Buy":
                    new_sl = row["close"] * (1 - trd/100)
                    cur_sl = max(cur_sl, new_sl)
                else:
                    new_sl = row["close"] * (1 + trd/100)
                    cur_sl = min(cur_sl, new_sl)

            # Check exit
            exit_price = None
            if direction == "Buy":
                if row["low"]  <= cur_sl: exit_price = cur_sl
                if row["high"] >= cur_tp: exit_price = cur_tp
            else:
                if row["high"] >= cur_sl: exit_price = cur_sl
                if row["low"]  <= cur_tp: exit_price = cur_tp

            if exit_price:
                pnl_p = ((exit_price-entry)/entry*100 if direction=="Buy"
                         else (entry-exit_price)/entry*100)
                pnl   = size * (pnl_p/100) * entry
                balance += pnl
                trades  += 1
                if pnl > 0: wins += 1; gains += pnl
                else:                  losses += abs(pnl)

                if balance > peak: peak = balance
                dd = (peak-balance)/peak*100
                if dd > max_dd: max_dd = dd
                halted = dd >= MAX_DD_HALT
                active = None
            else:
                active = (direction, entry, cur_sl, cur_tp, size, trail_on)
            continue

        # ── New entry ──────────────────────────────────────────────────
        if halted or not (SESSION_START_UTC <= hour < SESSION_END_UTC):
            if halted and (peak-balance)/peak*100 < MAX_DD_HALT:
                halted = False
            continue

        result = ensemble(df, i, cfg)
        if result:
            direction, _, _ = result
            price  = row["close"]
            risk   = balance * (RISK_PER_TRADE_PCT/100)
            sl_d   = price * (sl/100)
            size   = max(1.0, round(risk/sl_d, 2))
            cur_sl = price*(1-sl/100) if direction=="Buy" else price*(1+sl/100)
            cur_tp = price*(1+tp/100) if direction=="Buy" else price*(1-tp/100)
            active = (direction, price, cur_sl, cur_tp, size, False)

    # Close open trade at last bar
    if active:
        direction, entry, _, _, size, _ = active
        last = df.iloc[-1]
        pp   = ((last["close"]-entry)/entry*100 if direction=="Buy"
                else (entry-last["close"])/entry*100)
        pnl  = size*(pp/100)*entry
        balance += pnl
        trades  += 1
        if pnl > 0: wins += 1; gains += pnl
        else:                  losses += abs(pnl)
        if balance > peak: peak = balance
        dd = (peak-balance)/peak*100
        if dd > max_dd: max_dd = dd

    win_rate = wins/trades*100 if trades else 0
    pf       = gains/losses if losses > 0 else (10.0 if gains > 0 else 0.0)
    net_pct  = (balance-INITIAL_BALANCE)/INITIAL_BALANCE*100

    return {
        "trades":     trades,
        "win_rate":   win_rate,
        "net_pct":    net_pct,
        "max_dd":     max_dd,
        "profit_fac": pf,
        "balance":    balance,
    }

# ── Grid search ────────────────────────────────────────────────────────────────

def run_grid(name, df):
    results = []

    sl_vals   = PARAM_GRID["sl_pct"]
    tp_vals   = PARAM_GRID["tp_pct"]
    arf_vals  = PARAM_GRID["adx_range_filter"]
    adx_vals  = PARAM_GRID["adx_max"]
    trd_vals  = PARAM_GRID["trail_dist_pct"]
    tra_vals  = PARAM_GRID["trail_act_pct"]

    combos = list(product(sl_vals, tp_vals, arf_vals, adx_vals, trd_vals, tra_vals))
    # Remove adx_max combos where adx_range_filter=False (irrelevant)
    combos = [c for c in combos if c[2] or c[3] == adx_vals[0]]
    total  = len(combos)

    print(f"\n  Running {total} combinations on {name}...", end="", flush=True)

    for n, (sl, tp, arf, adx_max, trd, tra) in enumerate(combos):
        cfg = {
            "sl_pct":           sl,
            "tp_pct":           tp,
            "adx_range_filter": arf,
            "adx_max":          adx_max,
            "trail_dist_pct":   trd,
            "trail_act_pct":    tra,
        }
        r = simulate(df, cfg)
        if r["trades"] >= 10:   # ignore runs with too few trades
            results.append({**cfg, **r, "instrument": name})

        if (n+1) % 20 == 0:
            print(".", end="", flush=True)

    print(f" done ({len(results)} valid runs)")
    return results

# ── Entry point ────────────────────────────────────────────────────────────────

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--epic",   default=None,  help="Run only this instrument (EURUSD, USDJPY, GOLD)")
    parser.add_argument("--top",    type=int, default=15, help="Number of top results to display")
    parser.add_argument("--output", default=None,  help="Write top-20 results per instrument as JSON to this path")
    args = parser.parse_args()

    targets = ({args.epic: INSTRUMENTS[args.epic]}
               if args.epic and args.epic in INSTRUMENTS else INSTRUMENTS)

    print("\n🔍 IG Trading Engine — Parameter Optimizer")
    print(f"   Grid: SL×TP×ADRfilter×Trail = {len(list(product(*PARAM_GRID.values())))} raw combos")
    print(f"   Min trades for valid run: 10")

    all_results = []

    for name, filename in targets.items():
        path = os.path.join(DATA_DIR, filename)
        if not os.path.exists(path):
            print(f"\n⚠️  {path} not found — run fetch_historical_data.py first")
            continue
        with open(path) as f:
            raw = json.load(f)

        df = pd.DataFrame(raw)
        df["dt"] = pd.to_datetime(df["timestamp"], unit="s", utc=True)
        df = build_indicators(df)

        results = run_grid(name, df)
        all_results.extend(results)

    if not all_results:
        print("No results — check data files.")
        return

    # ── Per-instrument top results ─────────────────────────────────────────
    for name in targets:
        inst = [r for r in all_results if r["instrument"] == name]
        if not inst:
            continue
        inst.sort(key=lambda r: r["profit_fac"], reverse=True)
        top = inst[:args.top]

        print(f"\n{'═'*78}")
        print(f"  TOP {args.top} — {name}  (sorted by Profit Factor)")
        print(f"{'═'*78}")
        print(f"  {'SL':>4} {'TP':>4} {'ADRf':>5} {'ADXm':>5} {'Trd':>4} {'Tra':>4} "
              f"{'#Tr':>4} {'WR%':>6} {'Net%':>7} {'MaxDD':>6} {'PF':>5}")
        print(f"  {'-'*74}")
        for r in top:
            arf  = "Y" if r["adx_range_filter"] else "N"
            adxm = str(r["adx_max"]) if r["adx_range_filter"] else " — "
            net_c = "\033[92m" if r["net_pct"] >= 0 else "\033[91m"
            rst   = "\033[0m"
            print(f"  {r['sl_pct']:>4.1f} {r['tp_pct']:>4.1f} {arf:>5} {adxm:>5} "
                  f"{r['trail_dist_pct']:>4.1f} {r['trail_act_pct']:>4.1f} "
                  f"{r['trades']:>4} {r['win_rate']:>6.1f} "
                  f"{net_c}{r['net_pct']:>+7.2f}{rst} {r['max_dd']:>6.2f} {r['profit_fac']:>5.2f}")

    # ── Overall best across all instruments ───────────────────────────────
    if len(targets) > 1:
        # Group by config, sum net_pct
        config_keys = ["sl_pct", "tp_pct", "adx_range_filter", "adx_max",
                       "trail_dist_pct", "trail_act_pct"]
        combined: dict[tuple, dict] = {}
        for r in all_results:
            key = tuple(r[k] for k in config_keys)
            if key not in combined:
                combined[key] = {"cfg": {k: r[k] for k in config_keys},
                                 "net_pct": 0.0, "pf_sum": 0.0, "count": 0,
                                 "trades": 0, "wr_sum": 0.0, "dd_max": 0.0}
            combined[key]["net_pct"]  += r["net_pct"]
            combined[key]["pf_sum"]   += r["profit_fac"]
            combined[key]["count"]    += 1
            combined[key]["trades"]   += r["trades"]
            combined[key]["wr_sum"]   += r["win_rate"]
            combined[key]["dd_max"]    = max(combined[key]["dd_max"], r["max_dd"])

        rows = []
        for key, v in combined.items():
            if v["count"] == len(targets):  # only configs present in ALL instruments
                rows.append({
                    **v["cfg"],
                    "net_pct":    v["net_pct"],
                    "avg_pf":     v["pf_sum"] / v["count"],
                    "avg_wr":     v["wr_sum"] / v["count"],
                    "max_dd":     v["dd_max"],
                    "total_tr":   v["trades"],
                })
        rows.sort(key=lambda r: r["avg_pf"], reverse=True)

        print(f"\n{'═'*78}")
        print(f"  BEST CONFIGS ACROSS ALL INSTRUMENTS  (sorted by avg Profit Factor)")
        print(f"{'═'*78}")
        print(f"  {'SL':>4} {'TP':>4} {'ADRf':>5} {'ADXm':>5} {'Trd':>4} {'Tra':>4} "
              f"{'#Tr':>5} {'WR%':>6} {'NetSum%':>8} {'MaxDD':>6} {'AvgPF':>6}")
        print(f"  {'-'*74}")
        for r in rows[:args.top]:
            arf  = "Y" if r["adx_range_filter"] else "N"
            adxm = str(r["adx_max"]) if r["adx_range_filter"] else " — "
            nc   = "\033[92m" if r["net_pct"] >= 0 else "\033[91m"
            rst  = "\033[0m"
            print(f"  {r['sl_pct']:>4.1f} {r['tp_pct']:>4.1f} {arf:>5} {adxm:>5} "
                  f"{r['trail_dist_pct']:>4.1f} {r['trail_act_pct']:>4.1f} "
                  f"{r['total_tr']:>5} {r['avg_wr']:>6.1f} "
                  f"{nc}{r['net_pct']:>+8.2f}{rst} {r['max_dd']:>6.2f} {r['avg_pf']:>6.2f}")

    # ── JSON output for compare_params.py / walk-forward pipeline ─────────
    if args.output:
        output_data = {
            "generated_at": datetime.datetime.utcnow().strftime("%Y-%m-%dT%H:%M:%SZ"),
            "results_per_instrument": {},
        }
        for name in targets:
            inst = [r for r in all_results if r["instrument"] == name]
            inst.sort(key=lambda r: r["profit_fac"], reverse=True)
            # Serialise booleans properly (numpy bool → python bool)
            clean = []
            for r in inst[:20]:
                clean.append({k: (bool(v) if isinstance(v, (bool,)) else v)
                               for k, v in r.items()})
            output_data["results_per_instrument"][name] = clean

        out_dir = os.path.dirname(args.output)
        if out_dir:
            os.makedirs(out_dir, exist_ok=True)
        with open(args.output, "w") as f:
            json.dump(output_data, f, indent=2)
        print(f"\n📄 Results written → {args.output}")

    print()

if __name__ == "__main__":
    main()
