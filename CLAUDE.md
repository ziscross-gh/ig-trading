# CLAUDE.md — IG Trading Engine (Claude-specific)

> **Start here:** Read `AGENTS.md` first — it contains the full shared project reference for all AI tools.
> This file only adds Claude Code CLI-specific behaviour on top of that.

---

## How Claude Should Orient in Each Session

1. Read `AGENTS.md` — project overview, layout, conventions, active work
2. Read `TASK_TRACKER.md` — know what's in progress before suggesting anything
3. Read `PROJECT_ARCHITECTURE.md` only when the task involves data flow or module design
4. Dive into source files only for the specific file being worked on — avoid reading the whole codebase speculatively

---

## Claude-Specific Notes

- **Tool:** Claude Code CLI (terminal)
- The IG API is unreachable from sandboxed/proxied environments (`ProxyError: 403`). When running test scripts, explain this clearly rather than retrying.
- When editing Rust, flag any new `.unwrap()` or `println!` — both are banned. Use `.expect("reason")` or `?` instead of `.unwrap()`.
- `cargo clippy -- -D warnings` must exit 0 — zero warnings policy.
- Prefer `Edit` over `Write` for existing files.
- Dashboard (`src/`) is **archived** — do not modify frontend code.
- When completing any task, update `TASK_TRACKER.md` (🏗️ → ✅).
- If a task needs Gemini to continue, add it to `GEMINI.md` under "Active".

---

## Doc-Update Protocol (run after EVERY code change)

**Step 1 — AGENTS.md FIRST** (auto-loaded, stale facts here poison every future session):
- Is the `Current status:` line still accurate?
- Are the Strategy Ensemble, Risk Rules tables still accurate?
- Is the Repository Layout tree still accurate?
- If ANY answer is no → **update AGENTS.md before anything else**

**Step 2 — TASK_TRACKER.md** (always for tasks/bugs):
- Flip status (⏳ → 🏗️ → ✅), update header date + focus line
- Move fixed bugs to "Recently Fixed" with root cause

**Step 3 — PROJECT_ARCHITECTURE.md** (only if architecture changed — new module, new strategy, data flow change)

**Step 4 — TECH_DEBT_AUDIT.md** (only if a debt item was resolved)

**Step 5 — AI_ROADMAP.md** (only if Phase 8.x work was done)

> Rule: Never report "done" without completing Steps 1–2. Steps 3–5 are conditional.

---

## Operational Commands (Engine)

```bash
# Build + restart engine (run from ig-engine/)
cd /Users/ziscross/.openclaw/workspace/projects/ig-trading/ig-engine
cargo build --release 2>&1 | grep -E "^error|Compiling|Finished"
cargo test 2>&1 | grep -E "test result|FAILED"
kill $(cat ig-engine.pid 2>/dev/null) 2>/dev/null; sleep 1
nohup ./target/release/ig-engine >> logs/engine.log.$(date +%Y-%m-%d) 2>&1 &
echo $! > ig-engine.pid && echo "Started PID $(cat ig-engine.pid)"

# Check log (clean — no lightstreamer noise)
grep -v "lightstreamer\|\"span\"\|\"spans\"\|ConnectionDetails\|ConnectionOptions" \
  logs/engine.log.$(date +%Y-%m-%d) | tail -30

# Check for key events
grep -E "Bar closed|M15.*Bar|trade|SELL|BUY|signal|error|warn|self-heal|regime|Telegram" \
  logs/engine.log.$(date +%Y-%m-%d) | tail -30

# Check cron jobs
crontab -l

# Check cron script logs
tail -20 /Users/ziscross/.openclaw/workspace/projects/ig-trading/ig-engine/logs/cron_calendar.log

# Check regime file freshness
ls -la /Users/ziscross/.openclaw/workspace/projects/ig-trading/data/regime_latest.json
cat /Users/ziscross/.openclaw/workspace/projects/ig-trading/data/regime_latest.json | python3 -c \
  "import json,sys; d=json.load(sys.stdin); print(d.get('regime'), d.get('timestamp',''))"

# Check M15 candle files on disk
ls -lh /Users/ziscross/.openclaw/workspace/projects/ig-trading/ig-engine/data/candles/
```

---

## Assigned Tasks (Claude's Focus)

> Phases 1–15 + 14.A–I complete. Engine live and trading. Dashboard archived.

Claude owns the **Rust bot engine, Python ML pipeline, and Telegram notification** work.

### Status

- M15 dual-timeframe scheme fully live — tick accumulator, H1 gate, alignment bonus, self-heal, disk persistence
- Telegram send + receive working ✅
- Only long-term item: 8.6 RL position sizing (needs 3+ months live trade data in `logs/trades.jsonl`)

---

## Multi-Agent Setup

This project is used with **Claude Code CLI** and **Gemini CLI**.

| File | Read by | How | Purpose |
|------|---------|-----|---------|
| `AGENTS.md` | **All agents** | Directly | Shared source of truth — full project reference |
| `CLAUDE.md` | Claude Code CLI | Auto-loaded at session start | Claude-specific additions → then directs to `AGENTS.md` |
| `GEMINI.md` | Gemini CLI | Auto-loaded at session start | Gemini-specific additions → then directs to `AGENTS.md` |
| `PROJECT_ARCHITECTURE.md` | All agents | On demand | Deep module + interface reference |
| `TASK_TRACKER.md` | All agents | On demand | Live task status + bugs |
| `TECH_DEBT_AUDIT.md` | All agents | On demand | Debt audit + phase history |

**How the reading chain works:**
- **Claude Code CLI** → auto-loads `CLAUDE.md` → `CLAUDE.md` says read `AGENTS.md` → full context achieved
- **Gemini CLI** → auto-loads `GEMINI.md` → `GEMINI.md` says read `AGENTS.md` → full context achieved
- Both tools end up with identical shared knowledge from `AGENTS.md`

Do not duplicate content from `AGENTS.md` into this file.
