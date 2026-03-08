# TASK_TRACKER.md — IG Trading Engine

**Last updated:** 2026-03-08 (session 5)
**Current phase:** Phase 8 — AI/ML Enhancements (active)
**Current focus:** 🤖 Bot engine + Telegram only (dashboard paused) | 🧠 8.1 ✅ 8.2 ✅ 8.3 ✅ 8.4 ✅ 8.5 ✅ 8.7 ✅ done | Next: 8.6 RL position sizing (needs 3mo live data) or 7.5 backtest endpoint

For the full history of completed work and debt items, see `TECH_DEBT_AUDIT.md`.

---

## Phase Summary

| Phase | Name | Status |
|-------|------|--------|
| 1 | Safety Net | ✅ Complete |
| 2 | Testability | ✅ Complete |
| 3 | Architecture Cleanup | ✅ Complete |
| 4 | Production Hardening | 🏗️ Mostly done — 2 items paused |
| 5 | Advanced Strategy Features | ✅ Mostly done (dashboard items paused) |
| 6 | Engine Hardening / WS Migration | ✅ Complete |
| 7 | Production Backtesting | 🏗️ 4/6 done — 7.5 planned, 7.6 paused |
| 8 | AI/ML Enhancements | 🏗️ In progress — 8.1 ✅ 8.2 ✅ 8.3 ✅ 8.4 ✅ 8.5 ✅ | see `AI_ROADMAP.md` |

---

## Phase 4 — Production Hardening (Remaining)

| # | Task | Status | Owner | File(s) |
|---|------|--------|-------|---------|
| 4.6 | Migrate market data from REST polling to WebSocket push | ✅ Done | Claude | `useMarketData.ts`, `streaming_client.rs` |
| 4.7 | Bundle analysis + remove unused shadcn/ui components | ⏸️ Paused (dashboard) | — | `src/components/ui/` |

---

## Phase 5 — Advanced Strategy Features (Active)

| # | Task | Status | Owner | File(s) | Notes |
|---|------|--------|-------|---------|-------|
| 5.1 | Trailing Stop Loss logic | ✅ Done | Claude + Gemini | `event_loop/handlers.rs`, `analysis.rs`, `state.rs` | Ratchet logic complete; Gemini enhanced to support strategy-specific distances from config |
| 5.2 | Session-specific filters (news exclusion, refined hours) | ✅ Done | Claude | `risk/mod.rs`, `config/default.toml` | Session filter active; trading hours re-enabled; news blackout windows implemented (±15min, configurable) |
| 5.3 | Strategy Lab historical backtesting UI | ⏸️ Paused (dashboard) | — | `StrategyLab.tsx`, `backtester.rs` | Backend ready; UI wiring paused |
| 5.4 | Shadow Mode (Paper mode strategy validation) | ✅ Done | — | `engine/config.rs` | Mapped to Paper mode |
| 5.5 | Equity Curve visualisation on dashboard | ⏸️ Paused (dashboard) | — | `EquityCurvePanel.tsx` | Component exists; paused |

---

## Known Bugs / Open Issues

| Priority | Description | File(s) |
|----------|-------------|---------|
| High | Python test scripts (`test_ig_trade*.py`) fail in any proxied/sandboxed environment — `ProxyError: 403 Forbidden` on IG API. Must run locally or in Docker. | `test_ig_trade*.py` |
| Low | Dashboard `useMarketData.ts` may still fall back to REST polling — dashboard work is paused | `useMarketData.ts` |

---

## Phase 6 — Planned (Not Started)

These items are candidates for the next sprint once Phase 5 is complete.
⚠️ **Dashboard items paused** — focus is bot + Telegram only.

| # | Task | Owner | Status | Rationale |
|---|------|-------|--------|-----------|
| 6.1 | Multi-timeframe analysis | Claude | ✅ Done | Evaluates trend, signal, and entry timeframes together. Requires `trend_tf`, `signal_tf`, and `entry_tf` correctly configured in IG format |
| 6.2 | WebSocket push fully replace REST polling (finish 4.6) | Claude | ✅ Done | BarAccumulator drives OHLCV bars from WS ticks; candle_refresh_interval removed; borrow bug in handlers.rs fixed |
| 6.3 | Bundle analysis and tree-shake unused shadcn/ui components | — | ⏸️ Paused | Reduce frontend bundle size |
| 6.4 | Fix remaining low-priority `unwrap()` panics in optimizer + backtester | Claude | ✅ Done | Safety for live mode |
| 6.5 | Live mode pre-flight checklist | Gemini | ✅ Done | Wrote LIVE_PREFLIGHT_CHECKLIST.md with guidelines for real-money trading |
| 6.6 | Trade journal export (CSV/PDF) | — | ⏸️ Paused | Useful for tax/performance review |
| 6.7 | Engine hardening weekend session (Opus plan + Sonnet exec) | Claude | ✅ Done | 7 improvements: MARKET_STATE propagation (skip analysis when closed), state worker extracted from reconnect loop (no more resource churn), bar-close gating for analyze_market (hourly not per-tick), VecDeque ring buffers, ClosedTrade dedup, log level fix (trace!), unwrap convention fix |

---

## Phase 7 — Production Backtesting (Planned)

> **Trigger:** After demo mode has accumulated meaningful trade history (1–2 weeks of real signal data).
> Goal is a backtester that faithfully mirrors the live engine so results are trustworthy.

| # | Task | Owner | Status | Rationale |
|---|------|-------|--------|-----------|
| 7.1 | Historical candle data fetcher | Claude | ✅ Done | `scripts/fetch_historical_data.py` — yfinance pulls 2yr 1H OHLCV for EURUSD/USDJPY/GOLD into `data/*.json` |
| 7.2 | Python backtester — ensemble + trailing stop + session filter | Claude | ✅ Done | `scripts/backtest.py` — per-instrument strategy sets, ensemble vote, ratchet trailing stop, session filter. Results: GOLD +18.5%, USDJPY +8.4%, Portfolio +$2,625 (+26%) at 2.97% max DD |
| 7.3 | Parameter optimizer | Claude | ✅ Done | `scripts/optimize.py` — grid search across SL/TP/ADX filter/trail. ADX range filter is key |
| 7.4 | ADX range filter in Rust engine | Claude | ✅ Done | `analysis.rs` suppresses RSI_Reversal+Bollinger_Bands when ADX > 25; `config.rs` adds `InstrumentStrategyOverride`; `default.toml` enabled for all 3 instruments |
| 7.5 | Backtest CLI / HTTP endpoint | Gemini | ✅ Done | Expose `POST /backtest` on port 9090; verified with integration test `tests/backtest_api.rs` |
| 7.6 | Strategy Lab UI wiring (dashboard) | — | ⏸️ Paused | Wire `StrategyLab.tsx` to the `/backtest` endpoint — blocked on dashboard work resuming |

---

## Phase 8 — AI/ML Enhancements (Planned)

> **Full details:** See `AI_ROADMAP.md` — architecture, prompts, data requirements, implementation order.
> **Trigger:** Start 8.1 now (builds on existing optimize.py). 8.2–8.6 require live trade data.
> **Philosophy:** AI is additive — classical ensemble stays as core, AI layers on top.

| # | Task | Owner | Status | Priority | Rationale |
|---|------|-------|--------|----------|-----------|
| 8.1 | Walk-forward auto re-optimisation | Claude | ✅ Done | 🔴 High | `optimize.py --output`, `fetch_historical_data.py --months`, `compare_params.py`, `weekly_reoptimise.sh`, SIGUSR1 hot-reload + PID file in Rust. Engine self-tunes weekly with no downtime. |
| 8.2 | Performance-based strategy weighting | Claude | ✅ Done | 🔴 High | Was already fully wired: `event_loop/mod.rs` initialises `StrategyScorecard` + `AdaptiveWeightManager` at startup; `handlers.rs` feeds every closed trade in and propagates weight updates to ensemble. Fixed latent bug: ensemble weight keys used wrong names (`"MACrossover"` etc.) instead of matching `strategy.name()` (`"MA_Crossover"`). Also registered `"Gold_Sentiment"` weight. System now live — weights adjust automatically every 10 trades once each strategy has ≥ 20. |
| 8.3 | Gold news sentiment signal | Claude | ✅ Done | 🟠 Medium | `scripts/sentiment_agent.py` polls Reuters/Yahoo/Kitco RSS every 15min, scores Gold headlines via keyword/Ollama/Claude (auto-detect), writes `data/gold_sentiment_latest.json`. Rust `analysis.rs` injects sentiment as 5th Signal for Gold epic when `|score| ≥ 0.55` and file age < 30min. Strength = `6.0 + confidence × 3.5`, SL/TP from ATR. Cron: `*/15 * * * * python scripts/sentiment_agent.py --once`. |
| 8.4 | ML regime classifier (replace fixed ADX=25) | Claude | ✅ Done | 🟠 Medium | `scripts/train_regime_classifier.py` generates labels via forward-window mini-backtest (LOOKBACK=50, FORWARD=60) + trains LightGBM/sklearn per instrument. `scripts/run_regime_classifier.py` runs hourly, writes `data/regime_latest.json`. `ig-engine/src/regime/mod.rs` reads file, calls `apply_regime_multipliers()` in `analysis.rs` before `ensemble.vote()`: TRENDING → trend ×1.5 / reversion ×0.3; RANGING → reversion ×1.5 / trend ×0.3; VOLATILE → all ×0.4. No stale data accepted (90min TTL). Works on existing 2yr yfinance data — no live data needed. |
| 8.5 | Macro calendar awareness | Claude | ✅ Done | 🟡 Low | Added `MacroEvent` struct with per-event `blackout_mins`. `[[risk.macro_events]]` TOML array in `default.toml` — 8 events: London open ±20min, NFP/CPI ±30min, FOMC decision ±60min, FOMC presser ±45min, ECB/BOE ±45min. Legacy flat `news_blackout_windows_utc` used as fallback. Backward-compatible — old configs without `macro_events` keep flat 15min behaviour. |
| 8.6 | RL position sizing | Claude | 🏗️ In Progress | PPO agent learns optimal size multiplier from live trade outcomes. Data prep complete: `TradeLogger` now records every outcome to `logs/trades.jsonl` for future training. |
| 8.7 | Code quality pass — zero clippy warnings | Claude | ✅ Done | 🔴 High | `cargo clippy -- -D warnings` now exits 0 (was 40 errors). Fixes: optimizer.rs `total_cmp` + `.first().ok_or_else()` panic guards (return type → `anyhow::Result<>`); `or_default()` × 3 (candle_store); `is_none_or` / `is_some_and` (handlers + validation); collapsed identical if/else branches in all strategies; `OPU` → `Opu` acronym; doc-comment blank lines (atr/adx); indexed loops → slice iterators; `RingBuffer::is_empty()`; `clamp()` × 2; doc-quote markers; modulo-1 dead-code removed; `&mut [Signal]` slice API; 5× `#[allow(clippy::too_many_arguments)]` + `// TODO: struct refactor`. All 63 unit tests pass. |

### Data Collection — Start Now

Every trade logged today is future training data. Ensure structured logging is enabled:

| Data | File | Purpose |
|------|------|---------|
| OHLCV candles | `data/*.json` | Regime classifier, re-optimise |
| Trade outcomes | `logs/trades.jsonl` | Strategy weighting, RL |
| Strategy signals | `logs/signals.jsonl` | Win rate tracking |
| Sentiment scores | `data/sentiment.db` | Sentiment validation |

### Recommended Order

```
8.1 → 8.3 → 8.2 → 8.4 → 8.5 → 8.6
(quickest win first, data-hungry models last)
```

---

## How to Update This File

- When starting a task: change status to 🏗️ In Progress, add your name/date if helpful
- When completing a task: change status to ✅ Done and move it to the "completed" section of the relevant phase in `TECH_DEBT_AUDIT.md`
- When discovering a new bug: add a row to the Known Bugs table with priority (High / Medium / Low)
- When planning new work: add items to Phase 6 or create a new phase block
