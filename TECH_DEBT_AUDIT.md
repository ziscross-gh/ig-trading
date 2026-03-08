# Tech Debt Audit — IG Trading Engine + Dashboard

**Date:** 2026-02-25
**Scope:** Rust engine (`ig-engine/`) + Next.js frontend (`src/`)
**Total findings:** 72 across 6 categories

---

## Prioritized Remediation Plan

Items scored using: **Priority = (Impact + Risk) × (6 − Effort)**

### Tier 1 — Critical / Do Now (Score ≥ 30)

| # | Category | Issue | Impact | Risk | Effort | Score | Files |
|---|----------|-------|--------|------|--------|-------|-------|
| 1 | Config | `ignoreBuildErrors: true` in next.config.ts silences all TS build errors | 5 | 5 | 1 | 50 | `next.config.ts` |
| 2 | Config | All ESLint rules disabled — linting is cosmetic only | 5 | 5 | 2 | 40 | `eslint.config.mjs` |
| 3 | Test | Zero frontend test files — no unit, integration, or component tests | 5 | 5 | 3 | 30 | Entire `src/` |
| 4 | Code | `.expect()` on env vars panics at runtime if unset | 4 | 5 | 1 | 45 | `event_loop.rs:57,61,65` |
| 5 | Code | `.unwrap()` on `Option<Scorecard>` in HTTP handler — 500 during warmup | 4 | 5 | 1 | 45 | `http_server.rs:864-865` |
| 6 | Arch | `useEngine` hook is 713 lines with 5+ responsibilities | 5 | 4 | 3 | 27 | `useEngine.ts` |

### Tier 2 — High / This Sprint (Score 20–29)

| # | Category | Issue | Impact | Risk | Effort | Score | Files |
|---|----------|-------|--------|------|--------|-------|-------|
| 7 | Arch | `EngineState` is a god object (20+ fields, no separation) | 4 | 4 | 4 | 16 | `state.rs` |
| 8 | Arch | `event_loop.rs` is 1057 lines mixing init, analysis, monitoring, notifications | 4 | 4 | 4 | 16 | `event_loop.rs` |
| 9 | Arch | No error boundaries — one throw crashes the whole dashboard | 4 | 5 | 2 | 36 | All components |
| 10 | Arch | No custom error types — everything is `anyhow::Result` + `.unwrap()` | 4 | 4 | 3 | 24 | Rust engine-wide |
| 11 | Arch | No trait abstraction for REST client — untestable | 4 | 3 | 3 | 21 | `rest_client.rs` |
| 12 | Test | No Rust integration tests (full event-loop cycle, risk pipeline) | 4 | 4 | 3 | 24 | `ig-engine/tests/` (missing) |
| 13 | Test | Missing unit tests for HTTP server, streaming, events, strategy traits | 4 | 4 | 3 | 24 | Multiple `.rs` files |
| 14 | Infra | No CI/CD pipeline (no cargo test, clippy, fmt, audit) | 3 | 5 | 2 | 32 | `.github/workflows/` (missing) |
| 15 | Config | `reactStrictMode: false` disables React dev safety checks | 3 | 3 | 1 | 30 | `next.config.ts` |
| 16 | Config | `noImplicitAny: false` in tsconfig defeats strict mode | 3 | 3 | 2 | 24 | `tsconfig.json` |

### Tier 3 — Medium / Next Sprint (Score 10–19)

| # | Category | Issue | Impact | Risk | Effort | Score | Files |
|---|----------|-------|--------|------|--------|-------|-------|
| 17 | Code | Multiple `any` types in hooks and components | 3 | 3 | 2 | 24 | `useEngine.ts`, `setup-panel.tsx`, `preflight-checks.ts` |
| 18 | Code | Hardcoded instrument specs (25+ margin/pip values inline) | 3 | 3 | 2 | 24 | `position_sizer.rs:20-95` |
| 19 | Code | 25 `#[allow(dead_code)]` suppressions — likely unused code | 2 | 2 | 2 | 16 | Multiple `.rs` files |
| 20 | Code | Emoji status icons instead of Lucide icons in setup panel | 2 | 1 | 1 | 15 | `setup-panel.tsx:96-103` |
| 21 | Arch | `setup-panel.tsx` is 391 lines — should be split | 3 | 2 | 2 | 20 | `setup-panel.tsx` |
| 22 | Arch | `page.tsx` is 398 lines — header and nav should extract | 3 | 2 | 2 | 20 | `page.tsx` |
| 23 | Arch | Prop drilling for market/engine data — no React Context | 3 | 2 | 3 | 15 | `page.tsx` → children |
| 24 | Arch | `SizingMethod` enum defined in two places | 2 | 2 | 1 | 20 | `config.rs`, `position_sizer.rs` |
| 25 | Arch | Complex split-borrow pattern in learning system | 2 | 3 | 3 | 15 | `event_loop.rs:858-887` |
| 26 | Dep | `tokio` features = "full" — loads everything | 2 | 1 | 1 | 15 | `Cargo.toml` |
| 27 | Dep | No `cargo audit` or `cargo-deny` for supply chain checks | 2 | 4 | 1 | 30 | `Cargo.toml` |
| 28 | Dep | Unused crypto deps (`rsa`, `base64`, `pkcs8`) | 2 | 1 | 1 | 15 | `Cargo.toml` |
| 29 | Doc | No module-level docs on 18 of 41 Rust files | 2 | 2 | 2 | 16 | Multiple `.rs` files |
| 30 | Doc | Missing JSDoc on all exported frontend functions | 2 | 2 | 2 | 16 | All hooks + components |
| 31 | Infra | No Docker/container config | 2 | 2 | 2 | 16 | Project root |
| 32 | Infra | No graceful shutdown — no SIGTERM handler, no position drain | 3 | 4 | 3 | 21 | `event_loop.rs` |
| 33 | Infra | Polling (10s/5s) instead of WebSocket for market data | 3 | 2 | 3 | 15 | `useMarketData.ts`, `page.tsx` |

### Tier 4 — Low / Backlog (Score < 10)

| # | Category | Issue | Files |
|---|----------|-------|-------|
| 34 | Code | Magic numbers: 250 warmup candles, 21 SGT hour, 100 rate limit | `event_loop.rs`, `rest_client.rs` |
| 35 | Code | Duplicated candle conversion logic | `event_loop.rs` |
| 36 | Code | `unwrap()` on float `partial_cmp` in optimizer (NaN panic) | `optimizer.rs:70` |
| 37 | Code | `unwrap()` on `candles.last()` without empty check | `backtester.rs:139` |
| 38 | Code | Hardcoded strategy range defaults in StrategyLab | `StrategyLab.tsx:15-17` |
| 39 | Dep | No version pinning on critical crates | `Cargo.toml` |
| 40 | Dep | Unused shadcn/ui components inflating bundle | `src/components/ui/` |
| 41 | Doc | Missing architecture doc for data flow | Project root |
| 42 | Doc | Hardcoded config values undocumented (margins, windows, hours) | Multiple |
| 43 | Infra | No structured logging / log rotation | `main.rs` |
| 44 | Infra | No readiness/liveness probe endpoints | `http_server.rs` |
| 45 | Infra | No request caching/deduplication for market data | `useMarketData.ts` |

---

## Phased Remediation Plan

### Phase 1 — Safety Net (COMPLETED)

**Goal:** Stop hiding errors, catch crashes before production.

1. ✅ Remove `ignoreBuildErrors: true` from `next.config.ts` → fix any TS errors that surface
2. ✅ Enable `reactStrictMode: true`
3. ✅ Set `noImplicitAny: true` in tsconfig → fix resulting errors
4. ✅ Replace `.expect()` on env vars with proper validation at startup (return `Result`, not panic)
5. ✅ Replace `.unwrap()` on optional scorecard/weight_manager in HTTP handler with 503 response
6. ✅ Add `error.tsx` error boundary at app level
7. ✅ Enable ESLint rules gradually: start with `no-explicit-any: warn`, `no-unused-vars: warn`, `react-hooks/exhaustive-deps: warn`

### Phase 2 — Testability (COMPLETED)

**Goal:** Make the codebase testable, add critical path tests.

1. ✅ Split `useEngine.ts` (713 lines) into: `useEngineAPI`, `useEngineWebSocket`, `useEngineControl`, `useEngineConfig`
2. ✅ Create custom `EngineError` enum in Rust to replace raw `anyhow`
3. ✅ Create `trait TraderAPI` for REST client abstraction
4. ✅ Add `vitest` + `@testing-library/react` to frontend
5. ✅ Write unit tests for hooks and critical components
6. ✅ Add Rust integration tests for event loop cycle with mock API
7. ✅ Set up CI: `cargo test`, `cargo clippy`, `tsc --noEmit`, `eslint`


### Phase 3 — Architecture Cleanup (COMPLETED)

**Goal:** Reduce coupling, improve maintainability.

1. ✅ Break `EngineState` into sub-structs: `AccountState`, `MarketDataState`, `TradingMetrics`, `LearningState`
2. ✅ Break `event_loop.rs` into modules: `initialization`, `analysis`, `position_management`, `notifications`
3. ✅ Split `setup-panel.tsx` into `PreFlightChecks`, `EngineSettings`, `RiskSettings`
4. ✅ Extract `DashboardHeader` and `MobileNav` from `page.tsx`
5. ✅ Create React Context for engine + market data
6. ✅ Consolidate `SizingMethod` enum to single source
7. ✅ Consolidate instrument specs into a config-driven HashMap

### Phase 4 — Production Hardening (ongoing)

**Goal:** Prepare for real money.

1. ✅ Add graceful shutdown with SIGTERM handler and position drain
2. ✅ Add `cargo audit` + `cargo-deny` to CI
3. ✅ Create Dockerfile with multi-stage build
4. ✅ Add structured logging with `tracing-appender`
5. ✅ Add readiness/liveness probes
6. 🏗️ Migrate market data polling to WebSocket push (In Progress: logic implemented, refining listeners)
7. 🏗️ Run bundle analysis and remove unused shadcn/ui components
8. ✅ Add module-level and function-level documentation
9. ✅ Fix Critical Bug: Indicators were not updating with new data after engine startup (implemented periodic 15min candle refresh)
10. ✅ Fix ESLint warnings: 39 → 0 across 17 files (unused vars, exhaustive-deps, optional catch)
11. ✅ Clean up Rust `#[allow(dead_code)]` suppressions: removed stale ones, standardized to field-level
12. ✅ Fix Telegram notifications silent for 2+ days: empty env var bug, HTTP error swallowing, startup ping added
13. ✅ Consolidate instrument name mapping: single `get_instrument_name()` in `telegram.rs`, removed duplicate from `http_server.rs`
14. ✅ Add `name` field to all API endpoints (positions, trades, signals, equity curve, market analysis, prices)
15. ✅ Fix EnginePanel showing raw epic codes instead of instrument names
16. ✅ Add demo + live epic variant coverage to both Rust and frontend name mappings

### Phase 5 — Advanced Strategy Features (active)

**Goal:** Increase profitability through refined execution and data-driven insights.

1. 🏗️ Implement Trailing Stop Loss logic in `position_management`
2. 🏗️ Add session-specific filters (Trading Hours refinement, News exclusion)
3. 🏗️ Enhance Strategy Lab with historical backtesting UI
4. ✅ Implement "Shadow Mode" for strategy validation (mapped to Paper mode)
5. 🏗️ Add Equity Curve visualization to the main dashboard

| Category | Critical | High | Medium | Low | Total |
|----------|----------|------|--------|-----|-------|
| Code | 2 | 1 | 4 | 5 | 12 |
| Architecture | 1 | 4 | 5 | 1 | 11 |
| Test | 1 | 2 | 0 | 0 | 3 |
| Dependencies | 1 | 1 | 3 | 2 | 7 |
| Documentation | 0 | 0 | 3 | 3 | 6 |
| Infrastructure | 0 | 1 | 3 | 2 | 6 |
| **Total** | **5** | **9** | **18** | **13** | **45** |

**Biggest risk:** The combination of `ignoreBuildErrors: true` + all ESLint rules disabled + zero tests means bugs can ship silently. Phase 1 closes this gap.

**Biggest effort:** Splitting `EngineState` and `event_loop.rs` (Phase 3) is the largest refactor but pays the most dividends for long-term velocity.
