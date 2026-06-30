#!/bin/bash
# feed_watchdog.sh — autonomous stale-feed auto-restart for the IG engine.
#
# WHY THIS EXISTS
#   2026-06-24 and again 2026-06-30: the Lightstreamer market-data feed died at the
#   Friday weekend close and never reconnected at the Sunday reopen. The engine kept
#   running "alive but blind" (process up, KeepAlive satisfied, ZERO bars/signals/trades)
#   for days. The engine has no in-process auto-reconnect, and the human/agent monitoring
#   loop that was supposed to catch it had stalled. This watchdog closes that gap.
#
# WHAT IT DOES
#   Runs every ~2 min on a launchd StartInterval, INDEPENDENT of any Claude/agent loop.
#   During FX market hours, if the newest "Bar closed" in the engine log is older than
#   STALE_MIN, it restarts the engine (launchctl unload/load -> KeepAlive forces a fresh
#   start) to reconnect Lightstreamer. Automates the proven manual recovery.
#
# SAFE BY DESIGN
#   - Does nothing when the FX market is closed (Fri 21:00 -> Sun 21:00 UTC).
#   - Honors a cooldown so it cannot flap (a fresh M15 bar takes up to 15 min after a
#     restart; cooldown > that prevents double-restarts).
#   - Touches NO strategy/risk/gate parameters — pure ops/reliability (freeze-exempt).
#   - Logs every decision to WLOG for audit.

set -uo pipefail

LOG="/tmp/ig-engine-launchd.log"                       # engine log to scan for "Bar closed"
WLOG="/tmp/ig-feed-watchdog.log"                       # this watchdog's own audit log
PLIST="$HOME/Library/LaunchAgents/com.igengine.plist"  # engine launchd job
STATE="/tmp/ig-feed-watchdog.state"                    # epoch of last restart action

STALE_MIN=22     # restart if newest bar older than this (min) during market hours
COOLDOWN_MIN=20  # min gap between restarts (> max 15 min to first post-restart M15 bar)

now_epoch=$(date -u +%s)
ts()  { date -u "+%Y-%m-%dT%H:%M:%SZ"; }
wlog(){ echo "$(ts) $*" >> "$WLOG"; }

# --- market-hours gate (mirror of engine_status.sh mkt_open) ---
# date +%u: 1=Mon .. 7=Sun  => Sat=6, Fri=5, Sun=7
wd=$(date -u +%u)
h=$(date -u +%H); h=$((10#$h))   # force base-10 (avoid "08"/"09" octal errors)
mkt_open=1
if   [ "$wd" -eq 6 ]; then mkt_open=0                                  # Saturday: closed all day
elif [ "$wd" -eq 5 ] && [ "$h" -ge 21 ]; then mkt_open=0              # Friday >= 21:00 UTC: closed
elif [ "$wd" -eq 7 ] && [ "$h" -lt 21 ]; then mkt_open=0              # Sunday < 21:00 UTC: closed
fi
[ "$mkt_open" -eq 0 ] && exit 0   # market closed: staleness expected, do nothing

# --- newest "Bar closed" age in minutes (-1 if none found) ---
age_min=$(LOG="$LOG" python3 - <<'PY'
import json, os
from datetime import datetime, timezone
path = os.environ["LOG"]
last = None
try:
    lines = open(path, errors="replace").read().splitlines()
except OSError:
    lines = []
for line in reversed(lines[-4000:]):
    if "Bar closed for" in line:
        try:
            last = datetime.fromisoformat(json.loads(line)["timestamp"].replace("Z", "+00:00"))
            break
        except Exception:
            continue
print(-1 if last is None else int((datetime.now(timezone.utc) - last).total_seconds() // 60))
PY
)

[ "$age_min" -lt 0 ] && { wlog "WARN market-open but no 'Bar closed' found — treating as stale"; age_min=9999; }

# healthy feed -> nothing to do
[ "$age_min" -le "$STALE_MIN" ] && exit 0

# STALE during market hours -> restart, honoring cooldown
last_restart=0
[ -f "$STATE" ] && last_restart=$(cat "$STATE" 2>/dev/null || echo 0)
since_min=$(( (now_epoch - last_restart) / 60 ))
if [ "$last_restart" -gt 0 ] && [ "$since_min" -lt "$COOLDOWN_MIN" ]; then
    wlog "STALE bar_age=${age_min}m but in cooldown (${since_min}m < ${COOLDOWN_MIN}m) — waiting for reconnect"
    exit 0
fi

wlog "STALE bar_age=${age_min}m during market hours — RESTARTING engine (launchctl unload/load)"
launchctl unload "$PLIST" 2>>"$WLOG"
sleep 2
launchctl load "$PLIST" 2>>"$WLOG"   # KeepAlive=true forces an immediate start
echo "$now_epoch" > "$STATE"
sleep 3
wlog "restart issued — engine pid now: $(pgrep -f 'release/ig-engine' | tr '\n' ' ')"
exit 0
