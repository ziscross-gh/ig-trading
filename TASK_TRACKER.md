# TASK_TRACKER.md — IG Trading Engine

**Last updated:** 2026-03-10 (candle persistence layer)
**Current phase:** Production-ready. All engine phases complete.
**Current focus:** 🤖 Bot engine production-ready | 🧠 8.1–8.5, 8.7 ✅ done | 8.6 RL long-term (needs 3mo data)

> 📦 Dashboard (`src/`) is **archived** — not maintained. All dashboard tasks removed.

For the full history of completed work and debt items, see `TECH_DEBT_AUDIT.md`.

---

## Phase Summary

| Phase | Name | Status |
|-------|------|--------|
| 1 | Safety Net | ✅ Complete |
| 2 | Testability | ✅ Complete |
| 3 | Architecture Cleanup | ✅ Complete |
| 4 | Production Hardening | ✅ Complete |
| 5 | Advanced Strategy Features | ✅ Complete |
| 6 | Engine Hardening / WS Migration | ✅ Complete |
| 7 | Production Backtesting | ✅ Complete |
| 8 | AI/ML Enhancements | ✅ 8.1–8.5, 8.7 done · 8.6 long-term |

---

## Phase 5 — Advanced Strategy Features

| # | Task | Status | Owner | Notes |
|---|------|--------|-------|-------|
| 5.1 | Trailing Stop Loss logic | ✅ Done | Claude + Gemini | Ratchet logic + strategy-specific distances from config; configurable `trailing_stop_min_pips` in RiskConfig |
| 5.2 | Session-specific filters (news exclusion) | ✅ Done | Claude | Session filter + news blackout windows (±15min configurable) |
| 5.4 | Shadow Mode (Paper mode strategy validation) | ✅ Done | — | Mapped to Paper mode |

---

## Known Bugs / Open Issues

| Priority | Description | File(s) |
|----------|-------------|---------|
| High | Python test scripts (`test_ig_trade*.py`) fail in any proxied/sandboxed environment — `ProxyError: 403 Forbidden` on IG API. Must run locally or in Docker. | `test_ig_trade*.py` |

---

## Phase 6 — Engine Hardening (Complete)

| # | Task | Owner | Status | Rationale |
|---|------|-------|--------|-----------|
| 6.1 | Multi-timeframe analysis | Claude | ✅ Done | Evaluates trend, signal, and entry timeframes together |
| 6.2 | WebSocket push fully replace REST polling | Claude | ✅ Done | BarAccumulator drives OHLCV bars from WS ticks |
| 6.4 | Fix remaining `unwrap()` panics in optimizer + backtester | Claude | ✅ Done | Safety for live mode |
| 6.5 | Live mode pre-flight checklist | Gemini | ✅ Done | LIVE_PREFLIGHT_CHECKLIST.md |
| 6.7 | Engine hardening weekend session | Claude | ✅ Done | 7 improvements: MARKET_STATE propagation, state worker, bar-close gating, VecDeque, dedup, log levels, unwrap cleanup |
| 6.8 | Candle persistence layer (survive restarts) | Claude | ✅ Done | JSONL disk cache → instant warmup on restart. Disk-first startup, persist on bar close + shutdown. |

---

## Phase 7 — Production Backtesting (Complete)

| # | Task | Owner | Status | Rationale |
|---|------|-------|--------|-----------|
| 7.1 | Historical candle data fetcher | Claude | ✅ Done | `scripts/fetch_historical_data.py` — yfinance 2yr 1H OHLCV |
| 7.2 | Python backtester — ensemble + trailing stop | Claude | ✅ Done | Portfolio +$2,625 (+26%) at 2.97% max DD |
| 7.3 | Parameter optimizer | Claude | ✅ Done | `scripts/optimize.py` — grid search |
| 7.4 | ADX range filter in Rust engine | Claude | ✅ Done | Strategy override per instrument |
| 7.5 | Backtest HTTP endpoint | Gemini | ✅ Done | `POST /backtest` on port 9090 |

---

## Phase 8 — AI/ML Enhancements

> **Full details:** See `AI_ROADMAP.md`
> **Philosophy:** AI is additive — classical ensemble stays as core, AI layers on top.

| # | Task | Owner | Status | Priority | Rationale |
|---|------|-------|--------|----------|-----------|
| 8.1 | Walk-forward auto re-optimisation | Claude | ✅ Done | 🔴 High | Weekly self-tuning via SIGUSR1 hot-reload |
| 8.2 | Performance-based strategy weighting | Claude | ✅ Done | 🔴 High | Adaptive weights every 10 trades, rolling 50-trade window |
| 8.3 | Gold news sentiment signal | Claude | ✅ Done | 🟠 Medium | RSS → keyword/Ollama/Claude scoring → 5th Signal for Gold |
| 8.4 | ML regime classifier | Claude | ✅ Done | 🟠 Medium | LightGBM per instrument → TRENDING/RANGING/VOLATILE multipliers |
| 8.5 | Macro calendar awareness | Claude | ✅ Done | 🟡 Low | Per-event blackout windows (NFP ±30min, FOMC ±60min, etc.) |
| 8.6 | RL position sizing | Claude | 🏗️ Long-term | 🔵 | PPO on live trade outcomes. `TradeLogger` recording to `logs/trades.jsonl`. Needs 3+ months data. |
| 8.7 | Code quality pass — zero clippy warnings | Claude | ✅ Done | 🔴 High | `cargo clippy -- -D warnings` exits 0. All 66 tests pass. |

### Data Collection — Active

Every trade logged is future training data for 8.6:

| Data | File | Purpose |
|------|------|---------|
| OHLCV candles | `data/candles/*.jsonl` | Persist across restarts, regime classifier, re-optimise |
| Trade outcomes | `logs/trades.jsonl` | Strategy weighting, RL |
| Strategy signals | `logs/signals.jsonl` | Win rate tracking |
| Sentiment scores | `data/sentiment.db` | Sentiment validation |

---

## How to Update This File

- When starting a task: change status to 🏗️ In Progress
- When completing a task: change status to ✅ Done
- When discovering a new bug: add a row to the Known Bugs table
