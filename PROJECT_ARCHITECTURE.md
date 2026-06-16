# PROJECT_ARCHITECTURE.md — IG Trading Engine

**Last updated:** 2026-06-11 (Phase 17.F — EURUSD M15 SL/TP instrument override, BE-snap trigger 0.9)
**Scope:** Rust engine (`ig-engine/`) — bot + Telegram only (dashboard archived)

---

## System Overview

The IG Trading Engine is an autonomous trading system composed of two layers:

1. **Rust Engine (`ig-engine/`)** — Core trading logic. Connects to IG Markets APIs, runs technical analysis, enforces risk rules, executes trades, and exposes an internal HTTP + WebSocket API.
2. **Telegram Notifications** — Trade alerts, risk events, daily P&L summary.

> 📦 A Next.js dashboard exists in `src/` but is **archived** — not maintained.

---

## High-Level Data Flow

```
IG Markets (REST + Lightstreamer)
         │
         │  1. REST: auth, prices, positions, orders, confirmations
         │  2. Lightstreamer WS: live price ticks, account events, trade events
         ▼
┌─────────────────────────────────────────────────────────┐
│                Rust Engine (ig-engine)                   │
│                                                          │
│  [Startup]                                               │
│  ├── Load config/default.toml                            │
│  ├── Authenticate with IG REST API                       │
│  ├── Warm up candles per epic (disk-first strategy):     │
│  │     1. Load from data/candles/*.jsonl                 │
│  │     2. If ≥210 bars on disk → use disk, skip REST API │
│  │     3. Else try REST API 250 bars → merge with disk   │
│  │     4. Persist merged result back to disk              │
│  ├── Initialise IndicatorSet per epic                    │
│  ├── Spawn Lightstreamer streaming task (auto-reconnect) │
│  └── Start Axum HTTP/WS server (port 9090)               │
│                                                          │
│  [Runtime — tokio::select! loop]                         │
│  ├── MarketUpdate (Lightstreamer tick)                   │
│  │     └── analysis.rs                                  │
│  │           ├── IndicatorSet::update()                  │
│  │           ├── Strategy::generate_signal() × 4         │
│  │           ├── EnsembleVoter::vote()                   │
│  │           ├── RiskManager::check()  ◄── HARD GATE     │
│  │           └── IGRestClient::open_position()           │
│  │                  └── OrderManager::confirm()          │
│  ├── position_monitor_interval (every 5s)                │
│  │     └── handlers.rs — SL/TP hit detection, trailing  │
│  ├── candle_refresh_interval (every 15 min)              │
│  │     └── Fetch fresh HOUR candles → update H1 indicators│
│  ├── Lightstreamer tick (continuous)                     │
│  │     └── bar_accumulator (H1) + bar_accumulator_m15    │
│  │         On M15 bar close: push CandleStore + indicators│
│  │         + persist to disk (MINUTE_15.jsonl)            │
│  ├── m15_refresh_interval (every 60s) [Phase 14]         │
│  │     ├── Try IG API: fetch MINUTE_15 candles → dedup   │
│  │     │   → update M15 indicators → analyze_market_m15()│
│  │     └── API fail: if tick-warmed → analyze_market_m15()│
│  ├── session_refresh_interval (every 50 min)             │
│  │     └── Refresh IG CST + security tokens              │
│  ├── heartbeat_interval                                  │
│  │     └── Broadcast uptime + position count             │
│  └── daily_summary_interval (07:55 SGT, config-driven)   │
│        └── Telegram daily P&L summary                    │
│                                                          │
│  State:   Arc<RwLock<EngineState>>                       │
│  Events:  tokio broadcast channel<EngineEvent>           │
│                                                          │
│  [Shutdown — SIGTERM or Ctrl+C]                          │
│  ├── Shutdown event broadcast                            │
│  ├── Event loop exits cleanly                            │
│  ├── Persist all candle series to disk                   │
│  └── IG session logout                                   │
└─────────────────────────────────────────────────────────┘
         │
         │  HTTP + WebSocket  (localhost:9090)
         ▼
   External consumers (curl, Telegram, archived dashboard)
```

---

## Rust Engine — Module Breakdown

### `api/`

| File | Responsibility |
|------|----------------|
| `auth.rs` | Login, session token management |
| `rest_client.rs` | `IGRestClient` — all IG REST endpoints; includes Leaky Bucket rate limiting and granular error mapping |
| `streaming_client.rs` | Lightstreamer WebSocket client; subscribes to prices, account, trades; auto-reconnects on failure |
| `traits.rs` | `TraderAPI` trait — production and mock share the same interface |
| `mock_client.rs` | In-memory mock for integration tests — no real API calls |
| `types.rs` | Serde structs for all IG API request/response payloads |
| `errors.rs` | (Planned) Custom `IGError` enum for structured error recovery |

### `engine/`

| File | Responsibility |
|------|----------------|
| `config.rs` | `EngineConfig` — deserialised from `config/default.toml` |
| `state.rs` | `EngineState` partitioned into: `AccountState`, `MarketStateContainer`, `TradeState`, `MetricsState`, `LearningState`, `SessionState` |
| `order_manager.rs` | Polls `/confirms/{dealReference}` with configurable retries until confirmed or rejected |
| `optimizer.rs` | Grid search over strategy parameter space using historical P&L |
| `backtester.rs` | Replays candle history through a strategy to compute simulated returns |
| `event_loop/mod.rs` | Main `run()` — `tokio::select!` over all event streams and timers |
| `event_loop/analysis.rs` | Full analysis pipeline: indicators → signals → ensemble vote → risk gate → execution |
| `event_loop/handlers.rs` | Live position monitoring: SL/TP checks, trailing stop updates, position close |
| `event_loop/learning.rs` | Builds `LearningSnapshot` for the dashboard API |
| `event_loop/validation.rs` | Pre-flight config checks (epic format, risk param bounds, time windows) |

### `indicators/`

All indicators operate on an in-memory ring buffer — no disk reads during trading.
`IndicatorSet` holds one instance of each indicator per epic and is updated on every candle.

| Indicator | Default Parameters |
|-----------|--------------------|
| SMA | Configurable period |
| EMA | Configurable period |
| RSI | Period 14 |
| MACD | Fast 12 / Slow 26 / Signal 9 |
| Bollinger Bands | Period 20, 2σ |
| ATR | Period 14 — drives SL/TP distance |
| ADX | Period 14 — trend strength filter for MA Crossover |
| Stochastic | %K / %D |

### `strategy/`

H1 strategies implement `Strategy::evaluate(epic, price, indicators) → Option<Signal>`.
M15 strategies implement `M15Strategy::evaluate_m15(epic, price, m15_snap, h1_snap, regime) → Option<Signal>`.
Signals carry: direction, strength (0–10), stop loss price, take profit price.

**6 vote sources (H1 timeframe) + 3 M15 strategies:**

| Strategy | Signal Logic | Regime Role |
|----------|-------------|-------------|
| `MACrossoverStrategy` | EMA9/21 cross + EMA200 trend filter; ADX > threshold | Trending: 1.2× · Ranging: 0.6× |
| `RSIReversalStrategy` | RSI oversold/overbought + optional price divergence | Ranging: 1.2× · Trending: 0.7× |
| `MACDMomentumStrategy` | MACD histogram sign change (crossover) | Trending: 1.2× |
| `BollingerStrategy` | Price at outer band → mean reversion entry | Ranging: 1.2× · Trending: 0.6× |
| `MultiTimeframeStrategy` | HOUR_4/HOUR/MINUTE_15 EMA alignment; dynamic strength via ADX+MACD | Trending: 1.5× |
| `StochasticMomentumStrategy` | %K/%D crossover in overbought/oversold zone; ADX+RSI strength bonuses | Ranging: 1.2× · VOLATILE: 0.8× |
| `GoldSentimentStrategy` | RSS sentiment score ≥±0.55; keyword/Claude/Ollama backends | ALL regimes: 1.0× |

**3 M15 vote sources (MINUTE_15 timeframe, H1 as directional filter):**

| Strategy | Signal Logic | Active Regimes | Multiplier |
|---|---|---|---|
| `M15_MomentumBurst` | M15 RSI 55–75 + MACD hist expanding + H1 EMA200 confirm | Trending, Volatile | VOLATILE 1.3× · TRENDING 1.2× |
| `M15_EmaMicrotrend` | M15 EMA9>EMA21 + EMA21 slope + H1 EMA21 slope confirm | Trending, Volatile | TRENDING 1.2× |
| `M15_BollingerReversion` | M15 %B<0.05 + RSI<35 + H1 RSI>35 (mean reversion) | Ranging ONLY | RANGING 1.2× |

M15 ensemble: `m15_min_consensus=2, m15_min_avg_strength=6.5`. Position size: 0.5× H1 via `check_trade_m15()`. Cooldown: max 2 trades per H1 candle. R:R = 2.67 (SL 1.5× ATR, TP 4.0× ATR). All enabled — `config/default.toml`.

**Per-instrument M15 SL/TP override (Phase 17.F):** `[strategies.instrument_overrides."<epic>"]`
`m15_atr_sl_multiplier` / `m15_atr_tp_multiplier` recompute the ensemble signal's SL/TP from M15 ATR
before the risk gate (`analysis.rs`, M15 path). EURUSD ships 2.5× / 6.5× (R:R 2.6) — the strategy
default 1.5× ATR ≈ 5–6 pips sat inside spread noise and whipsawed out in both directions. Trailing
distance + BE-snap trigger derive from SL distance, so they widen with it. Guarded by
`tests/config_load.rs` (parses the real TOML, asserts R:R clears `min_risk_reward`).

> ⏰ **Trading hours single source of truth:** the `[trading_hours]` section (07:00–20:00 UTC).
> The engine overwrites `risk.trading_hours_utc` from it at startup (`event_loop/mod.rs`) — do not
> set `trading_hours_utc` under `[risk]`.

**H1 Direction Gate + Alignment Bonus (Phase 14.E):**
- `H1DirectionBias` (buy_count vs sell_count from H1 strategies) stored per epic in `MarketStateContainer.h1_bias`
- Gate: M15 signal contradicting H1 majority is blocked and logged
- Bonus: M15 signals agreeing with H1 bias get ×1.2 strength boost before ensemble vote

**M15 candle data resilience (Phase 14.F–H):**
- Tick accumulator (`bar_accumulator_m15`) builds M15 bars from live Lightstreamer ticks — never loses data
- Bars persisted to `data/candles/*_MINUTE_15.jsonl` on every close
- Self-heal: if indicators not warmed at 60s tick, fetches 250 bars from IG API automatically
- Fallback analysis: `analyze_market_m15()` runs from tick-warmed indicators even if IG API is rate-limited

**Ensemble Voting:**

```
Full consensus:    min 3 strategies, avg strength ≥ 7.5 → full position
VOLATILE scalp:    min 2 strategies, avg strength ≥ 7.5 → 0.5× position (VOLATILE regime only)
```

**Regime Multipliers (VOLATILE_MUTE = 0.5):**

| Strategy | Trending | Ranging | Volatile |
|---|---|---|---|
| MA_Crossover | 1.2× | 0.6× | 0.5× |
| RSI_Reversal | 0.7× | 1.2× | 0.5× |
| MACD_Momentum | 1.2× | 0.8× | 0.5× |
| Bollinger | 0.6× | 1.2× | 0.5× |
| Multi_Timeframe | 1.5× | 0.8× | 0.5× |
| Stochastic_Momentum | 0.5× | 1.2× | **0.8×** |
| Gold_Sentiment | 1.0× | 1.0× | **1.0×** |

**Signal Boosters** (applied before regime multipliers):
- ATR expansion: `bar_range > ATR × 1.5` → +1.0 to all signals
- Key level proximity: price within 0.1% of round level ($50 Gold / 0.50 JPY / 0.005 FX) → ×1.2 breakout-aligned

**Regime Cooldown** (Phase 17): When VOLATILE persists for `regime_cooldown_days` (default 7), the engine progressively relaxes VOLATILE restrictions:
- SL multiplier: 1.0× → `regime_cooldown_sl_multiplier` (1.25×)
- TP multiplier: 2.5× → `regime_cooldown_tp_multiplier` (3.0×)
- Breakeven snap: disabled (`regime_cooldown_disable_be_snap`)
- Persistence tracked in `data/regime_persistence.json` (per-instrument, records when regime last changed)

### `risk/`

`RiskManager::check()` is a hard gate. Called before every trade. Returns `Ok(size)` or `Err(reason)`. Nothing bypasses it.

Checks run in this order:

1. Engine status is `Running`
2. Current UTC time is within configured trading hours
3. Current trading session (Asia/London/US) is in the allowed list
4. Circuit breaker is not active
5. Daily loss limit not breached
6. Weekly drawdown limit not breached
7. Max open positions not exceeded
8. Max margin usage not exceeded
9. Risk/reward ratio ≥ `min_risk_reward`
10. Position size calculated via `position_sizer.rs` (Half-Kelly default)

Circuit breaker triggers: reduces position size after 3 consecutive losses; pauses trading 60 minutes after 5.

### `learning/`

| File | Responsibility |
|------|----------------|
| `scorecard.rs` | `StrategyScorecard` — rolling 50-trade window tracking win rate, profit factor, and per-session stats |
| `adaptive_weights.rs` | `AdaptiveWeightManager` — recalculates weight multipliers every 10 trades; feeds updated weights back into `EnsembleVoter` |

### `ipc/`

| File | Responsibility |
|------|----------------|
| `http_server.rs` | Axum server: REST endpoints + `/ws` WebSocket; reads from shared state |
| `events.rs` | `EngineEvent` enum — broadcast over tokio channel to all subscribers |

### `notifications/`

`TelegramNotifier` sends alerts on: trade opened, trade closed, risk limit hit, circuit breaker trigger, and the daily P&L summary at the `[notifications.telegram] summary_time` (SGT, default **07:55**). The summary fires late-SGT/morning so it covers the complete overnight UTC trading day (entries 07:00–20:00 UTC) before the 00:00 UTC stats reset — firing mid-day would structurally miss the US session.

---

## State Management

### Rust — `EngineState` partitions

| Sub-state | Key fields |
|-----------|------------|
| `AccountState` | balance, available, equity, P&L, currency |
| `MarketStateContainer` | live prices per epic, `IndicatorSet` per epic, `CandleStore` (JSONL disk persistence), `bar_accumulator` (H1), `bar_accumulator_m15` (M15 — builds candles from live ticks), `h1_bias` (H1DirectionBias per epic) |
| `TradeState` | active positions, last 200 signals, last 500 closed trades |
| `MetricsState` | daily stats (trades, wins, P&L, drawdown), circuit breaker state |
| `LearningState` | scorecard, weight manager, snapshot for API |
| `SessionState` | IG CST + security tokens (refreshed every 50 min) |

**Concurrency rule:** Acquire `Arc<RwLock<EngineState>>`, do minimal synchronous work, drop lock, then do async I/O. Never hold the lock across an `.await`.

---

## Key Interfaces

### `TraderAPI` (Rust trait)

```rust
#[async_trait]
pub trait TraderAPI {
    async fn open_position(&mut self, req: OpenPositionRequest) -> Result<DealReference>;
    async fn close_position(&mut self, deal_id: &str, size: f64) -> Result<()>;
    async fn get_positions(&self) -> Result<Vec<Position>>;
    async fn get_accounts(&self) -> Result<AccountsResponse>;
    async fn get_price_history(&self, epic: &str, resolution: &str, count: usize) -> Result<PriceResponse>;
    async fn refresh_session(&mut self) -> Result<()>;
    async fn logout(&mut self) -> Result<()>;
}
```

### `Strategy` (Rust trait)

```rust
pub trait Strategy {
    fn name(&self) -> &str;
    fn weight(&self) -> f64;
    fn generate_signal(&self, candles: &[Candle], indicators: &IndicatorSet) -> Option<Signal>;
}
```

---

## Concurrency Model

| Task | How spawned | Communicates via |
|------|-------------|-----------------|
| Engine event loop | Main task | Reads from broadcast channel |
| Lightstreamer streaming | `tokio::spawn` | Writes `MarketUpdate` to broadcast channel |
| HTTP/WS server | `tokio::spawn` | Reads `Arc<RwLock<EngineState>>`; subscribes to broadcast |
| Telegram notifications | `tokio::spawn` (fire-and-forget) | Called inline from event loop |

---

## Configuration Reference

All runtime behaviour lives in `config/default.toml`. Never hardcode values in source files.

| Section | Purpose |
|---------|---------|
| `[general]` | Mode (paper/demo/live), API port, heartbeat interval |
| `[ig]` | Environment, session refresh interval, rate limit, confirmation timeout |
| `[markets]` | List of epics to trade |
| `[risk]` | All limits: daily loss %, drawdown %, max positions, margin %, RR ratio |
| `[risk.instrument_specs.*]` | Per-epic: pip value, min/max deal size, margin %, pip scale |
| `[risk.circuit_breaker]` | Loss thresholds and pause durations |
| `[strategies.*]` | Per-strategy: enabled, weight, indicator parameters, ATR multipliers |
| `[trading_hours]` | Start/end UTC, allowed sessions |
| `[notifications.telegram]` | Enabled flag, alert types |

---

## Deployment

### Local Dev
```bash
cd ig-engine && cargo run --release  # Engine on :9090
```

### Docker
```bash
docker-compose up --build            # Engine + PostgreSQL + Redis
```

### Environment Variables
Copy `.env.example` → `.env`. Required: `IG_API_KEY`, `IG_IDENTIFIER`, `IG_PASSWORD`, `IG_ENVIRONMENT`.

> ⚠️ Direct internet access to `demo-api.ig.com` / `api.ig.com` (port 443) is required. Proxied or sandboxed environments will fail with `ProxyError: 403 Forbidden`.
