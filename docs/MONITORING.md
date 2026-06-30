# MONITORING.md — standing instructions for the engine-monitoring loop

Agent-facing runbook. A recurring monitoring prompt should just say
*"follow docs/MONITORING.md"* plus a short context delta (open positions,
active experiments, pending decisions) — everything stable lives here.

## Each cycle

1. `date -u "+%u %H%M"` → pick the state below.
2. `scripts/engine_status.sh` (and with yesterday's date after midnight UTC if
   positions were open at the rollover). Drill into `/tmp/ig-engine-launchd.log`
   only when the digest shows something that needs it.
3. Report tersely unless something in **Surface** applies. Never repeat old
   summaries; report deltas.

## States and cadence (engine takes new entries 07:00–20:00 UTC; closes happen anytime)

| When (UTC) | State | Wakeup |
|---|---|---|
| Weekday 07:00–20:59 | trading window | ~900 s with open positions or a pending verification; ~1800 s flat/quiet |
| Weekday 21:00–06:59 | overnight | ~3600 s, terse — riders can still close |
| Fri 21:00 → Sun 21:00 | market closed | ~3600 s, terse |
| ~20:00 weekdays | day summary | per-instrument booked P&L + open positions + pending-decision reminders |

## Surface prominently (otherwise: open count + day fills/net + alive)

- Closes: entry→exit, reason, pnl. Classify: trailed/TP win · BE scratch (0.00) ·
  whipsaw stop-out (loss within ~spread-noise distance of entry).
- Engine DEAD / panic / `⚠️ ESCALATE` line in the digest.
- **`DATA: ⚠️ STALE` line in the digest** — the Lightstreamer feed has gone
  silent during market hours (no bar in >20 min). This is the 2026-06-24 /
  2026-06-30 failure mode: the engine runs "alive" but blind (no data → no
  signals → no trades), and it can persist for DAYS. **An autonomous OS-level
  watchdog (`com.igengine.feedwatchdog`, every 2 min) now auto-restarts the
  engine on this within ~22 min** — it should self-heal without you (audit log
  `/tmp/ig-feed-watchdog.log`). If you still see `STALE` persist >25 min, the
  watchdog itself failed: check its log, then manually restart as fallback
  (`launchctl unload && load ~/Library/LaunchAgents/com.igengine.plist`) and
  confirm the next digest shows a fresh bar (`DATA: ... ok`). Report it. Manual
  restart on a confirmed stale feed during market hours remains the one
  exception to "don't restart without approval."
- `too-many-decimal` errors — Fix #6 regression, escalate immediately.
- Risk-gate rejections mentioning risk/reward — per-instrument SL/TP override
  math is off, escalate.
- Entries stacked < 45 min apart on one instrument (evidence for the pending
  entry-spacing suggestion) — flag, don't fix.
- Signals repeatedly blocked (H1 gate, trading hours) — count them; they feed
  the standing investigation queue.

## Known noise — do NOT escalate

- 403 `exceeded-account-historical-data-allowance`: weekly REST quota; 17.D
  backoff + tick accumulator handle it.
- Telegram send errors.
- Digest error counters are cumulative for the log day — compare against the
  count already reported, only new ones matter.
- No `Bar analysis` lines for < 18 min is normal M15 cadence, not a stall.
- Daily-trade counter resets on engine restart (restart artifact).

## Hard rules

- NEVER implement strategy/risk/gate changes from monitoring observations —
  propose with evidence and wait for explicit user approval. Track proposals in
  the TASK_TRACKER.md header/pending section.
- Restart (`launchctl unload && load ~/Library/LaunchAgents/com.igengine.plist`)
  only on engine death, a confirmed `DATA: ⚠️ STALE` feed outage, or
  user-approved deploys; say so when you do.
- Reschedule each cycle with the same short prompt (runbook pointer + updated
  context delta) unless the user says stop.
