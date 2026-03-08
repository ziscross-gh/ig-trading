# GEMINI.md — IG Trading Engine (Gemini-specific)

> **Start here:** Read `AGENTS.md` first — it contains the full shared project reference for all AI tools.
> This file only adds Gemini / Antigravity-specific behaviour on top of that.

---

## How Gemini Should Orient in Each Session

1. Read `AGENTS.md` — project overview, layout, conventions, active work
2. Read `TASK_TRACKER.md` — know what's in progress before suggesting anything
3. Read `PROJECT_ARCHITECTURE.md` only when the task involves data flow or module design
4. Dive into source files only for the specific file being worked on — avoid reading the whole codebase speculatively

---

## Gemini-Specific Notes

- When editing Rust, flag any new `.unwrap()` or `println!` — both violate project conventions.
- When editing TypeScript, confirm `noImplicitAny: true` is respected — no bare `any` types.
- Use browser tools to visually verify frontend changes when possible.
- When completing a task, update `TASK_TRACKER.md` (🏗️ → ✅).

---

## Assigned Tasks (Gemini's Focus)

> ⚠️ **Current focus: Bot engine + Telegram only.** All dashboard/frontend work is paused.

Gemini supports **bot-side Rust work and Telegram notification refinement**. Refer to `TASK_TRACKER.md` for full details.

### Active

| # | Task | Phase |
|---|------|-------|
| 5.1 | Trailing Stop Loss logic (support / review) | Phase 5 |
| 5.2 | Session-specific filters (support / review) | Phase 5 |

### Planned (Phase 6)

| # | Task |
|---|------|
| 6.5 | Live mode pre-flight checklist (bot-side validation) |

### Paused (Dashboard — Resume Later)

| # | Task |
|---|------|
| 4.7 | Bundle analysis + remove unused shadcn/ui components |
| 5.3 | Strategy Lab historical backtesting UI |
| 5.5 | Equity Curve visualization on dashboard |
| 6.3 | Bundle tree-shake unused shadcn/ui components |
| 6.6 | Trade journal export (CSV/PDF) |

---

## Multi-Agent Setup

This project is used with both **Claude Cowork** and **Google Antigravity**.

| File | Read by | How | Purpose |
|------|---------|-----|---------|
| `AGENTS.md` | **All agents** | Directly | Shared source of truth — full project reference |
| `CLAUDE.md` | Claude / Cowork | Auto-loaded at session start | Claude-specific additions → then directs to `AGENTS.md` |
| `GEMINI.md` | Google Antigravity | Auto-loaded at session start | Gemini-specific additions → then directs to `AGENTS.md` |
| `PROJECT_ARCHITECTURE.md` | All agents | On demand | Deep module + interface reference |
| `TASK_TRACKER.md` | All agents | On demand | Live task status + bugs |
| `TECH_DEBT_AUDIT.md` | All agents | On demand | Debt audit + phase history |

**How the reading chain works:**
- **Claude Cowork** → auto-loads `CLAUDE.md` → `CLAUDE.md` says read `AGENTS.md` → full context achieved
- **Google Antigravity** → auto-loads `GEMINI.md` → `GEMINI.md` says read `AGENTS.md` → full context achieved
- Both tools end up with identical shared knowledge from `AGENTS.md`

Do not duplicate content from `AGENTS.md` into this file.
