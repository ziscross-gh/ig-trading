#!/bin/bash
# exposure_watchdog.sh — autonomous correlated-exposure / pyramiding Telegram alert.
#
# WHY THIS EXISTS
#   The engine has no correlated-position or per-instrument concentration cap. It has
#   repeatedly stacked 3-4 same-instrument legs (2026-06-24 USDJPY -716, 2026-06-25 GOLD
#   -736) and, on 2026-06-27/07-01, 5-6 legs across TWO instruments all leaning the same
#   USD direction at once. Catching this requires a human watching positions live — but
#   the Claude-side monitoring loop (ScheduleWakeup / CronCreate) has repeatedly stalled
#   for hours to days, independent of anything wrong with the engine or account. This
#   script closes that gap the same way feed_watchdog.sh closed the feed-death gap: it
#   runs on the machine itself, independent of any Claude session, and pings Telegram
#   directly when concentration crosses a threshold.
#
# WHAT IT DOES NOT DO
#   It never touches the engine, positions, or any strategy/risk/gate parameter. Pure
#   read + alert. Freeze-exempt (zero P&L/risk-param impact).
#
# TRIGGERS
#   A) same-instrument pyramid: any single epic with >= PYRAMID_LEGS same-direction
#      open positions.
#   B) correlated USD-direction exposure: across FX majors (USDJPY/EURUSD/GBPUSD/AUDUSD),
#      net USD-long-or-short leg count >= USD_NET_LEGS (buy USDJPY / sell EURUSD,GBPUSD,
#      AUDUSD all count as USD-long; the inverse as USD-short).
#
# ANTI-SPAM
#   Per-trigger cooldown (COOLDOWN_MIN). An escalation (leg count growing past the last
#   alerted count) always re-alerts immediately regardless of cooldown.

set -uo pipefail

API="http://localhost:9090/api/positions"
ENV_FILE="/Users/ziscross/.openclaw/workspace/projects/ig-trading/.env"
WLOG="/tmp/ig-exposure-watchdog.log"
STATE="/tmp/ig-exposure-watchdog.state.json"   # {trigger_key: {epoch, legs}}

PYRAMID_LEGS=3     # same-instrument same-direction legs to alert on
USD_NET_LEGS=4     # abs(net USD-direction legs) across FX majors to alert on
COOLDOWN_MIN=30

ts()  { date -u "+%Y-%m-%dT%H:%M:%SZ"; }
wlog(){ echo "$(ts) $*" >> "$WLOG"; }

# --- market-hours gate (positions are frozen when FX is closed; mirrors feed_watchdog.sh) ---
wd=$(date -u +%u)
h=$(date -u +%H); h=$((10#$h))
mkt_open=1
if   [ "$wd" -eq 6 ]; then mkt_open=0
elif [ "$wd" -eq 5 ] && [ "$h" -ge 21 ]; then mkt_open=0
elif [ "$wd" -eq 7 ] && [ "$h" -lt 21 ]; then mkt_open=0
fi
[ "$mkt_open" -eq 0 ] && exit 0

# --- load Telegram creds from .env without polluting the shell / logging values ---
# strip inline "# comment", surrounding quotes, and leading/trailing whitespace
env_val() {
    grep -E "^$1=" "$ENV_FILE" 2>/dev/null | head -1 | cut -d= -f2- \
        | sed -E 's/[[:space:]]*#.*//; s/^[[:space:]]*"?//; s/"?[[:space:]]*$//; s/^[[:space:]]*'"'"'?//; s/'"'"'?[[:space:]]*$//'
}
BOT_TOKEN=$(env_val TELEGRAM_BOT_TOKEN)
CHAT_ID=$(env_val TELEGRAM_CHAT_ID)
if [ -z "$BOT_TOKEN" ] || [ -z "$CHAT_ID" ]; then
    wlog "ERROR missing TELEGRAM_BOT_TOKEN/CHAT_ID in $ENV_FILE — cannot alert"
    exit 0
fi

positions_json=$(curl -s --max-time 5 "$API" 2>/dev/null)
[ -z "$positions_json" ] && { wlog "WARN empty response from $API — engine likely down"; exit 0; }

# --- evaluate triggers in python, emit one line per firing trigger: key|legs|message ---
findings=$(POS="$positions_json" python3 - <<'PY'
import json, os
from collections import defaultdict

PYRAMID_LEGS = 3
USD_NET_LEGS = 4
# sign=+1 means "buy == USD-long" (USD is the quote currency, e.g. USDJPY);
# sign=-1 means "buy == USD-short" (USD is the base currency, e.g. EURUSD)
FX_USD_SIGN = {
    "USDJPY": +1,
    "EURUSD": -1,
    "GBPUSD": -1,
    "AUDUSD": -1,
}

d = json.loads(os.environ["POS"])
ps = d if isinstance(d, list) else d.get("positions", d.get("data", []))

per_epic = defaultdict(lambda: defaultdict(list))  # epic -> direction -> [pnl,...]
for p in ps:
    inst = p["epic"].split(".")[2]
    per_epic[inst][p["direction"]].append(p.get("unrealised_pnl", 0.0))

out = []

# Trigger A: same-instrument pyramid
for inst, dirs in per_epic.items():
    for direction, pnls in dirs.items():
        if len(pnls) >= PYRAMID_LEGS:
            total = sum(pnls)
            out.append(f"PYRAMID:{inst}:{direction}|{len(pnls)}|"
                       f"{inst} {direction} x{len(pnls)} stacked (same-instrument pyramid), "
                       f"unrealised {total:+.2f}")

# Trigger B: correlated USD-direction exposure across FX majors
net_usd = 0
fx_legs = 0
detail = []
for inst, sign in FX_USD_SIGN.items():
    dirs = per_epic.get(inst, {})
    buy_n = len(dirs.get("buy", []))
    sell_n = len(dirs.get("sell", []))
    fx_legs += buy_n + sell_n
    net_usd += sign * buy_n - sign * sell_n
    if buy_n or sell_n:
        detail.append(f"{inst} buy x{buy_n}/sell x{sell_n}")
if abs(net_usd) >= USD_NET_LEGS:
    direction = "USD-long" if net_usd > 0 else "USD-short"
    out.append(f"USDNET:{direction}|{fx_legs}|"
               f"correlated {direction} exposure across FX majors: net {abs(net_usd)} legs "
               f"({', '.join(detail)})")

for line in out:
    print(line)
PY
)

[ -z "$findings" ] && exit 0

now_epoch=$(date -u +%s)

alerts_to_send=""
fire_count=0
while IFS='|' read -r key legs msg; do
    [ -z "$key" ] && continue
    decision=$(python3 - "$key" "$legs" "$now_epoch" "$COOLDOWN_MIN" "$STATE" <<'PY'
import json, sys
key, legs, now_epoch, cooldown_min, state_path = sys.argv[1], int(sys.argv[2]), int(sys.argv[3]), int(sys.argv[4]), sys.argv[5]
try:
    state = json.load(open(state_path))
except Exception:
    state = {}
prev = state.get(key)
fire = False
if prev is None:
    fire = True
elif legs > prev.get("legs", 0):
    fire = True  # escalation: always re-alert
elif (now_epoch - prev.get("epoch", 0)) >= cooldown_min * 60:
    fire = True
if fire:
    state[key] = {"epoch": now_epoch, "legs": legs}
    json.dump(state, open(state_path, "w"))
print("FIRE" if fire else "SKIP")
PY
)
    if [ "$decision" = "FIRE" ]; then
        alerts_to_send="${alerts_to_send}${msg}"$'\n'
        fire_count=$((fire_count + 1))
        wlog "ALERT $key legs=$legs — $msg"
    else
        wlog "suppressed (cooldown) $key legs=$legs"
    fi
done <<EOF
$findings
EOF

[ -z "$alerts_to_send" ] && exit 0

text="⚠️ Exposure watchdog — concentration flagged (observe only, freeze in effect, no auto-action):
${alerts_to_send}
Top 2026-07-03 review item: no correlated-position cap exists yet."

http_code=$(curl -s --max-time 10 -X POST "https://api.telegram.org/bot${BOT_TOKEN}/sendMessage" \
    --data-urlencode "chat_id=${CHAT_ID}" \
    --data-urlencode "text=${text}" \
    -o /tmp/ig-exposure-watchdog.lastresp.json \
    -w '%{http_code}')
curl_exit=$?
if [ "$curl_exit" -eq 0 ] && [ "$http_code" = "200" ]; then
    wlog "sent Telegram alert ($fire_count trigger(s), HTTP $http_code)"
else
    wlog "ERROR Telegram send FAILED (curl_exit=$curl_exit http=$http_code) — see /tmp/ig-exposure-watchdog.lastresp.json"
fi
exit 0
