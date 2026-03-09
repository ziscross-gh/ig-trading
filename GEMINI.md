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

---

## Assigned Tasks (Gemini's Focus)

> All phases 1–8.7 complete. Dashboard archived. Engine production-ready.

Gemini owns **Rust engine hardening, backtesting, and ML pipeline validation** work.

### Completed

| # | Task | Phase | Notes |
|---|------|-------|-------|
| 7.5 | Backtest HTTP endpoint | Phase 7 | ✅ `POST /backtest` on port 9090 |
| 8.4-test | Regime classifier smoke test | Phase 8 | ✅ Verified TRENDING/RANGING/VOLATILE labels |

### Long-term (needs 3+ months live data)

| # | Task | Notes |
|---|------|-------|
| 8.6 | RL position sizing | PPO agent on live trade outcomes. Do not start until 3 months of `logs/trades.jsonl` data accumulated. |

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
