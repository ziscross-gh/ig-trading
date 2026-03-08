# PROJECT_ARCHITECTURE.md — IG Trading Engine

**Last updated:** 2026-03-08
**Scope:** Full system — Rust engine + Next.js dashboard

---

## System Overview

The IG Trading Engine is an autonomous trading system composed of two layers:

1. **Rust Engine (`ig-engine/`)** — Core trading logic. Connects to IG Markets APIs, runs technical analysis, enforces risk rules, executes trades, and exposes an internal HTTP + WebSocket API.
2. **Next.js Dashboard (`src/`)** — Web UI. Reads from and controls the Rust engine via its internal API. Displays live prices, positions, strategy signals, and adaptive learning state.

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
│  ├── Warm up 250 HOUR candles per epic (REST)            │
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
│  │     └── Fetch 20 fresh HOUR candles → update indicators│
│  ├── session_refresh_interval (every 50 min)             │
│  │     └── Refresh IG CST + security tokens              │
│  ├── heartbeat_interval                                  │
│  │     └── Broadcast uptime + position count             │
│  └── daily_summary_interval (21:00 SGT)                  │
│        └── Telegram daily P&L summary                    │
│                                                          │
│  State:   Arc<RwLock<EngineState>>                       │
│  Events:  tokio broadcast channel<EngineEvent>           │
│                                                          │
│  [Shutdown — SIGTERM or Ctrl+C]                          │
│  ├── Shutdown event broadcast                            │
│  ├── Event loop exits cleanly                            │
│  └── IG session logout                                   │
└─────────────────────────────────────────────────────────┘
         │
         │  HTTP + WebSocket  (localhost:9090)
         ▼
┌──────────────────────────────────────────┐
│   Next.js Dashboard (port 3000)           │
│                                           │
│   /api/engine/[...path] → proxy to 9090  │
│   useEngine() via EngineContext           │
│   Recharts + shadcn/ui components         │
└──────────────────────────────────────────┘
```

---

## Rust Engine — Module Breakdown

### `api/`

| File | Responsibility |
|------|----------------|
| `auth.rs` | Login, session token management |
| `rest_client.rs` | `IGRestClient` — all IG REST endpoints (orders, positions, prices, accounts, confirmations) |
| `streaming_client.rs` | Lightstreamer WebSocket client; subscribes to prices, account, trades; auto-reconnects on failure |
| `traits.rs` | `TraderAPI` trait — production and mock share the same interface |
| `mock_client.rs` | In-memory mock for integration tests — no real API calls |
| `types.rs` | Serde structs for all IG API request/response payloads |

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

Each strategy implements `Strategy::generate_signal(candles, indicators) → Option<Signal>`.
Signals carry: direction, strength (0–10), stop loss price, take profit price.

| Strategy | Signal Logic |
|----------|-------------|
| `MACrossoverStrategy` | Fast SMA crosses slow SMA; fires only if ADX > threshold |
| `RSIReversalStrategy` | RSI oversold/overbought + optional price divergence |
| `MACDMomentumStrategy` | MACD line crosses signal line |
| `BollingerStrategy` | Price closes outside outer band → mean reversion entry |
| `EnsembleVoter` | Aggregates signals with per-strategy weights; requires min consensus count AND min avg strength |

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

`TelegramNotifier` sends alerts on: trade opened, trade closed, risk limit hit, circuit breaker trigger, and the daily P&L summary at 21:00 SGT.

---

## Next.js Frontend — Component Breakdown

### Data Flow

```
Rust Engine (localhost:9090)
        │
        ▼
/api/engine/[...path]        ← Next.js proxy (avoids CORS, single origin)
        │
        ▼
useEngineAPI.ts              ← REST fetchers for all endpoints
useEngineWebSocket.ts        ← WebSocket subscriber, dispatches events
useEngineControl.ts          ← start / stop / pause / manual trigger
useEngineConfig.ts           ← Config read + write
        │
        ▼
useEngine.ts                 ← Facade hook: composes all sub-hooks + auto-refresh
        │
        ▼
EngineContext.tsx             ← React Context: exposes engine state to all components
        │
        ▼
Dashboard components         ← Consume via useEngine() or EngineContext
```

### Dashboard Components

| Component | What it shows |
|-----------|---------------|
| `EnginePanel.tsx` | Mode, status, start/stop/pause controls, daily P&L, win rate |
| `MarketOverview.tsx` | Live bid/ask price tiles per epic |
| `PriceChart.tsx` | Candlestick chart + indicator overlay (Recharts) |
| `TradeHistory.tsx` | Paginated table of closed trades |
| `LearningPanel.tsx` | Per-strategy adaptive weight bars, win rate, profit factor |
| `EquityCurvePanel.tsx` | Running equity curve over trade history |
| `StrategyLab.tsx` | Backtest runner: pick strategy + date range + params, view P&L results |
| `setup-panel.tsx` | Multi-step wizard: PreFlightChecks → EngineSettings → RiskSettings |

---

## State Management

### Rust — `EngineState` partitions

| Sub-state | Key fields |
|-----------|------------|
| `AccountState` | balance, available, equity, P&L, currency |
| `MarketStateContainer` | live prices per epic, `IndicatorSet` per epic, `CandleStore` |
| `TradeState` | active positions, last 200 signals, last 500 closed trades |
| `MetricsState` | daily stats (trades, wins, P&L, drawdown), circuit breaker state |
| `LearningState` | scorecard, weight manager, snapshot for API |
| `SessionState` | IG CST + security tokens (refreshed every 50 min) |

**Concurrency rule:** Acquire `Arc<RwLock<EngineState>>`, do minimal synchronous work, drop lock, then do async I/O. Never hold the lock across an `.await`.

### Frontend — `EngineState` (TypeScript)

Lives in `useEngine()` via `useState`. Populated by REST fetchers on mount; kept live via the WebSocket event stream. Distributed tree-wide via `EngineContext`.

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
cd ig-engine && cargo run         # Engine on :9090
bun dev                           # Dashboard on :3000
```

### Docker (Recommended for demo/live)
```bash
docker-compose up --build
# Starts: Rust engine + PostgreSQL + Redis
```

### Environment Variables
Copy `.env.example` → `.env`. Required: `IG_API_KEY`, `IG_IDENTIFIER`, `IG_PASSWORD`, `IG_ENVIRONMENT`.

> ⚠️ Direct internet access to `demo-api.ig.com` / `api.ig.com` (port 443) is required. Proxied or sandboxed environments will fail with `ProxyError: 403 Forbidden`.
