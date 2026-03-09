# Tech Debt Audit — IG Trading Engine

**Date:** 2026-02-25 (updated 2026-03-09)
**Scope:** Rust engine (`ig-engine/`) only — dashboard archived
**Total findings (original):** 72 across 6 categories — frontend items archived

---

## Prioritized Remediation Plan

Items scored using: **Priority = (Impact + Risk) × (6 − Effort)**

### Tier 1 — Critical / Do Now (Score ≥ 30)

| # | Category | Issue | Impact | Risk | Effort | Score | Files |
|---|----------|-------|--------|------|--------|-------|-------|
| 4 | Code | `.expect()` on env vars panics at runtime if unset | 4 | 5 | 1 | 45 | `event_loop.rs:57,61,65` |
| 5 | Code | `.unwrap()` on `Option<Scorecard>` in HTTP handler — 500 during warmup | 4 | 5 | 1 | 45 | `http_server.rs:864-865` |

### Tier 2 — High / This Sprint (Score 20–29)

| # | Category | Issue | Impact | Risk | Effort | Score | Files |
|---|----------|-------|--------|------|--------|-------|-------|
| 7 | Arch | `EngineState` is a god object (20+ fields, no separation) | 4 | 4 | 4 | 16 | `state.rs` |
| 8 | Arch | `event_loop.rs` is 1057 lines mixing init, analysis, monitoring, notifications | 4 | 4 | 4 | 16 | `event_loop.rs` |
| 10 | Arch | No custom error types — everything is `anyhow::Result` + `.unwrap()` | 4 | 4 | 3 | 24 | Rust engine-wide |
| 11 | Arch | No trait abstraction for REST client — untestable | 4 | 3 | 3 | 21 | `rest_client.rs` |
| 12 | Test | No Rust integration tests (full event-loop cycle, risk pipeline) | 4 | 4 | 3 | 24 | `ig-engine/tests/` (missing) |
| 13 | Test | Missing unit tests for HTTP server, streaming, events, strategy traits | 4 | 4 | 3 | 24 | Multiple `.rs` files |
| 14 | Infra | No CI/CD pipeline (no cargo test, clippy, fmt, audit) | 3 | 5 | 2 | 32 | `.github/workflows/` (missing) |

### Tier 3 — Medium / Next Sprint (Score 10–19)

| # | Category | Issue | Impact | Risk | Effort | Score | Files |
|---|----------|-------|--------|------|--------|-------|-------|
| 18 | Code | Hardcoded instrument specs (25+ margin/pip values inline) | 3 | 3 | 2 | 24 | `position_sizer.rs:20-95` |
| 19 | Code | 25 `#[allow(dead_code)]` suppressions — likely unused code | 2 | 2 | 2 | 16 | Multiple `.rs` files |
| 24 | Arch | `SizingMethod` enum defined in two places | 2 | 2 | 1 | 20 | `config.rs`, `position_sizer.rs` |
| 25 | Arch | Complex split-borrow pattern in learning system | 2 | 3 | 3 | 15 | `event_loop.rs:858-887` |
| 26 | Dep | `tokio` features = "full" — loads everything | 2 | 1 | 1 | 15 | `Cargo.toml` |
| 27 | Dep | No `cargo audit` or `cargo-deny` for supply chain checks | 2 | 4 | 1 | 30 | `Cargo.toml` |
| 28 | Dep | Unused crypto deps (`rsa`, `base64`, `pkcs8`) | 2 | 1 | 1 | 15 | `Cargo.toml` |
| 29 | Doc | No module-level docs on 18 of 41 Rust files | 2 | 2 | 2 | 16 | Multiple `.rs` files |
| 31 | Infra | No Docker/container config | 2 | 2 | 2 | 16 | Project root |
| 32 | Infra | No graceful shutdown — no SIGTERM handler, no position drain | 3 | 4 | 3 | 21 | `event_loop.rs` |

### Tier 4 — Low / Backlog (Score < 10)

| # | Category | Issue | Files |
|---|----------|-------|-------|
| 34 | Code | Magic numbers: 250 warmup candles, 21 SGT hour, 100 rate limit | `event_loop.rs`, `rest_client.rs` |
| 35 | Code | Duplicated candle conversion logic | `event_loop.rs` |
| 36 | Code | ~~`unwrap()` on float `partial_cmp` in optimizer (NaN panic)~~ | ~~`optimizer.rs:70`~~ ✅ Fixed in 8.7 |
| 37 | Code | ~~`unwrap()` on `candles.last()` without empty check~~ | ~~`backtester.rs:139`~~ ✅ Safe — guarded by `if let Some` |
| 39 | Dep | No version pinning on critical crates | `Cargo.toml` |
| 41 | Doc | Missing architecture doc for data flow | Project root |
| 42 | Doc | Hardcoded config values undocumented (margins, windows, hours) | Multiple |
| 43 | Infra | No structured logging / log rotation | `main.rs` |
| 44 | Infra | No readiness/liveness probe endpoints | `http_server.rs` |

---

## Phased Remediation Plan

### Phase 1 — Safety Net (COMPLETED)

**Goal:** Stop hiding errors, catch crashes before production.

1. ✅ Replace `.expect()` on env vars with proper validation at startup (return `Result`, not panic)
2. ✅ Replace `.unwrap()` on optional scorecard/weight_manager in HTTP handler with 503 response
3. ✅ Frontend items (ignoreBuildErrors, reactStrictMode, noImplicitAny, error boundary, ESLint) — archived

### Phase 2 — Testability (COMPLETED)

**Goal:** Make the codebase testable, add critical path tests.

1. ✅ Create custom `EngineError` enum in Rust to replace raw `anyhow`
2. ✅ Create `trait TraderAPI` for REST client abstraction
3. ✅ Add Rust integration tests for event loop cycle with mock API
4. ✅ Set up CI: `cargo test`, `cargo clippy`
5. ✅ Frontend items (split useEngine, vitest, testing-library) — archived


### Phase 3 — Architecture Cleanup (COMPLETED)

**Goal:** Reduce coupling, improve maintainability.

1. ✅ Break `EngineState` into sub-structs: `AccountState`, `MarketDataState`, `TradingMetrics`, `LearningState`
2. ✅ Break `event_loop.rs` into modules: `initialization`, `analysis`, `position_management`, `notifications`
3. ✅ Consolidate `SizingMethod` enum to single source
4. ✅ Consolidate instrument specs into a config-driven HashMap
5. ✅ Frontend items (split setup-panel, extract layouts, React Context) — archived

### Phase 4 — Production Hardening (ongoing)

**Goal:** Prepare for real money.

1. ✅ Add graceful shutdown with SIGTERM handler and position drain
2. ✅ Add `cargo audit` + `cargo-deny` to CI
3. ✅ Create Dockerfile with multi-stage build
4. ✅ Add structured logging with `tracing-appender`
5. ✅ Add readiness/liveness probes
6. ✅ Migrate market data polling to WebSocket push
7. ✅ Add module-level and function-level documentation
9. ✅ Fix Critical Bug: Indicators were not updating with new data after engine startup (implemented periodic 15min candle refresh)
10. ✅ Fix ESLint warnings: 39 → 0 across 17 files (unused vars, exhaustive-deps, optional catch)
11. ✅ Clean up Rust `#[allow(dead_code)]` suppressions: removed stale ones, standardized to field-level
12. ✅ Fix Telegram notifications silent for 2+ days: empty env var bug, HTTP error swallowing, startup ping added
13. ✅ Consolidate instrument name mapping: single `get_instrument_name()` in `telegram.rs`, removed duplicate from `http_server.rs`
14. ✅ Add `name` field to all API endpoints (positions, trades, signals, equity curve, market analysis, prices)
15. ✅ Fix EnginePanel showing raw epic codes instead of instrument names
16. ✅ Add demo + live epic variant coverage to both Rust and frontend name mappings

### Phase 5 — Advanced Strategy Features (Complete)

1. ✅ Trailing Stop Loss logic — ratchet + strategy-specific distances
2. ✅ Session-specific filters (trading hours, news blackout)
3. ✅ Shadow Mode (mapped to Paper mode)
4. ✅ Strategy Lab backend (UI archived)

| Category | Critical | High | Medium | Low | Total |
|----------|----------|------|--------|-----|-------|
| Code | 2 | 1 | 4 | 5 | 12 |
| Architecture | 1 | 4 | 5 | 1 | 11 |
| Test | 1 | 2 | 0 | 0 | 3 |
| Dependencies | 1 | 1 | 3 | 2 | 7 |
| Documentation | 0 | 0 | 3 | 3 | 6 |
| Infrastructure | 0 | 1 | 3 | 2 | 6 |
| **Total** | **5** | **9** | **18** | **13** | **45** |

**Current state:** All critical engine debt resolved through Phases 1–8.7. Remaining items are low-priority Rust cleanup (dead code, deps, docs). Frontend debt is archived — dashboard not maintained.
