#!/usr/bin/env bash
# engine_status.sh — one-call digest of the live ig-engine for humans and AI agents.
#
# Replaces the ad-hoc grep chains agents previously ran every monitoring cycle
# (process check + fills + closes + P&L sums + error scan = 4-6 tool calls).
# One invocation, ~25 lines of output, deterministic format.
#
# Usage: scripts/engine_status.sh [YYYY-MM-DD]   # default: today (UTC)
set -u
LOG=${IG_ENGINE_LOG:-/tmp/ig-engine-launchd.log}
D=${1:-$(date -u +%Y-%m-%d)}

echo "== ig-engine @ $(date -u '+%a %Y-%m-%d %H:%M UTC') — log day $D =="

PID=$(pgrep -f 'target/release/ig-engine' | head -1 || true)
if [ -z "${PID}" ]; then
  echo "ENGINE: DEAD — no process. Restart: launchctl unload && load ~/Library/LaunchAgents/com.igengine.plist"
else
  STATUS_JSON=$(curl -s --max-time 3 localhost:9090/api/status || true)
  if [ -n "$STATUS_JSON" ]; then
    echo "$STATUS_JSON" | PID="$PID" python3 -c '
import sys, json, os
s = json.load(sys.stdin)
a, d, cb = s["account"], s["daily_stats"], s["circuit_breaker"]
pid = os.environ["PID"]
print("ENGINE: alive pid %s | %s %s up %.1fh" % (pid, s["mode"], s["status"], s["uptime_secs"] / 3600))
print("ACCOUNT: bal %.2f | open positions %d | margin %.0f" % (a["balance"], s["open_positions"], a["margin_used"]))
print("ENGINE-DAY: %d trades %dW/%dL net %+.2f | CB: %d consec losses, paused=%s" % (
    d["trades_today"], d["winning"], d["losing"], d["net_pnl"], cb["consecutive_losses"], cb["is_paused"]))'
  else
    echo "ENGINE: process $PID alive but HTTP API (:9090) not responding"
  fi
fi

[ -r "$LOG" ] || { echo "LOG: $LOG not readable — no log digest"; exit 0; }

# ── Stale-data watchdog ─────────────────────────────────────────────────────
# 2026-06-24 incident: the Lightstreamer feed died at the Friday weekend close
# and never reconnected — the engine ran "alive" for ~4 days with zero market
# bars (no data → no signals → no trades) and nobody noticed. This flags a
# stalled feed within minutes instead of days. M15 bars close every 15 min, so
# >20 min with no bar during weekday market hours means the feed is dead.
LOG="$LOG" python3 <<'EOF'
import json, os, re, time
from datetime import datetime, timezone
path = os.environ["LOG"]
last_bar = None
# scan the tail for the most recent "Bar closed" timestamp
try:
    lines = open(path, errors="replace").read().splitlines()
except OSError:
    lines = []
for line in reversed(lines[-4000:]):
    if "Bar closed for" in line:
        try:
            last_bar = datetime.fromisoformat(json.loads(line)["timestamp"].replace("Z", "+00:00"))
        except Exception:
            continue
        break
now = datetime.now(timezone.utc)
wd, h = now.weekday(), now.hour  # Mon=0..Sun=6
mkt_open = not ((wd == 5) or (wd == 4 and h >= 21) or (wd == 6 and h < 21))
if last_bar is None:
    print("DATA: ⚠️  no 'Bar closed' found in recent log — feed status unknown")
else:
    age_min = (now - last_bar).total_seconds() / 60
    tag = "ok"
    if mkt_open and age_min > 20:
        tag = "⚠️  STALE — feed likely DEAD, restart to reconnect Lightstreamer"
    elif not mkt_open:
        tag = "(market closed — staleness expected)"
    print("DATA: last bar %s UTC (%.0f min ago) %s" % (last_bar.strftime("%m-%d %H:%M"), age_min, tag))
EOF


D="$D" LOG="$LOG" python3 <<'EOF'
import json, os, re
from collections import defaultdict

day, path = os.environ["D"], os.environ["LOG"]
fills, approvals, closes = 0, [], []
overrides, besnaps, gate_blocks, bypasses, rr_rejects = [], 0, 0, 0, 0
reconciled = []  # guaranteed-stop closes recovered by the accounting fix
err_decimal = err_panic = err_other = e403 = 0
consensus = defaultdict(int)

with open(path, errors="replace") as f:
    for line in f:
        if day not in line[:40]:
            continue
        try:
            rec = json.loads(line)
        except ValueError:
            continue
        ts, m = rec.get("timestamp", "")[11:16], rec.get("fields", {}).get("message", "")
        if "Trade execution confirmed" in m:
            fills += 1
        elif m.startswith("Trade approved:"):
            approvals.append(f"{ts} {m[15:90].strip()}")
        elif "OPU P&L recomputed" in m:
            mt = re.search(r"recomputed: (\S+) (\S+) entry=([\d.]+) exit=([\d.]+).*reason=(\w+).*pnl=([-\d.]+)", m)
            if mt:
                closes.append((ts, *mt.groups()))
        elif "reconciled" in m and "P&L" in m:
            mt = re.search(r"P&L ([-\d.]+)", m)
            if mt:
                reconciled.append((ts, float(mt.group(1))))
        elif "instrument SL/TP override" in m:
            overrides.append(f"{ts} {m[:110]}")
        elif "BE snap" in m:
            besnaps += 1
        elif "H1 direction gate" in m and "blocking" in m:
            gate_blocks += 1
        elif "H1-zero bypass" in m:
            bypasses += 1
        elif "risk/reward" in m.lower() and ("reject" in m.lower() or "below" in m.lower()):
            rr_rejects += 1
        elif "Bar analysis:" in m:
            mt = re.search(r"(\d)/(\d) fired", m)
            if mt:
                consensus[mt.group(1) + "/" + mt.group(2)] += 1
        if rec.get("level") == "ERROR":
            if "too-many-decimal" in m: err_decimal += 1
            elif "panic" in m.lower(): err_panic += 1
            elif "exceeded-account-historical-data-allowance" in m or "403" in m: e403 += 1
            else: err_other += 1

per = defaultdict(lambda: [0.0, 0, 0, 0])  # net, W, L, BE
for _, epic, _, _, _, _, pnl in closes:
    k = "GOLD" if "GOLD" in epic else ("EURUSD" if "EURUSD" in epic else "USDJPY" if "USDJPY" in epic else epic)
    p = float(pnl); per[k][0] += p
    per[k][1 if p > 0 else (3 if p == 0 else 2)] += 1

print(f"LOG-DAY: {fills} fills | {len(closes)} OPU closes net {sum(float(c[-1]) for c in closes):+.2f}")
for k, (net, w, l, be) in sorted(per.items()):
    print(f"  {k:7s} {net:+9.2f}  ({w}W/{l}L/{be}BE)")
if reconciled:
    rnet = sum(p for _, p in reconciled)
    print(f"  RECONCILED (guaranteed-stop closes recovered): {len(reconciled)}, net {rnet:+.2f} "
          f"(now counted in stats/scorecard/CB — included in ENGINE-DAY above)")
for c in closes[-5:]:
    print(f"  close {c[0]} {c[1].split('.')[2]:7s} {c[2]:4s} {c[3]}->{c[4]} {c[5]:12s} {float(c[6]):+9.2f}")
for a in approvals[-3:]:
    print(f"  appr  {a}")
print(f"M15: consensus {dict(consensus) or 'none'} | H1-gate blocks {gate_blocks} | bypasses {bypasses}")
print(f"17F: SL/TP overrides {len(overrides)} | BE snaps {besnaps} | RR rejects {rr_rejects}" + (f"\n  last override: {overrides[-1]}" if overrides else ""))
print(f"ERRORS: decimal {err_decimal} | panic {err_panic} | other {err_other} | 403-quota {e403} (expected)")
if err_decimal or err_panic or rr_rejects:
    print("⚠️  ESCALATE: decimal/panic/RR-reject errors present")
EOF
