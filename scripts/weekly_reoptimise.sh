#!/usr/bin/env bash
# weekly_reoptimise.sh — Walk-forward auto re-optimisation pipeline
# ------------------------------------------------------------------
# Runs every Sunday midnight via cron. Fetches 6 months of fresh data,
# runs the grid optimizer, applies improvements to default.toml, and
# hot-reloads the live engine via SIGUSR1.
#
# Install (cron):
#   crontab -e
#   0 0 * * 0 /path/to/ig-trading/scripts/weekly_reoptimise.sh >> /path/to/logs/reoptimise.log 2>&1
#
# Required env vars (add to .env or crontab):
#   TELEGRAM_BOT_TOKEN  — Telegram bot token (optional, for notifications)
#   TELEGRAM_CHAT_ID    — Telegram chat ID (optional)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
PID_FILE="$PROJECT_DIR/ig-engine.pid"
LOG_DIR="$PROJECT_DIR/logs"
RESULTS_JSON="$PROJECT_DIR/data/optimize_results.json"

mkdir -p "$LOG_DIR"

log() {
    echo "[$(date -u '+%Y-%m-%dT%H:%M:%SZ')] $*"
}

# ── Load .env if present ──────────────────────────────────────────────────────
if [ -f "$PROJECT_DIR/.env" ]; then
    set -a
    # shellcheck disable=SC1091
    source "$PROJECT_DIR/.env"
    set +a
fi

log "════════════════════════════════════════"
log "Weekly Re-optimise — IG Trading Engine"
log "════════════════════════════════════════"

cd "$PROJECT_DIR"

# ── Step 1: Refresh 6 months of historical data ───────────────────────────────
log "Step 1/4 — Fetching 6-month OHLCV data..."
python scripts/fetch_historical_data.py --months 6
log "Data fetch complete."

# ── Step 2: Run grid search, write results JSON ───────────────────────────────
log "Step 2/4 — Running parameter optimizer..."
python scripts/optimize.py --output "$RESULTS_JSON"
log "Optimizer complete. Results → $RESULTS_JSON"

# ── Step 3: Compare vs current config, apply if improved ─────────────────────
log "Step 3/4 — Comparing optimizer results to current config..."
set +e
python scripts/compare_params.py
COMPARE_EXIT=$?
set -e

# Exit codes from compare_params.py:
#   0 = no changes needed
#   1 = changes applied to config
#   2 = error

if [ "$COMPARE_EXIT" -eq 2 ]; then
    log "ERROR: compare_params.py returned error — aborting reload."
    exit 1
fi

# ── Step 4: Hot-reload engine if config changed ───────────────────────────────
if [ "$COMPARE_EXIT" -eq 1 ]; then
    log "Step 4/4 — Config updated. Sending SIGUSR1 to engine for hot-reload..."

    if [ -f "$PID_FILE" ]; then
        ENGINE_PID=$(cat "$PID_FILE")
        if kill -0 "$ENGINE_PID" 2>/dev/null; then
            kill -USR1 "$ENGINE_PID"
            sleep 2   # give engine time to reload
            log "SIGUSR1 sent to PID $ENGINE_PID — config reloaded in-place."
        else
            log "WARNING: Engine PID $ENGINE_PID is not running. Config will load on next startup."
        fi
    else
        log "WARNING: $PID_FILE not found. Is the engine running? Config will load on next startup."
    fi
else
    log "Step 4/4 — No config changes. Engine reload not needed."
fi

log "════════════════════════════════════════"
log "Re-optimise complete."
log "════════════════════════════════════════"
