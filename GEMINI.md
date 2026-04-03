# GEMINI.md — IG Trading Engine (Gemini-specific)

> **Start here:** Read `AGENTS.md` first — it contains the full shared project reference for all AI tools.
> This file only adds Gemini CLI-specific behaviour on top of that.

---

## How Gemini Should Orient in Each Session

1. Read `AGENTS.md` — project overview, layout, conventions, active work
2. Read `TASK_TRACKER.md` — know what's in progress before suggesting anything
3. Read `PROJECT_ARCHITECTURE.md` only when the task involves data flow or module design
4. Dive into source files only for the specific file being worked on — avoid reading the whole codebase speculatively

---

## Gemini CLI Notes

- **Tool:** Gemini CLI (terminal, no browser tools available)
- Use `read_file`, `run_shell_command`, and `write_file` for all work
- When editing Rust, flag any new `.unwrap()` or `println!` — both are banned. Use `.expect("reason")` or `?` instead of `.unwrap()`.
- `cargo clippy -- -D warnings` must exit 0 — zero warnings policy.
- Prefer editing existing files over creating new ones
- Dashboard (`src/`) is **archived** — do not modify frontend code.
- When completing a task, update `TASK_TRACKER.md` (🏗️ → ✅)
- If a task needs Claude to continue, add it to `CLAUDE.md` under "Active"

## Doc-Update Protocol (run after EVERY code change)

**Step 1 — AGENTS.md FIRST** (auto-loaded — stale facts here corrupt every future session):
- Is the `Current status:` line still accurate?
- Are the Strategy Ensemble, Risk Rules tables still accurate?
- If ANY answer is no → **update AGENTS.md before anything else**

**Step 2 — TASK_TRACKER.md** (always): flip status, update header, move fixed bugs to "Recently Fixed"

**Step 3 — PROJECT_ARCHITECTURE.md** (only if architecture changed)

**Step 4/5 — TECH_DEBT_AUDIT.md / AI_ROADMAP.md** (only if relevant debt/ML work done)

> Never report "done" without completing Steps 1–2.

---

## Assigned Tasks (Gemini's Focus)

> Phases 1–15 + 14.A–I complete. Engine live and trading. Dashboard archived.

Gemini owns **Rust engine hardening, backtesting, and ML pipeline validation** work.

### Confirmed Completed (verified in current source)

| # | Task | Phase | Notes |
|---|------|-------|-------|
| 7.5 | Backtest HTTP endpoint | Phase 7 | ✅ `POST /backtest` on port 9090 |
| L.1 | Close position fix | Live | ✅ `_method: DELETE` header — was completely broken |
| L.2 | Currency hybrid logic | Live | ✅ JPY/USD/account-base per-trade in order_manager.rs |
| L.4 | Config-driven guaranteed_stop | Live | ✅ Reads from `config.risk.limited_risk_account` |
| L.5 | api_lab CLI tool | Live | ✅ `src/bin/api_lab.rs` — list, close, inject trades |
| 9.1 | Leaky Bucket Rate Limiter | Phase 9 | ✅ `TokenBucket` in rest_client.rs — verified active |
| 9.2 | Granular Error Mapping | Phase 9 | ✅ `IGError` enum in errors.rs — integrated in handle_response |
| 10.1 | Client Sentiment Integration | Phase 10 | ✅ Polling loop in event_loop/mod.rs — verified active |
| 10.2 | Related Market Sentiment | Phase 10 | ✅ context_market_ids polling — verified active |
| 10.3 | Recursive API Pagination | Phase 10 | ✅ get_account_activity pagination — verified active |
| 12.1 | Regime-Switching Logic | Phase 12 | ✅ Multipliers in regime/mod.rs + analysis.rs |
| 13.1 | Birth Regime Tracking | Phase 13 | ✅ opened_in_regime on Position + ClosedTrade |
| 12.2 | Sentiment Velocity Guard | Phase 12 | ✅ macro_pause_until in MetricsState + analysis.rs |
| 12.3 | Dynamic Spread Gate | Phase 12 | ✅ avg_spread in MarketState + analysis.rs |
| 12.4 | Limit Order Migration | Phase 12 | ✅ order_type=LIMIT respected in order_manager.rs |
| 14.E | H1 Directional Bias/Gate | Phase 14 | ✅ H1 bias recording + M15 alignment gate in analysis.rs |

### Active Focus

| # | Task | Phase | Notes |
|---|------|-------|-------|
| 8.6 | RL Position Sizing | Phase 8 | **Data collection only**; RL agent implementation deferred until 3+ months of trade data is gathered (Phase 16+) |

---

## Multi-Agent Setup

This project is used with **Claude Code (CLI)** and **Gemini CLI**.

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
