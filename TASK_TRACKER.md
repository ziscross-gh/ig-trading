# TASK_TRACKER.md — IG Trading Engine

**Last updated:** 2026-03-12 (indicator warmup + MARKET_STATE fixes — engine now produces signals)
**Current phase:** Production-ready. All engine phases complete. Live transition planning in progress.
**Current focus:** 🤖 Bot engine production-ready | 🧠 8.1–8.5, 8.7 ✅ done | 8.6 RL long-term (needs 3mo data) | 🔜 Live trading transition

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
| 9 | High-Availability API | ⏳ Planned (code wiped during restore — orphan files remain) |
| 10 | Connectivity & Intelligence | ⏳ Planned (code wiped during restore — orphan files remain) |
| 11 | Advanced Deployment | ⏳ Planned |
| 12 | Tactical Volatility | 🏗️ In Progress |
| 13 | Sticky Trade DNA | ⏳ Planned |

---

## Phase 12 — Tactical Volatility (Engagement)

| # | Task | Owner | Status | Notes |
|---|------|-------|--------|-------|
| 12.1 | Regime-Switching Logic | Claude | ✅ Done | regime/mod.rs + apply_regime_multipliers in analysis.rs; TRENDING/RANGING/VOLATILE signal multipliers |
| 12.2 | Sentiment Velocity Guard | Claude | ✅ Done | velocity>0.5 sets macro_pause_until (+2h) in MetricsState; analysis.rs checks it before all trade entries |
| 12.3 | Dynamic Spread Gate | Claude | ✅ Done | avg_spread EMA(0.05) on MarketState; spread>1.5×avg rejects trade in analysis.rs |
| 12.4 | Limit Order Migration | Claude | ✅ Done | VOLATILE regime sets AdjustedTrade.order_type=LIMIT+entry_level; order_manager uses it |

---

## Phase 13 — Sticky Trade DNA (Management)

| # | Task | Owner | Status | Notes |
|---|------|-------|--------|-------|
| 13.1 | Birth Regime Tracking | Claude | ✅ Done | opened_in_regime: Option<String> on Position; set in analysis.rs at trade open |
| 13.2 | Management Personalities | Claude | ✅ Done | handlers.rs: VOLATILE-birth → break-even snap; TRENDING-birth+current VOLATILE → skip ratchet |
| 13.3 | Genetic P&L Logging | Claude | ✅ Done | ClosedTrade.opened_in_regime serialised to trades.jsonl via existing TradeLogger |

---

## Phase 9 — High-Availability API (Robustness)

| # | Task | Owner | Status | Notes |
|---|------|-------|--------|-------|
| 9.1 | Leaky Bucket Rate Limiter | Claude | ✅ Complete | TokenBucket replaces Semaphore; rate_limit_per_minute wired from config (commit 4591a90) |
| 9.2 | Granular IG Error Enum | Claude | ✅ Complete | errors.rs activated (was orphan); handle_response() uses IGError; UNAUTHORIZED sentinel preserved (commit 4591a90) |

---

## Phase 10 — Connectivity & Intelligence

| # | Task | Owner | Status | Notes |
|---|------|-------|--------|-------|
| 10.1 | Client Sentiment Indicators | Claude | ✅ Done | GlobalSentimentRegistry activated in state.rs; get_client_sentiment() on TraderAPI; 15-min poll timer in event_loop; rest_client + mock_client implemented |
| 10.2 | Related Market Sentiment | Claude | ✅ Done | context_market_ids polled alongside trading epics; all market IDs update the same GlobalSentimentRegistry |
| 10.3 | Recursive API Pagination | Claude | ✅ Done | get_account_activity() follows metadata.paging.next until exhausted; Version 1 |
| 10.4 | Watchlist Syncing (BOT_ACTIVE) | Claude | ✅ Done | 1-hr watchlist_sync_interval; fetches BOT_ACTIVE watchlist and adds new epics to state dynamically |

---

## Phase 5 — Advanced Strategy Features

| # | Task | Status | Owner | Notes |
|---|------|--------|-------|-------|
| 5.1 | Trailing Stop Loss logic | ✅ Done | Claude + Gemini | Ratchet logic + strategy-specific distances from config; configurable `trailing_stop_min_pips` in RiskConfig |
| 5.2 | Session-specific filters (news exclusion) | ✅ Done | Claude | Session filter + news blackout windows (±15min configurable) |
| 5.4 | Shadow Mode (Paper mode strategy validation) | ✅ Done | — | Mapped to Paper mode |

---

## Live Trading Transition (In Progress)

| # | Task | Owner | Status | Notes |
|---|------|-------|--------|-------|
| L.1 | Close position API fix (POST + _method:DELETE) | Gemini | ✅ Done | Was completely broken — DELETE verb not supported by IG |
| L.2 | Currency code hybrid logic (JPY/USD/account-base) | Gemini | ✅ Done | Per-trade currency + Position.currency field |
| L.3 | Verify min deal sizes on demo | Gemini | ✅ Done | EUR/USD=0.5, GBP/USD=0.5, USD/JPY=0.2, Gold=3.0 |
| L.4 | Config-driven guaranteed_stop | Gemini | ✅ Done | Reads from config.risk.limited_risk_account, not hardcoded |
| L.5 | api_lab CLI tool (Rust + Python) | Gemini | ✅ Done | List, close, clear_profit, inject trades |
| L.6 | Complete `config/live.toml` (all sections) | Claude | ✅ Done | Fixed 7 wrong field names, added 9 macro events, instrument specs, ADX overrides |
| L.7 | Create `config/live-ramp.toml` | Claude | ✅ Done | USD/JPY only, 0.25% risk, 1 position, 5 trades/day |
| L.8 | Live startup validation in Rust | Claude | ✅ Done | validate_live_readiness() — macro events, spec completeness, margin feasibility |
| L.9 | Verify live epic codes + min deal sizes | User | ⏳ Waiting | Must check on live IG platform before going live |

---

## Known Bugs / Open Issues

| Priority | Description | File(s) |
|----------|-------------|---------|
| High | Python test scripts (`test_ig_trade*.py`) fail in any proxied/sandboxed environment — `ProxyError: 403 Forbidden` on IG API. Must run locally or in Docker. | `test_ig_trade*.py` |
| Low | OPU parse failures (unknown field `guaranteedStop`) — non-blocking, position updates still work | `event_loop/mod.rs` |

### Recently Fixed (2026-03-12)

| Bug | Root Cause | Fix | Commit |
|-----|-----------|-----|--------|
| ALL market analysis silently skipped | `MARKET_STATE` comparison was `!= "TRADEABLE"` (uppercase) but IG sends lowercase `"tradeable"` — `debug!` log hidden in INFO mode | Changed to `to_ascii_uppercase().starts_with("TRADEABLE")` | 169d22f |
| Indicators never reached warmup (19 of 250 candles used) | `snapshotTime` parse used `%Y/%m/%d %H:%M:%S` but IG API returns `"YYYY/MM/DD HH:mm:ss:SSS"` (colon + ms suffix); mock client returns RFC3339 — both failed silently → all 250 candles got `Utc::now()` → deduplicated to 1 | Multi-format parse: try RFC3339 → strip `:SSS` → IG format → warn! on failure | 706a1dd |

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
| 8.5 | Macro calendar awareness | Claude | ✅ Done | 🟡 Low | Per-event blackout windows + live ForexFactory calendar (`scripts/fetch_calendar.py` → `data/economic_calendar.json`, 26h stale fallback). London Open blocks removed — only fire on actual event days. |
| 8.6 | RL position sizing | Claude | 🏗️ Long-term | 🔵 | PPO on live trade outcomes. `TradeLogger` recording to `logs/trades.jsonl`. Needs 3+ months data. |
| 8.7 | Code quality pass — zero clippy warnings | Claude | ✅ Done | 🔴 High | `cargo clippy -- -D warnings` exits 0. All 74 tests pass. |

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
