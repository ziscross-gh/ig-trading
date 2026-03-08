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
- When editing Rust, flag any new `.unwrap()` or `println!` — both violate project conventions.
- When editing TypeScript, confirm `noImplicitAny: true` is respected — no bare `any` types.
- Prefer `Edit` over `Write` for existing files.
- When completing any task, update `TASK_TRACKER.md` (🏗️ → ✅).
- If a task needs Gemini to continue, add it to `GEMINI.md` under "Active".

---

## Assigned Tasks (Claude's Focus)

> ⚠️ **Current focus: Bot engine + Telegram only.** All dashboard/frontend work is paused.
> Phases 5, 6, 8.1–8.5 are all complete. See `TASK_TRACKER.md` for full history.

Claude owns the **Rust bot engine, Python ML pipeline, and Telegram notification** work.

### Active

All current active tasks have been handed to Gemini CLI. See `GEMINI.md`.

### Bug Fixes (low priority)

- **Low:** `optimizer.rs:70` — NaN panic on `partial_cmp`
- **Low:** `backtester.rs:139` — `candles.last()` without empty guard

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
