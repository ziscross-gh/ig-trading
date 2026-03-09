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

## Assigned Tasks (Claude's Focus)

> All phases 1–8.7 complete. Dashboard archived. Engine production-ready.

Claude owns the **Rust bot engine, Python ML pipeline, and Telegram notification** work.

### Status

- All known bugs fixed (optimizer NaN ✅, backtester guard ✅, Bollinger NaN ✅)
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
